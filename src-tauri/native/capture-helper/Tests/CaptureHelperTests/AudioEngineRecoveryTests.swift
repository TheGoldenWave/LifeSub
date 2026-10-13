import Foundation
import Testing
@testable import CaptureHelper

@Test func recoveryCoalescesConfigurationBurstIntoOneAttempt() async {
    let recorder = RecoveryRecorder()
    let recovery = AudioEngineRecovery(
        debounce: .milliseconds(5),
        burstWindow: .seconds(1),
        maxAttemptsPerWindow: 3
    )

    recovery.schedule(reason: "first") { action in await recorder.record(action) }
    recovery.schedule(reason: "second") { action in await recorder.record(action) }
    recovery.schedule(reason: "latest") { action in await recorder.record(action) }
    try? await Task.sleep(for: .milliseconds(30))

    #expect(await recorder.actions == [.attempt(generation: 3, reason: "latest")])
}

@Test func recoveryCancelInvalidatesDelayedGeneration() async {
    let recorder = RecoveryRecorder()
    let recovery = AudioEngineRecovery(
        debounce: .milliseconds(20),
        burstWindow: .seconds(1),
        maxAttemptsPerWindow: 3
    )

    recovery.schedule(reason: "device_configuration_changed") { action in
        await recorder.record(action)
    }
    recovery.cancel()
    try? await Task.sleep(for: .milliseconds(40))

    #expect(await recorder.actions.isEmpty)
}

@Test func recoveryBurstLimitFailsOnceWithoutStartingAnotherAttempt() async {
    let recorder = RecoveryRecorder()
    let recovery = AudioEngineRecovery(
        debounce: .milliseconds(1),
        burstWindow: .seconds(1),
        maxAttemptsPerWindow: 2
    )

    for reason in ["one", "two", "three"] {
        recovery.schedule(reason: reason) { action in await recorder.record(action) }
        try? await Task.sleep(for: .milliseconds(10))
    }

    #expect(
        await recorder.actions == [
            .attempt(generation: 1, reason: "one"),
            .attempt(generation: 2, reason: "two"),
            .burstLimit(generation: 3, reason: "three"),
        ]
    )
}

private actor RecoveryRecorder {
    private(set) var actions: [AudioEngineRecoveryAction] = []
    func record(_ action: AudioEngineRecoveryAction) { actions.append(action) }
}
