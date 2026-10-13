@preconcurrency import AVFoundation
import CaptureProtocol
import Foundation
import os

final class MicrophoneCapture: @unchecked Sendable, CaptureSourceAdapter {
    let source: Source = .microphone

    private static let observedChangeCoalescingDelay = Duration.milliseconds(10)
    private static let inputFormatSettlePollInterval = Duration.milliseconds(10)
    private static let inputFormatSettleTimeout = Duration.milliseconds(500)
    private static let firstFrameReadinessTimeout = Duration.seconds(2)
    private static let recoveryDebounce = Duration.milliseconds(20)
    private static let recoveryBurstWindow = Duration.seconds(2)
    private static let recoveryBurstLimit = 3

    private struct RuntimeState {
        var isRunning = false
        var isStarting = false
        var isPublished = false
        var terminalSignalDelivered = false
        var selectedUID: String?
        var activeInput: ResolvedSelectedAudioInput?
        var startHostTime: UInt64?
        var observers: [any SelectedAudioInputObserver] = []
        var enginePathActive = false
    }

    private let engine: any MicrophoneAudioEngine
    private let selectedAudioInput: any SelectedAudioInputManaging
    private let encoder: ChunkEncoder
    private let frames: BoundedAudioFrameQueue
    private let resampler = MonoPcm16Resampler()
    private let failureHandler: @Sendable (CaptureFailure) -> Void
    private let signalHandler: @Sendable (CaptureSourceSignal) async -> Void
    private let firstFrameTimeout: Duration
    private let runtime = OSAllocatedUnfairLock(initialState: RuntimeState())
    private let recovery = AudioEngineRecovery(
        debounce: MicrophoneCapture.recoveryDebounce,
        burstWindow: MicrophoneCapture.recoveryBurstWindow,
        maxAttemptsPerWindow: MicrophoneCapture.recoveryBurstLimit
    )

    init(
        encoder: ChunkEncoder,
        failureHandler: @escaping @Sendable (CaptureFailure) -> Void,
        signalHandler: @escaping @Sendable (CaptureSourceSignal) async -> Void
    ) {
        let engine = LiveMicrophoneAudioEngine()
        self.encoder = encoder
        self.failureHandler = failureHandler
        self.signalHandler = signalHandler
        firstFrameTimeout = Self.firstFrameReadinessTimeout
        self.engine = engine
        selectedAudioInput = CoreAudioSelectedAudioInputManager(engine: engine)
        frames = BoundedAudioFrameQueue(
            label: "com.lifesub.capture.microphone",
            consumer: { frame in
                try await encoder.append(samples: frame.samples, hostTime: frame.hostTime)
            },
            failureHandler: failureHandler
        )
    }

    init(
        encoder: ChunkEncoder,
        failureHandler: @escaping @Sendable (CaptureFailure) -> Void,
        signalHandler: @escaping @Sendable (CaptureSourceSignal) async -> Void,
        engine: any MicrophoneAudioEngine,
        selectedAudioInput: any SelectedAudioInputManaging,
        firstFrameTimeout: Duration
    ) {
        self.encoder = encoder
        self.failureHandler = failureHandler
        self.signalHandler = signalHandler
        self.engine = engine
        self.selectedAudioInput = selectedAudioInput
        self.firstFrameTimeout = firstFrameTimeout
        frames = BoundedAudioFrameQueue(
            label: "com.lifesub.capture.microphone",
            consumer: { frame in
                try await encoder.append(samples: frame.samples, hostTime: frame.hostTime)
            },
            failureHandler: failureHandler
        )
    }

    deinit {
        recovery.cancel()
        let observers = runtime.withLock { state in
            let tokens = state.observers
            state.observers = []
            return tokens
        }
        observers.forEach { $0.cancel() }
    }

    func preflight() async throws {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized: return
        case .notDetermined:
            guard await AVCaptureDevice.requestAccess(for: .audio) else {
                throw CaptureSourceError.permissionDenied(source: .microphone)
            }
        case .denied, .restricted:
            throw CaptureSourceError.permissionDenied(source: .microphone)
        @unknown default:
            throw CaptureSourceError.permissionDenied(source: .microphone)
        }
    }

    func metadata() async throws -> CaptureSourceMetadata {
        let snapshot = runtime.withLock { ($0.activeInput, $0.startHostTime) }
        guard let activeInput = snapshot.0, let startHostTime = snapshot.1 else {
            throw CaptureSourceError.deviceUnavailable
        }
        return CaptureSourceMetadata(
            deviceId: activeInput.uid,
            deviceName: activeInput.name,
            sampleRate: 16_000,
            channelCount: 1,
            hostTime: startHostTime
        )
    }

    func start(selectedMicrophoneUID: String?) async throws {
        guard let selectedMicrophoneUID else {
            throw CaptureSourceError.selectedMicrophoneUnavailable
        }
        guard runtime.withLock({ state in
            if state.isRunning { return false }
            state.isRunning = true
            state.isStarting = true
            state.isPublished = false
            state.terminalSignalDelivered = false
            state.selectedUID = selectedMicrophoneUID
            return true
        }) else { return }

        do {
            let path = try await configureAndStartEngine(selectedMicrophoneUID: selectedMicrophoneUID)
            runtime.withLock { state in
                state.activeInput = path.input
                state.startHostTime = path.firstHostTime
                state.observers = path.observers
                state.enginePathActive = true
                state.isStarting = false
                state.isPublished = false
            }
        } catch {
            runtime.withLock { state in
                state.isRunning = false
                state.isStarting = false
                state.selectedUID = nil
                state.activeInput = nil
                state.startHostTime = nil
                state.observers = []
                state.enginePathActive = false
            }
            throw error
        }
    }

    func activate() async {
        runtime.withLock { state in
            if state.isRunning { state.isPublished = true }
        }
    }

    func pause() async throws {
        engine.pause()
    }

    func resume() async throws {
        do {
            try engine.start()
        } catch {
            throw CaptureSourceError.sourceFailed
        }
    }

    func stop() async -> CaptureSourceStopMetadata {
        recovery.cancel()
        let stopPayload = runtime.withLock { state -> (observers: [any SelectedAudioInputObserver], running: Bool, pathActive: Bool) in
            let payload = (state.observers, state.isRunning, state.enginePathActive)
            state.isRunning = false
            state.isStarting = false
            state.isPublished = false
            state.terminalSignalDelivered = false
            state.selectedUID = nil
            state.activeInput = nil
            state.startHostTime = nil
            state.observers = []
            state.enginePathActive = false
            return payload
        }
        guard stopPayload.running else { return await encoder.stopMetadata() }
        stopPayload.observers.forEach { $0.cancel() }
        if stopPayload.pathActive {
            engine.stop()
            engine.inputNode.removeTap(onBus: 0)
        }
        await frames.suspendAndDrain()
        try? await encoder.sealForDiscontinuity()
        return await encoder.stopMetadata()
    }

    func sealForDiscontinuity() async {
        try? await encoder.sealForDiscontinuity()
    }

    private struct EnginePath {
        let input: ResolvedSelectedAudioInput
        let firstHostTime: UInt64
        let observers: [any SelectedAudioInputObserver]
    }

    private func handleObservedChange() {
        recovery.schedule(reason: "device_configuration_changed") { [weak self] action in
            await self?.handleRecovery(action)
        }
    }

    private func handleRecovery(_ action: AudioEngineRecoveryAction) async {
        switch action {
        case let .burstLimit(generation, _):
            guard recovery.isCurrent(generation: generation) else { return }
            await failRecovery(reason: "audio_engine_recovery_burst_limit")
        case let .attempt(generation, reason):
            guard runtime.withLock({ !$0.isStarting && $0.isPublished }) else { return }
            let selectedUID = runtime.withLock { state -> String? in
                guard state.isRunning, !state.terminalSignalDelivered else { return nil }
                return state.selectedUID
            }
            guard let selectedUID, recovery.isCurrent(generation: generation) else { return }

            await signalHandler(
                .recovering(source: .microphone, generation: generation, reason: reason)
            )

            let observers = runtime.withLock { state -> [any SelectedAudioInputObserver] in
                let observers = state.observers
                state.observers = []
                state.activeInput = nil
                state.startHostTime = nil
                state.enginePathActive = false
                return observers
            }
            observers.forEach { $0.cancel() }
            engine.stop()
            engine.inputNode.removeTap(onBus: 0)
            await frames.suspendAndDrain()
            do {
                try await encoder.sealForDiscontinuity()
            } catch {
                await failRecovery(reason: "audio_engine_recovery_seal_failed")
                return
            }

            guard recovery.isCurrent(generation: generation),
                  runtime.withLock({ $0.isRunning && $0.selectedUID == selectedUID })
            else { return }

            do {
                let path = try await configureAndStartEngine(selectedMicrophoneUID: selectedUID)
                guard recovery.isCurrent(generation: generation),
                      runtime.withLock({ $0.isRunning && $0.selectedUID == selectedUID })
                else {
                    discard(path: path)
                    await frames.suspendAndDrain()
                    return
                }
                runtime.withLock { state in
                    state.activeInput = path.input
                    state.startHostTime = path.firstHostTime
                    state.observers = path.observers
                    state.enginePathActive = true
                }
                await signalHandler(
                    .recovered(
                        source: .microphone,
                        generation: generation,
                        deviceId: path.input.uid,
                        firstHostTime: path.firstHostTime
                    )
                )
            } catch let error as CaptureSourceError {
                let failureReason = switch error {
                case .selectedMicrophoneUnavailable, .selectedMicrophoneIdentityMismatch,
                     .selectedMicrophoneDisconnected, .deviceUnavailable:
                    "selected_microphone_disconnected"
                default:
                    "audio_engine_recovery_failed"
                }
                await failRecovery(reason: failureReason)
            } catch {
                await failRecovery(reason: "audio_engine_recovery_failed")
            }
        }
    }

    private func configureAndStartEngine(selectedMicrophoneUID: String) async throws -> EnginePath {
        var inputNode: (any MicrophoneAudioInputNode)?
        var observers: [any SelectedAudioInputObserver] = []
        var tapInstalled = false
        var engineStarted = false
        frames.startAccepting()
        do {
            let selected = try mapSelectedInputError {
                try selectedAudioInput.resolveSelectedInput(uid: selectedMicrophoneUID)
            }
            try mapSelectedInputError {
                try selectedAudioInput.bindSelectedInput(deviceID: selected.deviceID)
            }
            let current = try mapSelectedInputError {
                try selectedAudioInput.currentBoundInput()
            }
            guard current.uid == selected.uid, current.deviceID == selected.deviceID else {
                throw CaptureSourceError.selectedMicrophoneIdentityMismatch
            }
            let expectedFormat = try mapSelectedInputError {
                try selectedAudioInput.expectedInputFormat(deviceID: selected.deviceID)
            }
            let activeInputNode = engine.inputNode
            inputNode = activeInputNode
            let format: AVAudioFormat
            do {
                format = try await settledMicrophoneInputFormat(
                    inputNode: activeInputNode,
                    expected: expectedFormat,
                    timeout: Self.inputFormatSettleTimeout,
                    pollInterval: Self.inputFormatSettlePollInterval
                )
            } catch is MicrophoneInputFormatSettleError {
                throw CaptureSourceError.selectedMicrophoneUnavailable
            }

            let changeHandler: @Sendable () -> Void = { [weak self] in
                self?.handleObservedChange()
            }
            observers = try mapSelectedInputError {
                try selectedAudioInput.addChangeObservers(
                    selectedDeviceID: selected.deviceID,
                    handler: changeHandler
                )
            }
            observers.append(engine.addConfigurationChangeObserver(changeHandler))

            let firstFrameReadiness = FirstAudioFrameReadiness()
            activeInputNode.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self] buffer, time in
                let hostTime = CaptureContinuousClock.nanoseconds(fromMachTicks: time.hostTime)
                if self?.consume(buffer: buffer, hostTime: hostTime) == true {
                    firstFrameReadiness.report(hostTime: hostTime)
                }
            }
            tapInstalled = true
            do {
                try engine.start()
                engineStarted = true
            } catch {
                throw CaptureSourceError.sourceFailed
            }
            let firstHostTime = try await firstFrameReadiness.wait(timeout: firstFrameTimeout)
            return EnginePath(input: current, firstHostTime: firstHostTime, observers: observers)
        } catch {
            observers.forEach { $0.cancel() }
            if engineStarted { engine.stop() }
            if tapInstalled { inputNode?.removeTap(onBus: 0) }
            await frames.suspendAndDrain()
            throw error
        }
    }

    private func discard(path: EnginePath) {
        path.observers.forEach { $0.cancel() }
        engine.stop()
        engine.inputNode.removeTap(onBus: 0)
    }

    private func failRecovery(reason: String) async {
        let shouldStop = runtime.withLock { state in
            guard state.isRunning, !state.terminalSignalDelivered else { return false }
            state.terminalSignalDelivered = true
            return true
        }
        guard shouldStop else { return }
        await signalHandler(
            .interrupted(source: .microphone, reason: reason, recoverable: false)
        )
        _ = await stop()
    }

    private func mapSelectedInputError<T>(_ body: () throws -> T) throws -> T {
        do {
            return try body()
        } catch let error as SelectedAudioInputError {
            switch error {
            case .unavailable:
                throw CaptureSourceError.selectedMicrophoneUnavailable
            case .identityMismatch:
                throw CaptureSourceError.selectedMicrophoneIdentityMismatch
            case .listenerRegistrationFailed:
                throw CaptureSourceError.sourceFailed
            }
        } catch let error as CaptureSourceError {
            throw error
        } catch {
            throw CaptureSourceError.sourceFailed
        }
    }

    private func consume(buffer: AVAudioPCMBuffer, hostTime: UInt64) -> Bool {
        guard let channels = buffer.floatChannelData else { return false }
        let channelCount = Int(buffer.format.channelCount)
        let frameCount = Int(buffer.frameLength)
        guard channelCount > 0, frameCount > 0 else { return false }
        var mono = [Float](repeating: 0, count: frameCount)
        for frame in 0 ..< frameCount {
            var mixed: Float = 0
            for channel in 0 ..< channelCount {
                mixed += channels[channel][frame]
            }
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
            Task {
                _ = await stop()
            }
            return false
        }
        do {
            try frames.submit(CapturedPcmFrame(samples: samples, hostTime: hostTime))
            return !samples.isEmpty
        } catch {
            Task {
                _ = await stop()
            }
            return false
        }
    }
}
