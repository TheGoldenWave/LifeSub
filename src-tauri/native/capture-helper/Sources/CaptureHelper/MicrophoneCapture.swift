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
    private let failureHandler: @Sendable (CaptureFailure) -> Void
    private let signalHandler: @Sendable (CaptureSourceSignal) -> Void
    private let running = OSAllocatedUnfairLock(initialState: false)
    private var configurationObserver: NSObjectProtocol?

    init(
        encoder: ChunkEncoder,
        failureHandler: @escaping @Sendable (CaptureFailure) -> Void,
        signalHandler: @escaping @Sendable (CaptureSourceSignal) -> Void
    ) {
        self.encoder = encoder
        self.failureHandler = failureHandler
        self.signalHandler = signalHandler
        frames = BoundedAudioFrameQueue(
            label: "com.lifesub.capture.microphone",
            consumer: { frame in
                try await encoder.append(samples: frame.samples, hostTime: frame.hostTime)
            },
            failureHandler: failureHandler
        )
        configurationObserver = NotificationCenter.default.addObserver(
            forName: .AVAudioEngineConfigurationChange,
            object: engine,
            queue: nil
        ) { [weak self] _ in
            guard let self else { return }
            if AVCaptureDevice.authorizationStatus(for: .audio) == .authorized {
                self.signalHandler(
                    .interrupted(
                        source: .microphone,
                        reason: "device_configuration_changed",
                        recoverable: true
                    )
                )
            } else {
                self.signalHandler(.permissionRevoked(source: .microphone))
            }
        }
    }

    deinit {
        if let configurationObserver { NotificationCenter.default.removeObserver(configurationObserver) }
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
        frames.startAccepting()
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
        await frames.suspendAndDrain()
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
        let samples: [Int16]
        do {
            samples = try resampler.convert(
                floatSamples: mono,
                sampleRate: buffer.format.sampleRate
            )
        } catch {
            failureHandler(.conversionFailed)
            Task { await stop() }
            return
        }
        do {
            try frames.submit(CapturedPcmFrame(samples: samples, hostTime: hostTime))
        } catch {
            Task { await stop() }
        }
    }
}
