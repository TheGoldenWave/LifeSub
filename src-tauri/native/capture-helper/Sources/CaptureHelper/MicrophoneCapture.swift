@preconcurrency import AVFoundation
import CaptureProtocol
import Foundation
import os

final class MicrophoneCapture: @unchecked Sendable, CaptureSourceAdapter {
    let source: Source = .microphone
    private let engine = AVAudioEngine()
    private let encoder: ChunkEncoder
    private let frames: BoundedAudioFrameQueue
    private let resampler = MonoPcm16Resampler()
    private let running = OSAllocatedUnfairLock(initialState: false)

    init(
        encoder: ChunkEncoder,
        failureHandler: @escaping @Sendable (any Error) -> Void
    ) {
        self.encoder = encoder
        frames = BoundedAudioFrameQueue(
            label: "com.lifesub.capture.microphone",
            consumer: { frame in
                try await encoder.append(samples: frame.samples, hostTime: frame.hostTime)
            },
            failureHandler: failureHandler
        )
    }

    func preflight() async throws {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized: return
        case .notDetermined:
            guard await AVCaptureDevice.requestAccess(for: .audio) else {
                throw CaptureSourceError.permissionDenied
            }
        case .denied, .restricted:
            throw CaptureSourceError.permissionDenied
        @unknown default:
            throw CaptureSourceError.permissionDenied
        }
    }

    func start() async throws {
        guard running.withLock({ state in
            if state { return false }
            state = true
            return true
        }) else { return }
        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        guard format.sampleRate > 0, format.channelCount > 0 else {
            running.withLock { $0 = false }
            throw CaptureSourceError.deviceUnavailable
        }
        input.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self] buffer, time in
            self?.consume(buffer: buffer, hostTime: time.hostTime)
        }
        do {
            try engine.start()
        } catch {
            input.removeTap(onBus: 0)
            running.withLock { $0 = false }
            throw CaptureSourceError.sourceFailed
        }
    }

    func pause() async throws { engine.pause() }

    func resume() async throws {
        do { try engine.start() } catch { throw CaptureSourceError.sourceFailed }
    }

    func stop() async {
        guard running.withLock({ state in
            let wasRunning = state
            state = false
            return wasRunning
        }) else { return }
        engine.stop()
        engine.inputNode.removeTap(onBus: 0)
        try? await encoder.sealForDiscontinuity()
    }

    func sealForDiscontinuity() async {
        try? await encoder.sealForDiscontinuity()
    }

    private func consume(buffer: AVAudioPCMBuffer, hostTime: UInt64) {
        guard let channels = buffer.floatChannelData else { return }
        let channelCount = Int(buffer.format.channelCount)
        let frameCount = Int(buffer.frameLength)
        var mono = [Float](repeating: 0, count: frameCount)
        for frame in 0 ..< frameCount {
            var mixed: Float = 0
            for channel in 0 ..< channelCount { mixed += channels[channel][frame] }
            mono[frame] = mixed / Float(channelCount)
        }
        do {
            let samples = try resampler.convert(
                floatSamples: mono,
                sampleRate: buffer.format.sampleRate
            )
            try frames.submit(CapturedPcmFrame(samples: samples, hostTime: hostTime))
        } catch {
            Task { await stop() }
        }
    }
}
