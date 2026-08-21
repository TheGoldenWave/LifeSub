@preconcurrency import AVFoundation
import CaptureProtocol
import CoreGraphics
import CoreMedia
import Foundation
import ScreenCaptureKit

struct DecodedSystemAudio: Sendable {
    let samples: [Float]
    let sampleRate: Double
    let hostTime: UInt64
}

enum SystemAudioSampleDecoder {
    static func decodePlanarFloat(
        channels: [[Float]],
        sampleRate: Double,
        hostTime: UInt64
    ) throws -> DecodedSystemAudio {
        guard
            sampleRate > 0,
            let frameCount = channels.first?.count,
            frameCount > 0,
            channels.allSatisfy({ $0.count == frameCount })
        else { throw CaptureSourceError.sourceFailed }
        var mono = [Float](repeating: 0, count: frameCount)
        for channel in channels {
            for frame in 0 ..< frameCount { mono[frame] += channel[frame] }
        }
        let divisor = Float(channels.count)
        for frame in 0 ..< frameCount { mono[frame] /= divisor }
        return DecodedSystemAudio(samples: mono, sampleRate: sampleRate, hostTime: hostTime)
    }
}

final class SystemAudioCapture: NSObject, @unchecked Sendable, CaptureSourceAdapter, SCStreamOutput, SCStreamDelegate {
    let source: Source = .systemAudio
    private let encoder: ChunkEncoder
    private let frames: BoundedAudioFrameQueue
    private let sampleQueue = DispatchQueue(label: "com.lifesub.capture.system-audio.samples")
    private let resampler = MonoPcm16Resampler()
    private let failureHandler: @Sendable (CaptureFailure) -> Void
    private let signalHandler: @Sendable (CaptureSourceSignal) -> Void
    private var stream: SCStream?

    init(
        encoder: ChunkEncoder,
        failureHandler: @escaping @Sendable (CaptureFailure) -> Void,
        signalHandler: @escaping @Sendable (CaptureSourceSignal) -> Void
    ) {
        self.encoder = encoder
        self.failureHandler = failureHandler
        self.signalHandler = signalHandler
        frames = BoundedAudioFrameQueue(
            label: "com.lifesub.capture.system-audio.encode",
            consumer: { frame in
                try await encoder.append(samples: frame.samples, hostTime: frame.hostTime)
            },
            failureHandler: failureHandler
        )
        super.init()
    }

    func preflight() async throws {
        if CGPreflightScreenCaptureAccess() { return }
        guard CGRequestScreenCaptureAccess() else {
            throw CaptureSourceError.permissionDenied
        }
    }

    func start() async throws {
        frames.startAccepting()
        do {
            let content = try await SCShareableContent.excludingDesktopWindows(
                false,
                onScreenWindowsOnly: true
            )
            guard let display = content.displays.first else {
                throw CaptureSourceError.deviceUnavailable
            }
            let filter = SCContentFilter(display: display, excludingWindows: [])
            let configuration = SCStreamConfiguration()
            configuration.width = 2
            configuration.height = 2
            configuration.minimumFrameInterval = CMTime(value: 1, timescale: 1)
            configuration.showsCursor = false
            configuration.capturesAudio = true
            configuration.excludesCurrentProcessAudio = true
            configuration.sampleRate = 48_000
            configuration.channelCount = 2
            let stream = SCStream(filter: filter, configuration: configuration, delegate: self)
            try stream.addStreamOutput(self, type: .audio, sampleHandlerQueue: sampleQueue)
            try await stream.startCapture()
            self.stream = stream
        } catch let error as CaptureSourceError {
            throw error
        } catch {
            throw CaptureSourceError.sourceFailed
        }
    }

    func pause() async throws {
        try await stream?.stopCapture()
    }

    func resume() async throws {
        do { try await stream?.startCapture() } catch { throw CaptureSourceError.sourceFailed }
    }

    func stop() async {
        if let stream {
            try? await stream.stopCapture()
            try? stream.removeStreamOutput(self, type: .audio)
        }
        stream = nil
        await frames.suspendAndDrain()
        try? await encoder.sealForDiscontinuity()
    }

    func sealForDiscontinuity() async {
        try? await encoder.sealForDiscontinuity()
    }

    func stream(
        _: SCStream,
        didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
        of type: SCStreamOutputType
    ) {
        guard type == .audio, sampleBuffer.isValid, let decoded = Self.monoSamples(from: sampleBuffer) else {
            return
        }
        let samples: [Int16]
        do {
            samples = try resampler.convert(
                floatSamples: decoded.samples,
                sampleRate: decoded.sampleRate
            )
        } catch {
            failureHandler(.conversionFailed)
            Task { await stop() }
            return
        }
        do {
            try frames.submit(CapturedPcmFrame(samples: samples, hostTime: decoded.hostTime))
        } catch {
            Task { await stop() }
        }
    }

    func stream(_: SCStream, didStopWithError error: any Error) {
        let permissionGranted = CGPreflightScreenCaptureAccess()
        if permissionGranted {
            signalHandler(
                .interrupted(
                    source: .systemAudio,
                    reason: "stream_stopped",
                    recoverable: true
                )
            )
        } else {
            signalHandler(.permissionRevoked(source: .systemAudio))
        }
        _ = error
    }

    private static func monoSamples(
        from sampleBuffer: CMSampleBuffer
    ) -> DecodedSystemAudio? {
        guard
            let formatDescription = sampleBuffer.formatDescription,
            let description = CMAudioFormatDescriptionGetStreamBasicDescription(formatDescription)?.pointee,
            description.mFormatID == kAudioFormatLinearPCM
        else { return nil }
        let channels = Int(description.mChannelsPerFrame)
        let frameCount = sampleBuffer.numSamples
        guard channels > 0, frameCount > 0 else { return nil }
        let presentationTime = sampleBuffer.presentationTimeStamp
        guard presentationTime.isValid, presentationTime.value >= 0 else { return nil }
        let hostTime = UInt64(
            CMTimeConvertScale(presentationTime, timescale: 1_000_000_000, method: .default).value
        )

        var sizeNeeded = 0
        var retainedBlock: CMBlockBuffer?
        let sizeStatus = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
            sampleBuffer,
            bufferListSizeNeededOut: &sizeNeeded,
            bufferListOut: nil,
            bufferListSize: 0,
            blockBufferAllocator: kCFAllocatorDefault,
            blockBufferMemoryAllocator: kCFAllocatorDefault,
            flags: 0,
            blockBufferOut: &retainedBlock
        )
        guard sizeStatus == noErr, sizeNeeded >= MemoryLayout<AudioBufferList>.size else { return nil }

        return withUnsafeTemporaryAllocation(
            byteCount: sizeNeeded,
            alignment: MemoryLayout<AudioBufferList>.alignment
        ) { storage in
            let list = storage.baseAddress!.bindMemory(to: AudioBufferList.self, capacity: 1)
            let status = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
                sampleBuffer,
                bufferListSizeNeededOut: nil,
                bufferListOut: list,
                bufferListSize: sizeNeeded,
                blockBufferAllocator: kCFAllocatorDefault,
                blockBufferMemoryAllocator: kCFAllocatorDefault,
                flags: 0,
                blockBufferOut: &retainedBlock
            )
            guard status == noErr else { return nil }
            let buffers = UnsafeMutableAudioBufferListPointer(list)
            let planar = description.mFormatFlags & kAudioFormatFlagIsNonInterleaved != 0
            let decodedChannels: [[Float]]?
            if description.mBitsPerChannel == 32,
               description.mFormatFlags & kAudioFormatFlagIsFloat != 0 {
                decodedChannels = decodeFloatBuffers(
                    buffers,
                    channelCount: channels,
                    frameCount: frameCount,
                    planar: planar
                )
            } else if description.mBitsPerChannel == 16,
                      description.mFormatFlags & kAudioFormatFlagIsSignedInteger != 0 {
                decodedChannels = decodeInt16Buffers(
                    buffers,
                    channelCount: channels,
                    frameCount: frameCount,
                    planar: planar
                )
            } else {
                decodedChannels = nil
            }
            guard let decodedChannels else { return nil }
            return try? SystemAudioSampleDecoder.decodePlanarFloat(
                channels: decodedChannels,
                sampleRate: description.mSampleRate,
                hostTime: hostTime
            )
        }
    }

    private static func decodeFloatBuffers(
        _ buffers: UnsafeMutableAudioBufferListPointer,
        channelCount: Int,
        frameCount: Int,
        planar: Bool
    ) -> [[Float]]? {
        if planar {
            guard buffers.count >= channelCount else { return nil }
            return (0 ..< channelCount).compactMap { channel in
                guard let data = buffers[channel].mData else { return nil }
                let values = data.assumingMemoryBound(to: Float.self)
                return Array(UnsafeBufferPointer(start: values, count: frameCount))
            }
        }
        guard let data = buffers.first?.mData else { return nil }
        let values = data.assumingMemoryBound(to: Float.self)
        return (0 ..< channelCount).map { channel in
            (0 ..< frameCount).map { values[$0 * channelCount + channel] }
        }
    }

    private static func decodeInt16Buffers(
        _ buffers: UnsafeMutableAudioBufferListPointer,
        channelCount: Int,
        frameCount: Int,
        planar: Bool
    ) -> [[Float]]? {
        if planar {
            guard buffers.count >= channelCount else { return nil }
            return (0 ..< channelCount).compactMap { channel in
                guard let data = buffers[channel].mData else { return nil }
                let values = data.assumingMemoryBound(to: Int16.self)
                return (0 ..< frameCount).map { Float(values[$0]) / Float(Int16.max) }
            }
        }
        guard let data = buffers.first?.mData else { return nil }
        let values = data.assumingMemoryBound(to: Int16.self)
        return (0 ..< channelCount).map { channel in
            (0 ..< frameCount).map {
                Float(values[$0 * channelCount + channel]) / Float(Int16.max)
            }
        }
    }
}
