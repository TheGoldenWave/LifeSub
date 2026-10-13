import Foundation
import os

final class FirstAudioFrameReadiness: @unchecked Sendable {
    private enum State {
        case pending
        case waiting(CheckedContinuation<UInt64, any Error>)
        case ready(UInt64)
        case cancelled
    }

    private let state = OSAllocatedUnfairLock(initialState: State.pending)

    func report(hostTime: UInt64) {
        let continuation = state.withLock { state -> CheckedContinuation<UInt64, any Error>? in
            switch state {
            case .pending:
                state = .ready(hostTime)
                return nil
            case let .waiting(continuation):
                state = .ready(hostTime)
                return continuation
            case .ready, .cancelled:
                return nil
            }
        }
        continuation?.resume(returning: hostTime)
    }

    func wait(timeout: Duration) async throws -> UInt64 {
        try await withThrowingTaskGroup(of: UInt64.self) { group in
            group.addTask {
                try await self.waitForReport()
            }
            group.addTask {
                try await Task.sleep(for: timeout)
                throw CaptureSourceError.selectedMicrophoneUnavailable
            }
            defer { group.cancelAll() }
            guard let hostTime = try await group.next() else {
                throw CaptureSourceError.selectedMicrophoneUnavailable
            }
            return hostTime
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
                        return .failure(CaptureSourceError.sourceFailed)
                    case let .ready(hostTime):
                        return .success(hostTime)
                    case .cancelled:
                        return .failure(CancellationError())
                    }
                }
                if let immediate {
                    continuation.resume(with: immediate)
                }
            }
        } onCancel: {
            cancel()
        }
    }

    private func cancel() {
        let continuation = state.withLock { state -> CheckedContinuation<UInt64, any Error>? in
            switch state {
            case let .waiting(continuation):
                state = .cancelled
                return continuation
            case .pending:
                state = .cancelled
                return nil
            case .ready, .cancelled:
                return nil
            }
        }
        continuation?.resume(throwing: CancellationError())
    }
}
