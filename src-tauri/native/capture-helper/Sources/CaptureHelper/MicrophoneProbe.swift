@preconcurrency import AVFoundation
import CaptureProtocol
import Foundation
import os

protocol MicrophoneProbing: Sendable {
    func probe(uid: String) async -> MicrophoneProbeMessage
}

final class MicrophoneProbe: MicrophoneProbing, @unchecked Sendable {
    private let engine: any MicrophoneAudioEngine
    private let selectedAudioInput: any SelectedAudioInputManaging
    private let listDevices: @Sendable () throws -> [MicrophoneDevice]
    private let authorize: @Sendable () async -> Bool
    private let firstFrameTimeout: Duration
    private let formatSettleTimeout: Duration

    convenience init() {
        let engine = LiveMicrophoneAudioEngine()
        let catalog = CoreAudioInputDeviceCatalog()
        self.init(
            engine: engine,
            selectedAudioInput: CoreAudioSelectedAudioInputManager(engine: engine, catalog: catalog),
            listDevices: { try AudioInputDevices(catalog: catalog).listMicrophones() },
            authorize: {
                switch AVCaptureDevice.authorizationStatus(for: .audio) {
                case .authorized:
                    return true
                case .notDetermined:
                    return await AVCaptureDevice.requestAccess(for: .audio)
                case .denied, .restricted:
                    return false
                @unknown default:
                    return false
                }
            },
            firstFrameTimeout: .seconds(2),
            formatSettleTimeout: .milliseconds(500)
        )
    }

    init(
        engine: any MicrophoneAudioEngine,
        selectedAudioInput: any SelectedAudioInputManaging,
        listDevices: @escaping @Sendable () throws -> [MicrophoneDevice],
        authorize: @escaping @Sendable () async -> Bool,
        firstFrameTimeout: Duration,
        formatSettleTimeout: Duration
    ) {
        self.engine = engine
        self.selectedAudioInput = selectedAudioInput
        self.listDevices = listDevices
        self.authorize = authorize
        self.firstFrameTimeout = firstFrameTimeout
        self.formatSettleTimeout = formatSettleTimeout
    }

    func probe(uid: String) async -> MicrophoneProbeMessage {
        let clock = ContinuousClock()
        let startedAt = clock.now
        let device: MicrophoneDevice
        do {
            guard let matched = try listDevices().first(where: { $0.uid == uid }) else {
                return result(uid: uid, name: uid, transport: .unknown, elapsedSince: startedAt, outcome: .unavailable)
            }
            device = matched
        } catch {
            return result(uid: uid, name: uid, transport: .unknown, elapsedSince: startedAt, outcome: .unavailable)
        }

        guard await authorize() else {
            return result(for: device, elapsedSince: startedAt, outcome: .permissionDenied)
        }

        let selected: ResolvedSelectedAudioInput
        do {
            selected = try selectedAudioInput.resolveSelectedInput(uid: uid)
        } catch {
            return result(for: device, elapsedSince: startedAt, outcome: .unavailable)
        }

        do {
            try selectedAudioInput.bindSelectedInput(deviceID: selected.deviceID)
        } catch {
            return result(for: device, elapsedSince: startedAt, outcome: .routeConflict)
        }

        let readBack: ResolvedSelectedAudioInput
        do {
            readBack = try selectedAudioInput.currentBoundInput()
        } catch {
            return result(for: device, elapsedSince: startedAt, outcome: .unavailable)
        }
        guard readBack.uid == uid else {
            return result(
                for: device,
                readBackUid: readBack.uid,
                elapsedSince: startedAt,
                outcome: .identityMismatch
            )
        }

        let expected: SelectedAudioInputFormat
        do {
            expected = try selectedAudioInput.expectedInputFormat(deviceID: selected.deviceID)
        } catch {
            return result(for: device, readBackUid: readBack.uid, elapsedSince: startedAt, outcome: .invalidFormat)
        }
        guard expected.sampleRate.isFinite,
              expected.sampleRate > 0,
              expected.sampleRate <= Double(UInt32.max),
              expected.channelCount > 0
        else {
            return result(for: device, readBackUid: readBack.uid, elapsedSince: startedAt, outcome: .invalidFormat)
        }

        let inputNode = engine.inputNode
        let format: AVAudioFormat
        do {
            format = try await settledMicrophoneInputFormat(
                inputNode: inputNode,
                expected: expected,
                timeout: formatSettleTimeout
            )
        } catch {
            return result(
                for: device,
                sampleRate: UInt32(expected.sampleRate.rounded()),
                channelCount: UInt16(expected.channelCount),
                readBackUid: readBack.uid,
                elapsedSince: startedAt,
                outcome: .routeConflict
            )
        }

        let readiness = ProbeFrameReadiness()
        var tapInstalled = false
        var engineStarted = false
        defer {
            if engineStarted { engine.stop() }
            if tapInstalled { inputNode.removeTap(onBus: 0) }
        }
        inputNode.installTap(onBus: 0, bufferSize: 1024, format: format) { buffer, _ in
            guard buffer.frameLength > 0,
                  buffer.format.channelCount > 0,
                  buffer.floatChannelData != nil
            else { return }
            readiness.report(sampleCount: UInt64(buffer.frameLength))
        }
        tapInstalled = true
        do {
            try engine.start()
            engineStarted = true
        } catch {
            return result(
                for: device,
                sampleRate: UInt32(expected.sampleRate.rounded()),
                channelCount: UInt16(expected.channelCount),
                readBackUid: readBack.uid,
                elapsedSince: startedAt,
                outcome: .routeConflict
            )
        }

        do {
            let sampleCount = try await readiness.wait(timeout: firstFrameTimeout)
            return result(
                for: device,
                sampleRate: UInt32(expected.sampleRate.rounded()),
                channelCount: UInt16(expected.channelCount),
                readBackUid: readBack.uid,
                sampleCount: sampleCount,
                elapsedSince: startedAt,
                outcome: .compatible
            )
        } catch {
            return result(
                for: device,
                sampleRate: UInt32(expected.sampleRate.rounded()),
                channelCount: UInt16(expected.channelCount),
                readBackUid: readBack.uid,
                elapsedSince: startedAt,
                outcome: .noAudioFrames
            )
        }
    }

    private func result(
        for device: MicrophoneDevice,
        sampleRate: UInt32? = nil,
        channelCount: UInt16? = nil,
        readBackUid: String? = nil,
        sampleCount: UInt64 = 0,
        elapsedSince start: ContinuousClock.Instant,
        outcome: MicrophoneProbeResult
    ) -> MicrophoneProbeMessage {
        result(
            uid: device.uid,
            name: device.name,
            transport: device.transport,
            sampleRate: sampleRate,
            channelCount: channelCount,
            readBackUid: readBackUid,
            sampleCount: sampleCount,
            elapsedSince: start,
            outcome: outcome
        )
    }

    private func result(
        uid: String,
        name: String,
        transport: MicrophoneTransport,
        sampleRate: UInt32? = nil,
        channelCount: UInt16? = nil,
        readBackUid: String? = nil,
        sampleCount: UInt64 = 0,
        elapsedSince start: ContinuousClock.Instant,
        outcome: MicrophoneProbeResult
    ) -> MicrophoneProbeMessage {
        MicrophoneProbeMessage(
            uid: uid,
            name: name,
            transport: transport,
            inputSampleRateHz: sampleRate,
            inputChannelCount: channelCount,
            readBackUid: readBackUid,
            firstFrameSampleCount: sampleCount,
            elapsedMs: elapsedMilliseconds(ContinuousClock.now - start),
            result: outcome
        )
    }
}

private final class ProbeFrameReadiness: @unchecked Sendable {
    private enum State {
        case pending
        case waiting(CheckedContinuation<UInt64, any Error>)
        case ready(UInt64)
        case cancelled
    }

    private let state = OSAllocatedUnfairLock(initialState: State.pending)

    func report(sampleCount: UInt64) {
        let continuation = state.withLock { state -> CheckedContinuation<UInt64, any Error>? in
            switch state {
            case .pending:
                state = .ready(sampleCount)
                return nil
            case let .waiting(continuation):
                state = .ready(sampleCount)
                return continuation
            case .ready, .cancelled:
                return nil
            }
        }
        continuation?.resume(returning: sampleCount)
    }

    func wait(timeout: Duration) async throws -> UInt64 {
        try await withThrowingTaskGroup(of: UInt64.self) { group in
            group.addTask { try await self.waitForReport() }
            group.addTask {
                try await Task.sleep(for: timeout)
                throw MicrophoneInputFormatSettleError.timeout
            }
            defer { group.cancelAll() }
            guard let count = try await group.next() else {
                throw MicrophoneInputFormatSettleError.timeout
            }
            return count
        }
    }

    private func waitForReport() async throws -> UInt64 {
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                let immediate = state.withLock { state -> Result<UInt64, any Error>? in
                    switch state {
                    case .pending:
                        state = .waiting(continuation)
                        return nil
                    case .waiting:
                        return .failure(MicrophoneInputFormatSettleError.timeout)
                    case let .ready(value):
                        return .success(value)
                    case .cancelled:
                        return .failure(CancellationError())
                    }
                }
                if let immediate { continuation.resume(with: immediate) }
            }
        } onCancel: {
            let continuation = state.withLock { state -> CheckedContinuation<UInt64, any Error>? in
                guard case let .waiting(continuation) = state else {
                    state = .cancelled
                    return nil
                }
                state = .cancelled
                return continuation
            }
            continuation?.resume(throwing: CancellationError())
        }
    }
}

private func elapsedMilliseconds(_ duration: Duration) -> UInt64 {
    let components = duration.components
    let milliseconds = components.seconds * 1_000 + components.attoseconds / 1_000_000_000_000_000
    return UInt64(max(0, milliseconds))
}
