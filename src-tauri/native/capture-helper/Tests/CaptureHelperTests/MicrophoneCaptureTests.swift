@preconcurrency import AVFoundation
import CoreAudio
import Foundation
import Testing
@testable import CaptureHelper

@Test func selectedMicrophoneIsBoundBeforeTapAndEngineStart() async throws {
    let harness = try MicrophoneCaptureHarness()
    let capture = harness.makeCapture()

    try await capture.start(selectedMicrophoneUID: "mic-1")
    let metadata = try await capture.metadata()

    #expect(
        harness.operations == [
            .resolve("mic-1"),
            .bind(101),
            .current,
            .inputNode,
            .hardwareInputFormat,
            .observe(101),
            .installTap,
            .startEngine,
        ]
    )
    #expect(metadata.deviceId == "mic-1")
    #expect(metadata.deviceName == "Studio Mic")
}

@Test func sourceStartedMetadataUsesOneStablePreCaptureHostAnchor() async throws {
    let firstFrameMachTicks: UInt64 = 987_654
    let harness = try MicrophoneCaptureHarness(firstFrameMachTicks: firstFrameMachTicks)
    let capture = harness.makeCapture()

    try await capture.start(selectedMicrophoneUID: "mic-1")
    let first = try await capture.metadata()
    try await Task.sleep(for: .milliseconds(2))
    let second = try await capture.metadata()

    #expect(first.hostTime == second.hostTime)
    #expect(first.hostTime == CaptureContinuousClock.nanoseconds(fromMachTicks: firstFrameMachTicks))
}

@Test func startDoesNotSucceedUntilTheSelectedMicrophoneDeliversAFrame() async throws {
    let harness = try MicrophoneCaptureHarness(firstFrameMachTicks: nil)
    let capture = harness.makeCapture()
    let completed = Locked(false)

    let startTask = Task {
        try await capture.start(selectedMicrophoneUID: "mic-1")
        completed.withValue { $0 = true }
    }

    try await Task.sleep(for: .milliseconds(20))
    #expect(completed.value == false)

    let firstFrameMachTicks: UInt64 = 456_789
    try harness.inputNode.emitFrame(hostTime: firstFrameMachTicks)
    try await startTask.value

    #expect(completed.value)
    let metadata = try await capture.metadata()
    #expect(metadata.hostTime == CaptureContinuousClock.nanoseconds(fromMachTicks: firstFrameMachTicks))
}

@Test func firstFrameTimeoutFailsSelectedMicrophoneAndCleansUpCapture() async throws {
    let harness = try MicrophoneCaptureHarness(
        firstFrameMachTicks: nil,
        firstFrameTimeout: .milliseconds(20)
    )
    let capture = harness.makeCapture()

    await #expect(throws: CaptureSourceError.selectedMicrophoneUnavailable) {
        try await capture.start(selectedMicrophoneUID: "mic-1")
    }

    #expect(harness.engine.stopCount == 1)
    #expect(harness.inputNode.removeTapCount == 1)
    await #expect(throws: CaptureSourceError.deviceUnavailable) {
        _ = try await capture.metadata()
    }
}

@Test func waitsForSelectedMicrophoneFormatToSettleBeforeInstallingTap() async throws {
    let staleFormat = try #require(
        AVAudioFormat(standardFormatWithSampleRate: 48_000, channels: 1)
    )
    let selectedFormat = try #require(
        AVAudioFormat(standardFormatWithSampleRate: 44_100, channels: 1)
    )
    let harness = try MicrophoneCaptureHarness(
        inputFormats: [staleFormat, selectedFormat],
        expectedInputFormat: SelectedAudioInputFormat(sampleRate: 44_100, channelCount: 1)
    )
    let capture = harness.makeCapture()

    try await capture.start(selectedMicrophoneUID: "mic-1")

    #expect(harness.inputNode.installedTapSampleRate == 44_100)
    #expect(harness.operations.filter { $0 == .hardwareInputFormat }.count == 2)
}

@Test func formatSettleTimeoutUsesSelectedMicrophoneUnavailableError() async throws {
    let staleFormat = try #require(
        AVAudioFormat(standardFormatWithSampleRate: 48_000, channels: 1)
    )
    let harness = try MicrophoneCaptureHarness(
        inputFormats: [staleFormat],
        expectedInputFormat: SelectedAudioInputFormat(sampleRate: 44_100, channelCount: 1)
    )
    let capture = harness.makeCapture()

    await #expect(throws: CaptureSourceError.selectedMicrophoneUnavailable) {
        try await capture.start(selectedMicrophoneUID: "mic-1")
    }
    #expect(harness.inputNode.installedTapSampleRate == nil)
}

@Test func startFailsWhenSelectedMicrophoneIsUnavailable() async throws {
    let harness = try MicrophoneCaptureHarness(
        resolvedInputs: [:]
    )
    let capture = harness.makeCapture()

    await #expect(throws: CaptureSourceError.selectedMicrophoneUnavailable) {
        try await capture.start(selectedMicrophoneUID: "missing")
    }
    #expect(harness.operations == [.resolve("missing")])
}

@Test func startFailsClosedWhenAudioUnitReadbackDoesNotMatchRequestedDevice() async throws {
    let harness = try MicrophoneCaptureHarness(
        resolvedInputs: [
            "mic-1": ResolvedSelectedAudioInput(deviceID: 101, uid: "mic-1", name: "Studio Mic"),
            "mic-2": ResolvedSelectedAudioInput(deviceID: 202, uid: "mic-2", name: "USB Mic"),
        ],
        currentResolvedInput: ResolvedSelectedAudioInput(deviceID: 202, uid: "mic-2", name: "USB Mic")
    )
    let capture = harness.makeCapture()

    await #expect(throws: CaptureSourceError.selectedMicrophoneIdentityMismatch) {
        try await capture.start(selectedMicrophoneUID: "mic-1")
    }
    #expect(harness.operations == [.resolve("mic-1"), .bind(101), .current])
}

@Test func sameSelectedUIDConfigurationChangeRebuildsEngineAndRequiresANewFrame() async throws {
    let harness = try MicrophoneCaptureHarness()
    let capture = harness.makeCapture()

    try await capture.start(selectedMicrophoneUID: "mic-1")
    await capture.activate()
    harness.emitObservedChange()

    try await waitForMicrophoneCondition { harness.signalEvents.count >= 2 }

    #expect(
        harness.signalEvents == [
            "recovering:microphone:2:device_configuration_changed",
            "recovered:microphone:2:mic-1",
        ]
    )
    #expect(harness.operations.filter { $0 == .startEngine }.count == 2)
    #expect(harness.engine.stopCount == 1)
    #expect(harness.inputNode.removeTapCount == 1)
}

@Test func selectedMicrophoneDisconnectEmitsSingleNonrecoverableSignalAndStops() async throws {
    let harness = try MicrophoneCaptureHarness()
    let capture = harness.makeCapture()

    try await capture.start(selectedMicrophoneUID: "mic-1")
    await capture.activate()
    harness.selectedInput.currentResolvedInput = ResolvedSelectedAudioInput(deviceID: 202, uid: "mic-2", name: "USB Mic")
    harness.emitObservedChange()
    harness.emitObservedChange()

    try await waitForMicrophoneCondition { harness.signalEvents.count >= 2 }

    #expect(
        harness.signalEvents == [
            "recovering:microphone:4:device_configuration_changed",
            "interrupted:microphone:selected_microphone_disconnected:false",
        ]
    )
    #expect(harness.engine.stopCount == 1)
    #expect(harness.inputNode.removeTapCount == 1)
}

@Test func sameUIDRecoverySealsPreChangeChunkAndKeepsSampleSequenceContinuous() async throws {
    let chunks = RecordingRecoveryChunkSink()
    let harness = try MicrophoneCaptureHarness(chunkSink: chunks)
    let capture = harness.makeCapture()

    try await capture.start(selectedMicrophoneUID: "mic-1")
    await capture.activate()
    harness.emitObservedChange()
    try await waitForMicrophoneCondition { harness.signalEvents.count >= 2 }
    _ = await capture.stop()

    let sealed = await chunks.chunks
    #expect(sealed.count == 2)
    #expect(sealed[0].endSamplePosition == sealed[1].startSamplePosition)
    #expect(sealed[0].source == .microphone)
    #expect(sealed[1].source == .microphone)
}

@Test func configurationChangeDuringStartupWaitsForInitialFrameCommitBeforeRecovery() async throws {
    let harness = try MicrophoneCaptureHarness(firstFrameMachTicks: nil)
    let capture = harness.makeCapture()

    let start = Task { try await capture.start(selectedMicrophoneUID: "mic-1") }
    try await waitForMicrophoneCondition {
        harness.operations.filter { $0 == .startEngine }.count == 1
    }
    harness.emitObservedChange()
    harness.inputNode.setAutomaticFirstFrame(hostTime: 456)
    try harness.inputNode.emitFrame(hostTime: 123)
    try await start.value
    await capture.activate()
    try await waitForMicrophoneCondition { harness.signalEvents.count >= 2 }

    #expect(harness.operations.filter { $0 == .startEngine }.count == 2)
    #expect(harness.signalEvents == [
        "recovering:microphone:2:device_configuration_changed",
        "recovered:microphone:2:mic-1",
    ])
}

@Test func configurationChangeWaitsUntilTheSourceIdentityIsPublished() async throws {
    let chunks = RecordingRecoveryChunkSink()
    let harness = try MicrophoneCaptureHarness(chunkSink: chunks)
    let capture = harness.makeCapture()

    try await capture.start(selectedMicrophoneUID: "mic-1")
    harness.emitObservedChange()
    try await Task.sleep(for: .milliseconds(50))
    await capture.activate()
    try await Task.sleep(for: .milliseconds(30))

    #expect(harness.signals.isEmpty)
    #expect(await chunks.chunks.isEmpty)
}

@Test func stopRemovesObserversAndIgnoresLateCallbacks() async throws {
    let harness = try MicrophoneCaptureHarness()
    let capture = harness.makeCapture()

    try await capture.start(selectedMicrophoneUID: "mic-1")
    _ = await capture.stop()
    harness.emitObservedChange()

    try await Task.sleep(for: .milliseconds(50))

    #expect(harness.selectedInput.cancelledObserverCount == 2)
    #expect(harness.signals.isEmpty)
}

private struct MicrophoneCaptureHarness {
    enum Operation: Equatable, Sendable {
        case resolve(String)
        case bind(AudioDeviceID)
        case current
        case inputNode
        case hardwareInputFormat
        case observe(AudioDeviceID)
        case installTap
        case startEngine
    }

    let engine: FakeMicrophoneAudioEngine
    let inputNode: FakeMicrophoneAudioInputNode
    let selectedInput: FakeSelectedAudioInputManager
    let firstFrameTimeout: Duration
    let chunkSink: any SealedChunkSink
    let stagingRoot: URL
    private let failures = Locked<[CaptureFailure]>([])
    private let recordedSignals = Locked<[CaptureSourceSignal]>([])

    init(
        resolvedInputs: [String: ResolvedSelectedAudioInput] = [
            "mic-1": ResolvedSelectedAudioInput(deviceID: 101, uid: "mic-1", name: "Studio Mic"),
        ],
        currentResolvedInput: ResolvedSelectedAudioInput? = ResolvedSelectedAudioInput(deviceID: 101, uid: "mic-1", name: "Studio Mic"),
        inputFormats: [AVAudioFormat]? = nil,
        firstFrameMachTicks: UInt64? = 123_456,
        firstFrameTimeout: Duration = .seconds(2),
        expectedInputFormat: SelectedAudioInputFormat = SelectedAudioInputFormat(
            sampleRate: 48_000,
            channelCount: 1
        ),
        chunkSink: any SealedChunkSink = NoopChunkSink()
    ) throws {
        let operations = Locked<[Operation]>([])
        let defaultFormat = try #require(
            AVAudioFormat(standardFormatWithSampleRate: 48_000, channels: 1)
        )
        inputNode = FakeMicrophoneAudioInputNode(
            operations: operations,
            formats: inputFormats ?? [defaultFormat],
            firstFrameMachTicks: firstFrameMachTicks
        )
        engine = FakeMicrophoneAudioEngine(inputNode: inputNode, operations: operations)
        selectedInput = FakeSelectedAudioInputManager(
            operations: operations,
            resolvedInputs: resolvedInputs,
            currentResolvedInput: currentResolvedInput,
            expectedInputFormat: expectedInputFormat
        )
        self.firstFrameTimeout = firstFrameTimeout
        self.chunkSink = chunkSink
        stagingRoot = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    }

    var operations: [Operation] { engine.operations.value }
    var signals: [CaptureSourceSignal] { recordedSignals.value }
    var signalEvents: [String] {
        signals.map { signal in
            switch signal {
            case let .interrupted(source, reason, recoverable):
                return "interrupted:\(source.rawValue):\(reason):\(recoverable)"
            case let .permissionRevoked(source):
                return "permission_revoked:\(source.rawValue)"
            case let .recovering(source, generation, reason):
                return "recovering:\(source.rawValue):\(generation):\(reason)"
            case let .recovered(source, generation, deviceId, _):
                return "recovered:\(source.rawValue):\(generation):\(deviceId)"
            }
        }
    }

    func makeCapture() -> MicrophoneCapture {
        MicrophoneCapture(
            encoder: try! ChunkEncoder(
                source: .microphone,
                stagingRoot: stagingRoot,
                sampleRate: 16_000,
                channelCount: 1,
                maxFramesPerChunk: 16_000 * 30,
                sink: chunkSink
            ),
            failureHandler: { failure in
                failures.withValue { $0.append(failure) }
            },
            signalHandler: { signal in
                recordedSignals.withValue { $0.append(signal) }
            },
            engine: engine,
            selectedAudioInput: selectedInput,
            firstFrameTimeout: firstFrameTimeout
        )
    }

    func emitObservedChange() {
        selectedInput.emitObservedChange()
    }
}

private actor NoopChunkSink: SealedChunkSink {
    func didSeal(_: SealedChunk) async {}
}

private actor RecordingRecoveryChunkSink: SealedChunkSink {
    private(set) var chunks: [SealedChunk] = []
    func didSeal(_ chunk: SealedChunk) async { chunks.append(chunk) }
}

private final class FakeMicrophoneAudioEngine: MicrophoneAudioEngine, @unchecked Sendable {
    private let node: any MicrophoneAudioInputNode
    let operations: Locked<[MicrophoneCaptureHarness.Operation]>
    private let observers = Locked<[UUID: @Sendable () -> Void]>([:])
    private(set) var stopCount = 0

    init(
        inputNode: any MicrophoneAudioInputNode,
        operations: Locked<[MicrophoneCaptureHarness.Operation]>
    ) {
        node = inputNode
        self.operations = operations
    }

    var inputNode: any MicrophoneAudioInputNode {
        operations.withValue { $0.append(.inputNode) }
        return node
    }

    func start() throws {
        operations.withValue { $0.append(.startEngine) }
        try (node as? FakeMicrophoneAudioInputNode)?.emitFirstFrameIfConfigured()
    }

    func stop() {
        stopCount += 1
    }

    func pause() {}

    func addConfigurationChangeObserver(
        _ handler: @escaping @Sendable () -> Void
    ) -> any SelectedAudioInputObserver {
        let id = UUID()
        observers.withValue { $0[id] = handler }
        return FakeSelectedAudioInputObserver(cancelHandler: { () -> Void in
            _ = self.observers.withValue { $0.removeValue(forKey: id) }
        })
    }
}

private final class FakeMicrophoneAudioInputNode: MicrophoneAudioInputNode, @unchecked Sendable {
    let operations: Locked<[MicrophoneCaptureHarness.Operation]>
    private let formats: Locked<[AVAudioFormat]>
    private let tapBlock = Locked<AVAudioNodeTapBlock?>(nil)
    private let firstFrameMachTicks: Locked<UInt64?>
    private(set) var removeTapCount = 0
    private(set) var installedTapSampleRate: Double?

    init(
        operations: Locked<[MicrophoneCaptureHarness.Operation]>,
        formats: [AVAudioFormat],
        firstFrameMachTicks: UInt64?
    ) {
        self.operations = operations
        self.formats = Locked(formats)
        self.firstFrameMachTicks = Locked(firstFrameMachTicks)
    }

    func hardwareInputFormat(forBus _: AVAudioNodeBus) -> AVAudioFormat {
        operations.withValue { $0.append(.hardwareInputFormat) }
        return formats.withValue { formats in
            if formats.count > 1 {
                return formats.removeFirst()
            }
            return formats[0]
        }
    }

    func installTap(
        onBus _: AVAudioNodeBus,
        bufferSize _: AVAudioFrameCount,
        format: AVAudioFormat?,
        block: @escaping AVAudioNodeTapBlock
    ) {
        installedTapSampleRate = format?.sampleRate
        tapBlock.withValue { $0 = block }
        operations.withValue { $0.append(.installTap) }
    }

    func removeTap(onBus _: AVAudioNodeBus) {
        removeTapCount += 1
        tapBlock.withValue { $0 = nil }
    }

    func emitFirstFrameIfConfigured() throws {
        guard let firstFrameMachTicks = firstFrameMachTicks.value else { return }
        try emitFrame(hostTime: firstFrameMachTicks)
    }

    func setAutomaticFirstFrame(hostTime: UInt64?) {
        firstFrameMachTicks.withValue { $0 = hostTime }
    }

    func emitFrame(hostTime: UInt64) throws {
        let format = formats.value[0]
        let buffer = try #require(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 1_024))
        buffer.frameLength = 1_024
        if let channels = buffer.floatChannelData {
            for channel in 0 ..< Int(format.channelCount) {
                for frame in 0 ..< Int(buffer.frameLength) {
                    channels[channel][frame] = 0.25
                }
            }
        }
        let block = try #require(tapBlock.value)
        block(buffer, AVAudioTime(hostTime: hostTime))
    }
}

private final class FakeSelectedAudioInputManager: SelectedAudioInputManaging, @unchecked Sendable {
    let operations: Locked<[MicrophoneCaptureHarness.Operation]>
    let resolvedInputs: [String: ResolvedSelectedAudioInput]
    var currentResolvedInput: ResolvedSelectedAudioInput?
    let expectedInputFormat: SelectedAudioInputFormat
    private let observers = Locked<[UUID: @Sendable () -> Void]>([:])
    private let cancelledObservers = Locked<Int>(0)

    init(
        operations: Locked<[MicrophoneCaptureHarness.Operation]>,
        resolvedInputs: [String: ResolvedSelectedAudioInput],
        currentResolvedInput: ResolvedSelectedAudioInput?,
        expectedInputFormat: SelectedAudioInputFormat
    ) {
        self.operations = operations
        self.resolvedInputs = resolvedInputs
        self.currentResolvedInput = currentResolvedInput
        self.expectedInputFormat = expectedInputFormat
    }

    var cancelledObserverCount: Int { cancelledObservers.value }

    func resolveSelectedInput(uid: String) throws -> ResolvedSelectedAudioInput {
        operations.withValue { $0.append(.resolve(uid)) }
        guard let resolved = resolvedInputs[uid] else {
            throw SelectedAudioInputError.unavailable
        }
        return resolved
    }

    func bindSelectedInput(deviceID: AudioDeviceID) throws {
        operations.withValue { $0.append(.bind(deviceID)) }
    }

    func currentBoundInput() throws -> ResolvedSelectedAudioInput {
        operations.withValue { $0.append(.current) }
        guard let currentResolvedInput else {
            throw SelectedAudioInputError.unavailable
        }
        return currentResolvedInput
    }

    func expectedInputFormat(deviceID _: AudioDeviceID) throws -> SelectedAudioInputFormat {
        expectedInputFormat
    }

    func addChangeObservers(
        selectedDeviceID: AudioDeviceID,
        handler: @escaping @Sendable () -> Void
    ) throws -> [any SelectedAudioInputObserver] {
        operations.withValue { $0.append(.observe(selectedDeviceID)) }
        let ids = [UUID(), UUID()]
        observers.withValue {
            for id in ids {
                $0[id] = handler
            }
        }
        return ids.map { id in
            FakeSelectedAudioInputObserver(cancelHandler: {
                self.cancelledObservers.withValue { $0 += 1 }
                _ = self.observers.withValue { $0.removeValue(forKey: id) }
            })
        }
    }

    func emitObservedChange() {
        for handler in observers.value.values {
            handler()
        }
    }
}

private struct FakeSelectedAudioInputObserver: SelectedAudioInputObserver {
    let cancelHandler: @Sendable () -> Void
    func cancel() {
        cancelHandler()
    }
}

private final class Locked<Value>: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: Value

    init(_ value: Value) {
        storage = value
    }

    var value: Value {
        lock.withLock { storage }
    }

    func withValue<Result>(_ body: (inout Value) throws -> Result) rethrows -> Result {
        try lock.withLock {
            try body(&storage)
        }
    }
}

private func waitForMicrophoneCondition(
    timeout: Duration = .seconds(1),
    condition: @escaping @Sendable () -> Bool
) async throws {
    let deadline = ContinuousClock.now.advanced(by: timeout)
    while !condition() {
        guard ContinuousClock.now < deadline else { throw TimeoutError() }
        try await Task.sleep(for: .milliseconds(5))
    }
}

private struct TimeoutError: Error {}
