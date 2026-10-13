import CaptureProtocol
import Foundation

enum CaptureSourceError: Error, Equatable, Sendable {
    case permissionDenied(source: Source)
    case permissionRevoked
    case deviceUnavailable
    case selectedMicrophoneUnavailable
    case selectedMicrophoneIdentityMismatch
    case selectedMicrophoneDisconnected
    case sourceFailed
}

protocol CaptureSourceAdapter: Sendable {
    var source: Source { get }
    func metadata() async throws -> CaptureSourceMetadata
    func preflight() async throws
    func start(selectedMicrophoneUID: String?) async throws
    func activate() async
    func pause() async throws
    func resume() async throws
    func stop() async -> CaptureSourceStopMetadata
    func sealForDiscontinuity() async
}

extension CaptureSourceAdapter {
    func activate() async {}
}

struct CaptureSourceStopMetadata: Equatable, Sendable {
    let finalSequence: UInt64
    let startHostTime: UInt64
    let endHostTime: UInt64
}

struct CaptureSourceMetadata: Equatable, Sendable {
    let deviceId: String
    let deviceName: String
    let sampleRate: UInt32
    let channelCount: UInt16
    let hostTime: UInt64
}

enum CaptureControllerState: Equatable, Sendable {
    case idle
    case starting
    case running(enabled: Set<Source>)
    case paused(enabled: Set<Source>)
    case stopping
    case stopped
    case failed
}

enum CaptureControllerEvent: Equatable, Sendable {
    case permissionState(microphone: Permission, screenRecording: Permission)
    case sourceStarted(source: Source, deviceId: String, deviceName: String, sampleRate: UInt32, channelCount: UInt16, hostTime: UInt64)
    case sourceStopped(source: Source, finalSequence: UInt64, startHostTime: UInt64, endHostTime: UInt64)
    case chunkSealed(source: Source, relativePath: String, byteLength: UInt64, sha256: String, durationMs: UInt64, startSamplePosition: UInt64, endSamplePosition: UInt64, startHostTime: UInt64, endHostTime: UInt64)
    case fatalError(code: String, summary: String)
    case sourceInterrupted(source: Source, reason: String, recoverable: Bool)
    case sourceRecovering(source: Source, generation: UInt64, reason: String)
    case sourceRecovered(source: Source, generation: UInt64, deviceId: String, firstHostTime: UInt64)
    case permissionRevoked(source: Source)
}

protocol HelperCaptureSessionControlling: Sendable {
    func start(requested: Set<Source>, selectedMicrophoneUID: String?) async throws
    func pause() async throws
    func resume() async throws
    func stop() async throws
    func shutdown() async
}

protocol CaptureSessionControllerFactory: Sendable {
    func makeController(
        stagingRoot: URL,
        eventSink: any CaptureEventSink,
        selectedMicrophoneUID: String?
    ) async throws -> any HelperCaptureSessionControlling
}

enum CaptureSourceSignal: Sendable {
    case interrupted(source: Source, reason: String, recoverable: Bool)
    case permissionRevoked(source: Source)
    case recovering(source: Source, generation: UInt64, reason: String)
    case recovered(source: Source, generation: UInt64, deviceId: String, firstHostTime: UInt64)
}

protocol CaptureEventSink: Sendable {
    func emit(_ event: CaptureControllerEvent) async
}

private actor NullCaptureEventSink: CaptureEventSink {
    func emit(_: CaptureControllerEvent) async {}
}

actor CaptureSessionController: HelperCaptureSessionControlling {
    private let sources: [Source: any CaptureSourceAdapter]
    private let eventSink: any CaptureEventSink
    private(set) var state: CaptureControllerState = .idle
    private var enabledSources: Set<Source> = []
    private var startupGeneration: UInt64 = 0

    init(
        sources: [any CaptureSourceAdapter],
        eventSink: any CaptureEventSink = NullCaptureEventSink()
    ) {
        self.sources = Dictionary(uniqueKeysWithValues: sources.map { ($0.source, $0) })
        self.eventSink = eventSink
    }

    func start(requested: Set<Source>, selectedMicrophoneUID: String?) async throws {
        guard state == .idle || state == .stopped, !requested.isEmpty else {
            throw CaptureSourceError.sourceFailed
        }
        state = .starting
        startupGeneration &+= 1
        let generation = startupGeneration
        let ordered = requested.sorted(by: sourceOrder)
        var started: [any CaptureSourceAdapter] = []
        do {
            for source in ordered {
                guard let adapter = sources[source] else {
                    throw CaptureSourceError.deviceUnavailable
                }
                try await adapter.preflight()
                guard startupGeneration == generation else {
                    throw CaptureSourceError.sourceFailed
                }
            }

            for source in ordered {
                let adapter = sources[source]!
                try await adapter.start(
                    selectedMicrophoneUID: source == .microphone ? selectedMicrophoneUID : nil
                )
                started.append(adapter)
                guard startupGeneration == generation else {
                    throw CaptureSourceError.sourceFailed
                }
            }
            enabledSources = requested
            state = .running(enabled: requested)
            await eventSink.emit(
                .permissionState(
                    microphone: requested.contains(.microphone) ? .granted : .notDetermined,
                    screenRecording: requested.contains(.systemAudio) ? .granted : .notDetermined
                )
            )
            guard startupGeneration == generation else {
                throw CaptureSourceError.sourceFailed
            }
            for source in ordered {
                let metadata = try await sources[source]!.metadata()
                guard startupGeneration == generation else {
                    throw CaptureSourceError.sourceFailed
                }
                await eventSink.emit(.sourceStarted(source: source, deviceId: metadata.deviceId, deviceName: metadata.deviceName, sampleRate: metadata.sampleRate, channelCount: metadata.channelCount, hostTime: metadata.hostTime))
                guard startupGeneration == generation else {
                    throw CaptureSourceError.sourceFailed
                }
            }
            for source in ordered {
                await sources[source]!.activate()
                guard startupGeneration == generation else {
                    throw CaptureSourceError.sourceFailed
                }
            }
        } catch {
            for adapter in started.reversed() {
                _ = await adapter.stop()
            }
            enabledSources.removeAll()
            if state != .stopped {
                state = .failed
            }
            throw error
        }
    }

    func pause() async throws {
        guard case let .running(enabled) = state else {
            throw CaptureSourceError.sourceFailed
        }
        do {
            for source in enabled.sorted(by: sourceOrder) {
                try await sources[source]?.pause()
            }
        } catch {
            await stopAfterTransitionFailure()
            throw error
        }
        state = .paused(enabled: enabled)
    }

    func resume() async throws {
        guard case let .paused(enabled) = state else {
            throw CaptureSourceError.sourceFailed
        }
        do {
            for source in enabled.sorted(by: sourceOrder) {
                try await sources[source]?.resume()
            }
        } catch {
            await stopAfterTransitionFailure()
            throw error
        }
        state = .running(enabled: enabled)
    }

    func stop() async throws {
        guard state != .stopped else { return }
        startupGeneration &+= 1
        state = .stopping
        for source in enabledSources.sorted(by: sourceOrder) {
            if let metadata = await sources[source]?.stop() {
                await eventSink.emit(.sourceStopped(source: source, finalSequence: metadata.finalSequence, startHostTime: metadata.startHostTime, endHostTime: metadata.endHostTime))
            }
        }
        enabledSources.removeAll()
        state = .stopped
    }

    func sourceInterrupted(_ source: Source, reason: String, recoverable: Bool) async {
        guard enabledSources.contains(source), let adapter = sources[source] else { return }
        await adapter.sealForDiscontinuity()
        await eventSink.emit(
            .sourceInterrupted(source: source, reason: reason, recoverable: recoverable)
        )
        guard recoverable else {
            _ = await adapter.stop()
            enabledSources.remove(source)
            state = .failed
            return
        }
    }

    func sourceRecovering(_ source: Source, generation: UInt64, reason: String) async {
        guard enabledSources.contains(source) else { return }
        await eventSink.emit(
            .sourceRecovering(source: source, generation: generation, reason: reason)
        )
    }

    func sourceRecovered(
        _ source: Source,
        generation: UInt64,
        deviceId: String,
        firstHostTime: UInt64
    ) async {
        guard enabledSources.contains(source) else { return }
        await eventSink.emit(
            .sourceRecovered(
                source: source,
                generation: generation,
                deviceId: deviceId,
                firstHostTime: firstHostTime
            )
        )
    }

    func permissionRevoked(_ source: Source) async {
        guard enabledSources.contains(source), let adapter = sources[source] else { return }
        await adapter.sealForDiscontinuity()
        _ = await adapter.stop()
        enabledSources.remove(source)
        state = .failed
        await eventSink.emit(.permissionRevoked(source: source))
    }

    func shutdown() async {
        try? await stop()
    }

    private func stopAfterTransitionFailure() async {
        for source in enabledSources.sorted(by: sourceOrder) {
            _ = await sources[source]?.stop()
        }
        enabledSources.removeAll()
        state = .failed
    }
}

private func sourceOrder(_ lhs: Source, _ rhs: Source) -> Bool {
    func rank(_ source: Source) -> Int {
        switch source {
        case .microphone: 0
        case .systemAudio: 1
        }
    }
    return rank(lhs) < rank(rhs)
}
