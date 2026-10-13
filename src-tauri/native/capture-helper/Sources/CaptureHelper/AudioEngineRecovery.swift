import Foundation
import os

enum AudioEngineRecoveryAction: Equatable, Sendable {
    case attempt(generation: UInt64, reason: String)
    case burstLimit(generation: UInt64, reason: String)
}

final class AudioEngineRecovery: @unchecked Sendable {
    private struct State {
        var generation: UInt64 = 0
        var task: Task<Void, Never>?
        var active = false
        var attemptStarts: [ContinuousClock.Instant] = []
    }

    private let debounce: Duration
    private let burstWindow: Duration
    private let maxAttemptsPerWindow: Int
    private let state = OSAllocatedUnfairLock(initialState: State())

    init(debounce: Duration, burstWindow: Duration, maxAttemptsPerWindow: Int) {
        precondition(maxAttemptsPerWindow > 0)
        self.debounce = debounce
        self.burstWindow = burstWindow
        self.maxAttemptsPerWindow = maxAttemptsPerWindow
    }

    func schedule(
        reason: String,
        operation: @escaping @Sendable (AudioEngineRecoveryAction) async -> Void
    ) {
        let scheduled = state.withLock { state -> (UInt64, Task<Void, Never>?)? in
            guard !state.active else { return nil }
            state.generation &+= 1
            let generation = state.generation
            let previous = state.task
            state.task = nil
            return (generation, previous)
        }
        guard let (generation, previous) = scheduled else { return }
        previous?.cancel()

        let task = Task { [weak self] in
            do {
                try await Task.sleep(for: self?.debounce ?? .zero)
            } catch {
                return
            }
            guard let self, let action = begin(generation: generation, reason: reason) else {
                return
            }
            await operation(action)
            finish(generation: generation)
        }
        let keep = state.withLock { state -> Bool in
            guard state.generation == generation, !state.active else { return false }
            state.task = task
            return true
        }
        if !keep { task.cancel() }
    }

    func cancel() {
        let task = state.withLock { state -> Task<Void, Never>? in
            state.generation &+= 1
            state.active = false
            let task = state.task
            state.task = nil
            return task
        }
        task?.cancel()
    }

    func isCurrent(generation: UInt64) -> Bool {
        state.withLock { $0.generation == generation && $0.task?.isCancelled != true }
    }

    private func begin(generation: UInt64, reason: String) -> AudioEngineRecoveryAction? {
        state.withLock { state in
            guard state.generation == generation, !Task.isCancelled else { return nil }
            state.active = true
            let now = ContinuousClock.now
            state.attemptStarts.removeAll { now - $0 > burstWindow }
            if state.attemptStarts.count >= maxAttemptsPerWindow {
                return .burstLimit(generation: generation, reason: reason)
            }
            state.attemptStarts.append(now)
            return .attempt(generation: generation, reason: reason)
        }
    }

    private func finish(generation: UInt64) {
        state.withLock { state in
            guard state.generation == generation else { return }
            state.active = false
            state.task = nil
        }
    }
}
