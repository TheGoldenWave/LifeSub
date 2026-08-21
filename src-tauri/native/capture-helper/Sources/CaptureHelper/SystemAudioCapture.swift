@preconcurrency import AVFoundation
import CaptureProtocol
import CoreGraphics
import CoreMedia
import Foundation
import ScreenCaptureKit

final class SystemAudioCapture: NSObject, @unchecked Sendable, CaptureSourceAdapter, SCStreamOutput {
    let source: Source = .systemAudio
    private let encoder: ChunkEncoder
    private let frames: BoundedAudioFrameQueue
    private let sampleQueue = DispatchQueue(label: "com.lifesub.capture.system-audio.samples")
    private let resampler = MonoPcm16Resampler()
    private var stream: SCStream?

    init(
        encoder: ChunkEncoder,
        failureHandler: @escaping @Sendable (any Error) -> Void
    ) {
        self.encoder = encoder
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
            let stream = SCStream(filter: filter, configuration: configuration, delegate: nil)
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
        guard
            type == .audio,
            sampleBuffer.isValid,
            let decoded = Self.monoSamples(from: sampleBuffer),
            let samples = try? resampler.convert(
                floatSamples: decoded.samples,
                sampleRate: decoded.sampleRate
            )
        else {
            return
        }
        do {
            try frames.submit(CapturedPcmFrame(samples: samples, hostTime: mach_continuous_time()))
        } catch {
            Task { await stop() }
        }
    }

    private static func monoSamples(
        from sampleBuffer: CMSampleBuffer
    ) -> (samples: [Float], sampleRate: Double)? {
        guard
            let formatDescription = sampleBuffer.formatDescription,
            let description = CMAudioFormatDescriptionGetStreamBasicDescription(formatDescription)?.pointee,
            description.mFormatID == kAudioFormatLinearPCM,
            let block = sampleBuffer.dataBuffer
        else { return nil }
        let channels = Int(description.mChannelsPerFrame)
        let frameCount = sampleBuffer.numSamples
        guard channels > 0, frameCount > 0 else { return nil }
        var data = Data(count: CMBlockBufferGetDataLength(block))
        let status = data.withUnsafeMutableBytes { bytes in
            CMBlockBufferCopyDataBytes(
                block,
                atOffset: 0,
                dataLength: bytes.count,
                destination: bytes.baseAddress!
            )
        }
        guard status == kCMBlockBufferNoErr else { return nil }

        var mono = [Float](repeating: 0, count: frameCount)
        if description.mBitsPerChannel == 32,
           description.mFormatFlags & kAudioFormatFlagIsFloat != 0 {
            data.withUnsafeBytes { bytes in
                let values = bytes.bindMemory(to: Float.self)
                guard values.count >= frameCount * channels else { return }
                for frame in 0 ..< frameCount {
                    var mixed: Float = 0
                    for channel in 0 ..< channels { mixed += values[frame * channels + channel] }
                    mixed = max(-1, min(1, mixed / Float(channels)))
                    mono[frame] = mixed / Float(channels)
                }
            }
            return (mono, description.mSampleRate)
        }
        if description.mBitsPerChannel == 16,
           description.mFormatFlags & kAudioFormatFlagIsSignedInteger != 0 {
            data.withUnsafeBytes { bytes in
                let values = bytes.bindMemory(to: Int16.self)
                guard values.count >= frameCount * channels else { return }
                for frame in 0 ..< frameCount {
                    var mixed = 0
                    for channel in 0 ..< channels { mixed += Int(values[frame * channels + channel]) }
                    mono[frame] = Float(mixed / channels) / Float(Int16.max)
                }
            }
            return (mono, description.mSampleRate)
        }
        return nil
    }
}
