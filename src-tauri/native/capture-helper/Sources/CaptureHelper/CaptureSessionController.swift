import CaptureProtocol

enum CaptureSourceError: Error, Equatable, Sendable {
    case permissionDenied
    case permissionRevoked
    case deviceUnavailable
    case sourceFailed
}

protocol CaptureSourceAdapter: Sendable {
    var source: Source { get }
    func preflight() async throws
    func start() async throws
    func pause() async throws
    func resume() async throws
    func stop() async
    func sealForDiscontinuity() async
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
    case sourceInterrupted(source: Source, reason: String, recoverable: Bool)
    case permissionRevoked(source: Source)
}

enum CaptureSourceSignal: Sendable {
    case interrupted(source: Source, reason: String, recoverable: Bool)
    case permissionRevoked(source: Source)
}

protocol CaptureEventSink: Sendable {
    func emit(_ event: CaptureControllerEvent) async
}

private actor NullCaptureEventSink: CaptureEventSink {
    func emit(_: CaptureControllerEvent) async {}
}

actor CaptureSessionController {
    private let sources: [Source: any CaptureSourceAdapter]
    private let eventSink: any CaptureEventSink
    private(set) var state: CaptureControllerState = .idle
    private var enabledSources: Set<Source> = []

    init(
        sources: [any CaptureSourceAdapter],
        eventSink: any CaptureEventSink = NullCaptureEventSink()
    ) {
        self.sources = Dictionary(uniqueKeysWithValues: sources.map { ($0.source, $0) })
        self.eventSink = eventSink
    }

    func start(requested: Set<Source>) async throws {
        guard state == .idle || state == .stopped, !requested.isEmpty else {
            throw CaptureSourceError.sourceFailed
        }
        state = .starting
        let ordered = requested.sorted(by: sourceOrder)
        do {
            for source in ordered {
                guard let adapter = sources[source] else {
                    throw CaptureSourceError.deviceUnavailable
                }
                try await adapter.preflight()
            }

            var started: [any CaptureSourceAdapter] = []
            do {
                for source in ordered {
                    let adapter = sources[source]!
                    try await adapter.start()
                    started.append(adapter)
                }
            } catch {
                for adapter in started.reversed() {
                    await adapter.stop()
                }
                throw error
            }
            enabledSources = requested
            state = .running(enabled: requested)
        } catch {
            enabledSources.removeAll()
            state = .failed
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
        state = .stopping
        for source in enabledSources.sorted(by: sourceOrder) {
            await sources[source]?.stop()
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
        await adapter.stop()
        guard recoverable else {
            enabledSources.remove(source)
            state = .failed
            return
        }
        do {
            try await adapter.preflight()
            try await adapter.start()
        } catch {
            enabledSources.remove(source)
            state = .failed
        }
    }

    func permissionRevoked(_ source: Source) async {
        guard enabledSources.contains(source), let adapter = sources[source] else { return }
        await adapter.sealForDiscontinuity()
        await adapter.stop()
        enabledSources.remove(source)
        state = .failed
        await eventSink.emit(.permissionRevoked(source: source))
    }

    func shutdown() async {
        try? await stop()
    }

    private func stopAfterTransitionFailure() async {
        for source in enabledSources.sorted(by: sourceOrder) {
            await sources[source]?.stop()
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
