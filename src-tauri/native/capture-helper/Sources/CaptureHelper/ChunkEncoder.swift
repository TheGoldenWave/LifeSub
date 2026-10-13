import AVFoundation
import CaptureProtocol
import CryptoKit
import Foundation

struct SealedChunk: Equatable, Sendable {
    let source: Source
    let relativePath: String
    let byteLength: UInt64
    let sha256: String
    let frameCount: UInt64
    let startSamplePosition: UInt64
    let endSamplePosition: UInt64
    let startHostTime: UInt64
    let endHostTime: UInt64
}

protocol SealedChunkSink: Sendable {
    func didSeal(_ chunk: SealedChunk) async
}

enum ChunkEncoderError: Error {
    case invalidConfiguration
    case stagingPathEscaped
    case bufferAllocationFailed
}

struct CapturedPcmFrame: Sendable {
    let samples: [Int16]
    let hostTime: UInt64
}

enum AudioBackpressureError: Error {
    case queueFull
    case stopped
}

enum CaptureFailure: String, Error, Equatable, Sendable {
    case backpressure = "audio_backpressure"
    case queueStopped = "audio_queue_stopped"
    case conversionFailed = "audio_conversion_failed"
    case encodingFailed = "audio_encoding_failed"
}

final class MonoPcm16Resampler: @unchecked Sendable {
    private let targetSampleRate: Double
    private let lock = NSLock()
    private var converterSampleRate: Double?
    private var inputFormat: AVAudioFormat?
    private var outputFormat: AVAudioFormat?
    private var converter: AVAudioConverter?

    init(targetSampleRate: Double = 16_000) {
        self.targetSampleRate = targetSampleRate
    }

    func convert(floatSamples: [Float], sampleRate: Double) throws -> [Int16] {
        try lock.withLock {
            guard !floatSamples.isEmpty, sampleRate > 0 else { return [] }
            if sampleRate == targetSampleRate {
                converterSampleRate = nil
                inputFormat = nil
                outputFormat = nil
                converter = nil
                return floatSamples.map { Int16(max(-1, min(1, $0)) * Float(Int16.max)) }
            }
            if converterSampleRate != sampleRate || converter == nil {
                inputFormat = AVAudioFormat(
                    commonFormat: .pcmFormatFloat32,
                    sampleRate: sampleRate,
                    channels: 1,
                    interleaved: false
                )
                outputFormat = AVAudioFormat(
                    commonFormat: .pcmFormatInt16,
                    sampleRate: targetSampleRate,
                    channels: 1,
                    interleaved: false
                )
                if let inputFormat, let outputFormat {
                    converter = AVAudioConverter(from: inputFormat, to: outputFormat)
                } else {
                    converter = nil
                }
                converterSampleRate = sampleRate
            }
            guard
                let inputFormat,
                let outputFormat,
                let converter,
                let input = AVAudioPCMBuffer(
                    pcmFormat: inputFormat,
                    frameCapacity: AVAudioFrameCount(floatSamples.count)
                ),
                let inputData = input.floatChannelData
            else { throw ChunkEncoderError.bufferAllocationFailed }
            input.frameLength = input.frameCapacity
            inputData[0].update(from: floatSamples, count: floatSamples.count)
            let capacity = AVAudioFrameCount(
                ceil(Double(floatSamples.count) * targetSampleRate / sampleRate) + 32
            )
            guard
                let output = AVAudioPCMBuffer(pcmFormat: outputFormat, frameCapacity: capacity),
                let outputData = output.int16ChannelData
            else { throw ChunkEncoderError.bufferAllocationFailed }
            var supplied = false
            var conversionError: NSError?
            let status = converter.convert(to: output, error: &conversionError) { _, inputStatus in
                if supplied {
                    inputStatus.pointee = .noDataNow
                    return nil
                }
                supplied = true
                inputStatus.pointee = .haveData
                return input
            }
            guard status != .error, conversionError == nil else {
                throw conversionError ?? ChunkEncoderError.bufferAllocationFailed
            }
            return Array(UnsafeBufferPointer(start: outputData[0], count: Int(output.frameLength)))
        }
    }
}

final class BoundedAudioFrameQueue: @unchecked Sendable {
    private let slots: DispatchSemaphore
    private let queue: DispatchQueue
    private let consumer: @Sendable (CapturedPcmFrame) async throws -> Void
    private let failureHandler: @Sendable (CaptureFailure) -> Void
    private let pending = DispatchGroup()
    private let lock = NSLock()
    private var accepting = true

    init(
        capacity: Int = 64,
        label: String,
        consumer: @escaping @Sendable (CapturedPcmFrame) async throws -> Void,
        failureHandler: @escaping @Sendable (CaptureFailure) -> Void
    ) {
        slots = DispatchSemaphore(value: capacity)
        queue = DispatchQueue(label: label, qos: .userInitiated)
        self.consumer = consumer
        self.failureHandler = failureHandler
    }

    func submit(_ frame: CapturedPcmFrame) throws {
        lock.lock()
        guard accepting else {
            lock.unlock()
            let error = AudioBackpressureError.stopped
            failureHandler(.queueStopped)
            throw error
        }
        guard slots.wait(timeout: .now()) == .success else {
            lock.unlock()
            let error = AudioBackpressureError.queueFull
            failureHandler(.backpressure)
            throw error
        }
        pending.enter()
        lock.unlock()
        queue.async { [consumer, failureHandler, pending, slots] in
            let completed = DispatchSemaphore(value: 0)
            Task {
                do {
                    try await consumer(frame)
                } catch {
                    failureHandler(.encodingFailed)
                }
                completed.signal()
            }
            completed.wait()
            slots.signal()
            pending.leave()
        }
    }

    func startAccepting() {
        lock.lock()
        accepting = true
        lock.unlock()
    }

    func suspendAndDrain() async {
        lock.withLock { accepting = false }
        await withCheckedContinuation { continuation in
            pending.notify(queue: queue) { continuation.resume() }
        }
    }

    func finish() async {
        await suspendAndDrain()
    }
}

actor ChunkEncoder {
    private static let nanosecondsPerSecond = 1_000_000_000.0
    private let source: Source
    private let stagingRoot: URL
    private let sampleRate: Double
    private let channelCount: AVAudioChannelCount
    private let maxFramesPerChunk: AVAudioFramePosition
    private let sink: any SealedChunkSink
    private var file: AVAudioFile?
    private var activeRelativePath: String?
    private var relativePath: String?
    private var frameCount: AVAudioFramePosition = 0
    private var totalFrameCount: UInt64 = 0
    private var startHostTime: UInt64?
    private var endHostTime: UInt64 = 0
    private var firstHostTime: UInt64?

    init(
        source: Source,
        stagingRoot: URL,
        sampleRate: Double,
        channelCount: AVAudioChannelCount,
        maxFramesPerChunk: AVAudioFramePosition,
        sink: any SealedChunkSink
    ) throws {
        guard sampleRate > 0, channelCount > 0, maxFramesPerChunk > 0 else {
            throw ChunkEncoderError.invalidConfiguration
        }
        self.source = source
        self.stagingRoot = stagingRoot.standardizedFileURL
        self.sampleRate = sampleRate
        self.channelCount = channelCount
        self.maxFramesPerChunk = maxFramesPerChunk
        self.sink = sink
        try FileManager.default.createDirectory(
            at: self.stagingRoot,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
    }

    func append(samples: [Int16], hostTime: UInt64) async throws {
        if firstHostTime == nil { firstHostTime = hostTime }
        guard samples.count.isMultiple(of: Int(channelCount)) else {
            throw ChunkEncoderError.invalidConfiguration
        }
        let inputFrames = samples.count / Int(channelCount)
        var offset = 0
        while offset < inputFrames {
            let writeStartHostTime = hostTimeForFrameOffset(offset, from: hostTime)
            try openIfNeeded(hostTime: writeStartHostTime)
            let remaining = Int(maxFramesPerChunk - frameCount)
            let writeFrames = min(inputFrames - offset, remaining)
            try write(samples: samples, frameOffset: offset, frameCount: writeFrames)
            frameCount += AVAudioFramePosition(writeFrames)
            offset += writeFrames
            endHostTime = hostTimeForFrameOffset(offset, from: hostTime)
            if frameCount == maxFramesPerChunk {
                try await seal()
            }
        }
    }

    func sealForDiscontinuity() async throws {
        if frameCount > 0 {
            try await seal()
        }
    }

    func stopMetadata() -> CaptureSourceStopMetadata {
        CaptureSourceStopMetadata(
            finalSequence: totalFrameCount,
            startHostTime: firstHostTime ?? 0,
            endHostTime: endHostTime
        )
    }

    private func openIfNeeded(hostTime: UInt64) throws {
        guard file == nil else { return }
        let name = "chk_\(UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()).wav"
        let url = stagingRoot.appendingPathComponent(name, isDirectory: false).standardizedFileURL
        guard url.deletingLastPathComponent() == stagingRoot else {
            throw ChunkEncoderError.stagingPathEscaped
        }
        let format = AVAudioFormat(
            commonFormat: .pcmFormatInt16,
            sampleRate: sampleRate,
            channels: channelCount,
            interleaved: false
        )!
        file = try AVAudioFile(
            forWriting: url,
            settings: format.settings,
            commonFormat: .pcmFormatInt16,
            interleaved: false
        )
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
        activeRelativePath = name
        relativePath = "\(name).partial"
        startHostTime = hostTime
        endHostTime = hostTime
    }

    private func hostTimeForFrameOffset(_ frameOffset: Int, from hostTime: UInt64) -> UInt64 {
        let elapsed = UInt64(
            (Double(frameOffset) * Self.nanosecondsPerSecond / sampleRate).rounded()
        )
        return hostTime.addingReportingOverflow(elapsed).overflow
            ? UInt64.max
            : hostTime + elapsed
    }

    private func write(samples: [Int16], frameOffset: Int, frameCount: Int) throws {
        let format = file!.processingFormat
        guard let buffer = AVAudioPCMBuffer(
            pcmFormat: format,
            frameCapacity: AVAudioFrameCount(frameCount)
        ), let channels = buffer.int16ChannelData else {
            throw ChunkEncoderError.bufferAllocationFailed
        }
        buffer.frameLength = AVAudioFrameCount(frameCount)
        for channel in 0 ..< Int(channelCount) {
            for frame in 0 ..< frameCount {
                channels[channel][frame] = samples[(frameOffset + frame) * Int(channelCount) + channel]
            }
        }
        try file!.write(from: buffer)
    }

    private func seal() async throws {
        guard let activeRelativePath, let relativePath, let startHostTime else { return }
        let sealedFrames = frameCount
        let startSamplePosition = totalFrameCount
        let endSamplePosition = totalFrameCount + UInt64(sealedFrames)
        file = nil
        let activeURL = stagingRoot.appendingPathComponent(activeRelativePath, isDirectory: false)
        let partialURL = stagingRoot.appendingPathComponent(relativePath, isDirectory: false)
        let handle = try FileHandle(forWritingTo: activeURL)
        try handle.synchronize()
        try handle.close()
        try FileManager.default.moveItem(at: activeURL, to: partialURL)
        let attributes = try FileManager.default.attributesOfItem(atPath: partialURL.path)
        let byteLength = (attributes[.size] as? NSNumber)?.uint64Value ?? 0
        let digest = try SHA256.hash(data: Data(contentsOf: partialURL)).map { String(format: "%02x", $0) }.joined()
        self.activeRelativePath = nil
        self.relativePath = nil
        frameCount = 0
        self.startHostTime = nil
        totalFrameCount = endSamplePosition
        await sink.didSeal(
            SealedChunk(
                source: source,
                relativePath: relativePath,
                byteLength: byteLength,
                sha256: digest,
                frameCount: UInt64(sealedFrames),
                startSamplePosition: startSamplePosition,
                endSamplePosition: endSamplePosition,
                startHostTime: startHostTime,
                endHostTime: endHostTime
            )
        )
    }
}
