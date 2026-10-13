import Darwin
import Foundation
import Testing
import CaptureProtocol
@testable import CaptureHelper

@Test func captureClockUsesTheSameMachAbsoluteHostTimeDomainAsAudioTimestamps() {
    let first = CaptureContinuousClock.nowNanoseconds()
    let second = CaptureContinuousClock.nowNanoseconds()
    let audioHostNow = CaptureContinuousClock.nanoseconds(fromMachTicks: mach_absolute_time())
    let difference = max(second, audioHostNow) - min(second, audioHostNow)
    #expect(second >= first)
    #expect(difference < 100_000_000)
    #expect(CaptureContinuousClock.nanoseconds(fromMachTicks: 0) == 0)
}

@Test func startsOnlyTheRequestedSource() async throws {
    let microphone = FakeSource(.microphone)
    let system = FakeSource(.systemAudio)
    let controller = CaptureSessionController(sources: [microphone, system])

    try await controller.start(requested: [.microphone], selectedMicrophoneUID: "mic-1")
    #expect(await microphone.startedCount == 1)
    #expect(await system.startedCount == 0)
    #expect(await microphone.startUIDs == ["mic-1"])
    #expect(await controller.state == .running(enabled: [.microphone]))
}

@Test func dualSourceStartRollsBackAtomically() async throws {
    let microphone = FakeSource(.microphone)
    let system = FakeSource(.systemAudio, startError: CaptureSourceError.permissionRevoked)
    let controller = CaptureSessionController(sources: [microphone, system])

    await #expect(throws: CaptureSourceError.permissionRevoked) {
        try await controller.start(requested: [.microphone, .systemAudio], selectedMicrophoneUID: "mic-1")
    }
    #expect(await microphone.stoppedCount == 1)
    #expect(await controller.state == .failed)
}

@Test func stopDuringStartupPreventsSourcePublicationAndActivation() async throws {
    let gate = StartGate()
    let microphone = FakeSource(.microphone, startGate: gate)
    let sink = RecordingEventSink()
    let controller = CaptureSessionController(sources: [microphone], eventSink: sink)
    let start = Task {
        try await controller.start(requested: [.microphone], selectedMicrophoneUID: "mic-1")
    }
    while !(await gate.entered) {
        try await Task.sleep(for: .milliseconds(1))
    }

    try await controller.stop()
    await gate.release()
    await #expect(throws: CaptureSourceError.sourceFailed) { try await start.value }

    #expect(await microphone.stoppedCount == 1)
    #expect(await microphone.activatedCount == 0)
    #expect(await sink.events.allSatisfy { event in
        if case .sourceStarted = event { return false }
        return true
    })
    #expect(await controller.state == .stopped)
}

@Test func pauseResumeAndStopAffectOnlyEnabledSources() async throws {
    let microphone = FakeSource(.microphone)
    let system = FakeSource(.systemAudio)
    let controller = CaptureSessionController(sources: [microphone, system])
    try await controller.start(requested: [.systemAudio], selectedMicrophoneUID: nil)

    try await controller.pause()
    try await controller.resume()
    try await controller.stop()

    #expect(await system.pausedCount == 1)
    #expect(await system.resumedCount == 1)
    #expect(await system.stoppedCount == 1)
    #expect(await microphone.stoppedCount == 0)
    #expect(await controller.state == .stopped)
}

@Test func partialPauseFailureStopsTheWholeDualSourceSession() async throws {
    let microphone = FakeSource(.microphone)
    let system = FakeSource(.systemAudio, pauseError: CaptureSourceError.sourceFailed)
    let controller = CaptureSessionController(sources: [microphone, system])
    try await controller.start(requested: [.microphone, .systemAudio], selectedMicrophoneUID: "mic-1")

    await #expect(throws: CaptureSourceError.sourceFailed) { try await controller.pause() }

    #expect(await microphone.stoppedCount == 1)
    #expect(await system.stoppedCount == 1)
    #expect(await controller.state == .failed)
}

@Test func partialResumeFailureStopsTheWholeDualSourceSession() async throws {
    let microphone = FakeSource(.microphone)
    let system = FakeSource(.systemAudio, resumeError: CaptureSourceError.sourceFailed)
    let controller = CaptureSessionController(sources: [microphone, system])
    try await controller.start(requested: [.microphone, .systemAudio], selectedMicrophoneUID: "mic-1")
    try await controller.pause()

    await #expect(throws: CaptureSourceError.sourceFailed) { try await controller.resume() }

    #expect(await microphone.stoppedCount == 1)
    #expect(await system.stoppedCount == 1)
    #expect(await controller.state == .failed)
}

@Test func recoverableInterruptionSealsAndKeepsSourceRunning() async throws {
    let microphone = FakeSource(.systemAudio)
    let sink = RecordingEventSink()
    let controller = CaptureSessionController(sources: [microphone], eventSink: sink)
    try await controller.start(requested: [.systemAudio], selectedMicrophoneUID: nil)

    await controller.sourceInterrupted(.systemAudio, reason: "device_configuration_changed", recoverable: true)

    #expect(await microphone.sealedCount == 1)
    #expect(await microphone.startedCount == 1)
    #expect(await microphone.stoppedCount == 0)
    #expect(await sink.events.contains(.sourceInterrupted(source: .systemAudio, reason: "device_configuration_changed", recoverable: true)))
    #expect(await controller.state == .running(enabled: [.systemAudio]))
}

@Test func microphoneRecoveryEventsAreForwardedWithoutDuplicateSealing() async throws {
    let microphone = FakeSource(.microphone)
    let sink = RecordingEventSink()
    let controller = CaptureSessionController(sources: [microphone], eventSink: sink)
    try await controller.start(requested: [.microphone], selectedMicrophoneUID: "mic-1")

    await controller.sourceRecovering(
        .microphone,
        generation: 7,
        reason: "device_configuration_changed"
    )
    await controller.sourceRecovered(
        .microphone,
        generation: 7,
        deviceId: "mic-1",
        firstHostTime: 42
    )

    #expect(await microphone.sealedCount == 0)
    #expect(
        await sink.events.suffix(2) == [
            .sourceRecovering(source: .microphone, generation: 7, reason: "device_configuration_changed"),
            .sourceRecovered(source: .microphone, generation: 7, deviceId: "mic-1", firstHostTime: 42),
        ]
    )
    #expect(await controller.state == .running(enabled: [.microphone]))
}

@Test func nonrecoverableInterruptionStopsSelectedMicrophoneWithoutRestart() async throws {
    let microphone = FakeSource(.microphone)
    let sink = RecordingEventSink()
    let controller = CaptureSessionController(sources: [microphone], eventSink: sink)
    try await controller.start(requested: [.microphone], selectedMicrophoneUID: "mic-1")

    await controller.sourceInterrupted(.microphone, reason: "selected_microphone_disconnected", recoverable: false)

    #expect(await microphone.sealedCount == 1)
    #expect(await microphone.startedCount == 1)
    #expect(await microphone.stoppedCount == 1)
    #expect(await sink.events.contains(.sourceInterrupted(source: .microphone, reason: "selected_microphone_disconnected", recoverable: false)))
    #expect(await controller.state == .failed)
}

@Test func permissionRevocationSealsAffectedSource() async throws {
    let microphone = FakeSource(.microphone)
    let system = FakeSource(.systemAudio)
    let sink = RecordingEventSink()
    let controller = CaptureSessionController(sources: [microphone, system], eventSink: sink)
    try await controller.start(requested: [.microphone, .systemAudio], selectedMicrophoneUID: "mic-1")

    await controller.permissionRevoked(.systemAudio)

    #expect(await system.sealedCount == 1)
    #expect(await sink.events.contains(.permissionRevoked(source: .systemAudio)))
}

@Test func shutdownStopsAllEnabledSources() async throws {
    let microphone = FakeSource(.microphone)
    let system = FakeSource(.systemAudio)
    let controller = CaptureSessionController(sources: [microphone, system])
    try await controller.start(requested: [.microphone, .systemAudio], selectedMicrophoneUID: "mic-1")

    await controller.shutdown()

    #expect(await microphone.stoppedCount == 1)
    #expect(await system.stoppedCount == 1)
    #expect(await controller.state == .stopped)
}

@Test func chunkRotationKeepsSourcesIndependent() async throws {
    let root = FileManager.default.temporaryDirectory
        .appendingPathComponent(UUID().uuidString, isDirectory: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let sink = RecordingChunkSink()
    let microphone = try ChunkEncoder(
        source: .microphone,
        stagingRoot: root,
        sampleRate: 16_000,
        channelCount: 1,
        maxFramesPerChunk: 4,
        sink: sink
    )
    let system = try ChunkEncoder(
        source: .systemAudio,
        stagingRoot: root,
        sampleRate: 16_000,
        channelCount: 1,
        maxFramesPerChunk: 4,
        sink: sink
    )

    try await microphone.append(samples: [1, 2, 3, 4], hostTime: 10)
    try await system.append(samples: [5, 6, 7, 8], hostTime: 20)
    let sealed = await sink.chunks

    #expect(sealed.count == 2)
    #expect(Set(sealed.map(\.source)) == [.microphone, .systemAudio])
    #expect(sealed.allSatisfy { chunk in
        let components = chunk.relativePath.split(separator: ".", omittingEmptySubsequences: false)
        return components.count == 3
            && components[0].hasPrefix("chk_")
            && components[1] == "wav"
            && components[2] == "partial"
    })
    #expect(sealed.allSatisfy { FileManager.default.fileExists(atPath: root.appendingPathComponent($0.relativePath).path) })
    for chunk in sealed {
        let bytes = try Data(contentsOf: root.appendingPathComponent(chunk.relativePath))
        #expect(String(decoding: bytes.prefix(4), as: UTF8.self) == "RIFF")
        #expect(String(decoding: bytes.dropFirst(8).prefix(4), as: UTF8.self) == "WAVE")
    }
}

@Test func boundedQueueReportsOverflowAndDrainsAcceptedFrames() async throws {
    let consumed = FrameRecorder()
    let failures = FailureRecorder()
    let queue = BoundedAudioFrameQueue(
        capacity: 1,
        label: "com.lifesub.tests.bounded-queue",
        consumer: { frame in
            try await Task.sleep(for: .milliseconds(30))
            await consumed.append(frame.hostTime)
        },
        failureHandler: failures.record
    )

    try queue.submit(CapturedPcmFrame(samples: [1, 2], hostTime: 1))
    #expect(throws: AudioBackpressureError.queueFull) {
        try queue.submit(CapturedPcmFrame(samples: [3, 4], hostTime: 2))
    }
    await queue.finish()

    #expect(await consumed.values == [1])
    #expect(failures.codes == ["queue_full"])
}

@Test func planarSystemAudioIsMixedOnceAndPreservesTiming() throws {
    let decoded = try SystemAudioSampleDecoder.decodePlanarFloat(
        channels: [[1, 0], [0, 0]],
        sampleRate: 16_000,
        hostTime: 987_654
    )

    #expect(decoded.samples == [0.5, 0])
    #expect(decoded.sampleRate == 16_000)
    #expect(decoded.hostTime == 987_654)
}

@Test func streamingResamplerPreservesOneSecondAcrossShortHardwareBuffers() throws {
    let resampler = MonoPcm16Resampler()
    var remainingInputFrames = 44_100
    var outputFrames = 0

    while remainingInputFrames > 0 {
        let inputFrames = min(256, remainingInputFrames)
        outputFrames += try resampler.convert(
            floatSamples: [Float](repeating: 0.25, count: inputFrames),
            sampleRate: 44_100
        ).count
        remainingInputFrames -= inputFrames
    }

    #expect((15_990 ... 16_010).contains(outputFrames))
}

@Test func streamingResamplerDropsCachedStateAcrossDirectRateMode() throws {
    let resampler = MonoPcm16Resampler()
    let input = [Float](repeating: 0.25, count: 256)

    for _ in 0 ..< 4 {
        _ = try resampler.convert(floatSamples: input, sampleRate: 44_100)
    }
    _ = try resampler.convert(floatSamples: input, sampleRate: 16_000)

    let resumedOutput = try resampler.convert(floatSamples: input, sampleRate: 44_100)
    let freshOutput = try MonoPcm16Resampler().convert(
        floatSamples: input,
        sampleRate: 44_100
    )

    #expect(resumedOutput.count == freshOutput.count)
}

@Test func stopDrainsAcceptedFramesBeforeFinalSealAndRejectsReopen() async throws {
    let root = FileManager.default.temporaryDirectory
        .appendingPathComponent(UUID().uuidString, isDirectory: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let chunks = RecordingChunkSink()
    let failures = FailureRecorder()
    let encoder = try ChunkEncoder(
        source: .microphone,
        stagingRoot: root,
        sampleRate: 16_000,
        channelCount: 1,
        maxFramesPerChunk: 100,
        sink: chunks
    )
    let queue = BoundedAudioFrameQueue(
        capacity: 2,
        label: "com.lifesub.tests.stop-drain",
        consumer: { frame in
            try await encoder.append(samples: frame.samples, hostTime: frame.hostTime)
        },
        failureHandler: failures.record
    )
    try queue.submit(CapturedPcmFrame(samples: [1, 2, 3, 4], hostTime: 42))

    await queue.finish()
    try await encoder.sealForDiscontinuity()
    #expect(throws: AudioBackpressureError.stopped) {
        try queue.submit(CapturedPcmFrame(samples: [5, 6], hostTime: 43))
    }

    #expect(await chunks.chunks.count == 1)
    #expect(failures.codes == ["other"])
}

@Test func singleCallbackChunkEndsAfterItsCapturedSamples() async throws {
    let root = FileManager.default.temporaryDirectory
        .appendingPathComponent(UUID().uuidString, isDirectory: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let chunks = RecordingChunkSink()
    let encoder = try ChunkEncoder(
        source: .microphone,
        stagingRoot: root,
        sampleRate: 16_000,
        channelCount: 1,
        maxFramesPerChunk: 16_000,
        sink: chunks
    )

    try await encoder.append(
        samples: [Int16](repeating: 1, count: 1_480),
        hostTime: 1_000_000_000
    )
    try await encoder.sealForDiscontinuity()

    let sealed = try #require(await chunks.chunks.first)
    #expect(sealed.frameCount == 1_480)
    #expect(sealed.endHostTime - sealed.startHostTime == 92_500_000)
}

private actor FakeSource: CaptureSourceAdapter {
    nonisolated let source: Source
    private let startError: CaptureSourceError?
    private let pauseError: CaptureSourceError?
    private let resumeError: CaptureSourceError?
    private let startGate: StartGate?
    private(set) var startedCount = 0
    private(set) var startUIDs: [String?] = []
    private(set) var pausedCount = 0
    private(set) var resumedCount = 0
    private(set) var stoppedCount = 0
    private(set) var sealedCount = 0
    private(set) var activatedCount = 0

    init(
        _ source: Source,
        startError: CaptureSourceError? = nil,
        pauseError: CaptureSourceError? = nil,
        resumeError: CaptureSourceError? = nil,
        startGate: StartGate? = nil
    ) {
        self.source = source
        self.startError = startError
        self.pauseError = pauseError
        self.resumeError = resumeError
        self.startGate = startGate
    }

    func preflight() async throws {}

    func metadata() async throws -> CaptureSourceMetadata {
        CaptureSourceMetadata(
            deviceId: source.rawValue,
            deviceName: "Test \(source.rawValue)",
            sampleRate: 16_000,
            channelCount: 1,
            hostTime: 1
        )
    }

    func start(selectedMicrophoneUID: String?) async throws {
        if let startError { throw startError }
        if let startGate { await startGate.wait() }
        startedCount += 1
        startUIDs.append(selectedMicrophoneUID)
    }

    func activate() async { activatedCount += 1 }

    func pause() async throws {
        if let pauseError { throw pauseError }
        pausedCount += 1
    }
    func resume() async throws {
        if let resumeError { throw resumeError }
        resumedCount += 1
    }
    func stop() async -> CaptureSourceStopMetadata {
        stoppedCount += 1
        return CaptureSourceStopMetadata(finalSequence: UInt64(stoppedCount), startHostTime: 1, endHostTime: 2)
    }
    func sealForDiscontinuity() async { sealedCount += 1 }
}

private actor StartGate {
    private(set) var entered = false
    private var continuation: CheckedContinuation<Void, Never>?

    func wait() async {
        entered = true
        await withCheckedContinuation { continuation = $0 }
    }

    func release() {
        continuation?.resume()
        continuation = nil
    }
}

private actor RecordingEventSink: CaptureEventSink {
    private(set) var events: [CaptureControllerEvent] = []
    func emit(_ event: CaptureControllerEvent) async { events.append(event) }
}

private actor RecordingChunkSink: SealedChunkSink {
    private(set) var chunks: [SealedChunk] = []
    func didSeal(_ chunk: SealedChunk) async { chunks.append(chunk) }
}

private actor FrameRecorder {
    private(set) var values: [UInt64] = []
    func append(_ value: UInt64) { values.append(value) }
}

private final class FailureRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var stored: [String] = []
    var codes: [String] { lock.withLock { stored } }

    func record(_ error: CaptureFailure) {
        lock.withLock {
            if error == .backpressure { stored.append("queue_full") }
            else { stored.append("other") }
        }
    }
}
