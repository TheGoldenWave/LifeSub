import Foundation
import Testing
import CaptureProtocol
@testable import CaptureHelper

@Test func startsOnlyTheRequestedSource() async throws {
    let microphone = FakeSource(.microphone)
    let system = FakeSource(.systemAudio)
    let controller = CaptureSessionController(sources: [microphone, system])

    try await controller.start(requested: [.microphone])
    #expect(await microphone.startedCount == 1)
    #expect(await system.startedCount == 0)
    #expect(await controller.state == .running(enabled: [.microphone]))
}

@Test func dualSourceStartRollsBackAtomically() async throws {
    let microphone = FakeSource(.microphone)
    let system = FakeSource(.systemAudio, startError: CaptureSourceError.permissionRevoked)
    let controller = CaptureSessionController(sources: [microphone, system])

    await #expect(throws: CaptureSourceError.permissionRevoked) {
        try await controller.start(requested: [.microphone, .systemAudio])
    }
    #expect(await microphone.stoppedCount == 1)
    #expect(await controller.state == .failed)
}

@Test func pauseResumeAndStopAffectOnlyEnabledSources() async throws {
    let microphone = FakeSource(.microphone)
    let system = FakeSource(.systemAudio)
    let controller = CaptureSessionController(sources: [microphone, system])
    try await controller.start(requested: [.systemAudio])

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
    try await controller.start(requested: [.microphone, .systemAudio])

    await #expect(throws: CaptureSourceError.sourceFailed) { try await controller.pause() }

    #expect(await microphone.stoppedCount == 1)
    #expect(await system.stoppedCount == 1)
    #expect(await controller.state == .failed)
}

@Test func partialResumeFailureStopsTheWholeDualSourceSession() async throws {
    let microphone = FakeSource(.microphone)
    let system = FakeSource(.systemAudio, resumeError: CaptureSourceError.sourceFailed)
    let controller = CaptureSessionController(sources: [microphone, system])
    try await controller.start(requested: [.microphone, .systemAudio])
    try await controller.pause()

    await #expect(throws: CaptureSourceError.sourceFailed) { try await controller.resume() }

    #expect(await microphone.stoppedCount == 1)
    #expect(await system.stoppedCount == 1)
    #expect(await controller.state == .failed)
}

@Test func interruptionAndPermissionRevocationSealAffectedSource() async throws {
    let microphone = FakeSource(.microphone)
    let system = FakeSource(.systemAudio)
    let sink = RecordingEventSink()
    let controller = CaptureSessionController(sources: [microphone, system], eventSink: sink)
    try await controller.start(requested: [.microphone, .systemAudio])

    await controller.sourceInterrupted(.microphone, reason: "device_disconnected", recoverable: true)
    await controller.permissionRevoked(.systemAudio)

    #expect(await microphone.sealedCount == 1)
    #expect(await microphone.startedCount == 2)
    #expect(await microphone.stoppedCount == 1)
    #expect(await system.sealedCount == 1)
    #expect(await sink.events.contains(.sourceInterrupted(source: .microphone, reason: "device_disconnected", recoverable: true)))
    #expect(await sink.events.contains(.permissionRevoked(source: .systemAudio)))
}

@Test func shutdownStopsAllEnabledSources() async throws {
    let microphone = FakeSource(.microphone)
    let system = FakeSource(.systemAudio)
    let controller = CaptureSessionController(sources: [microphone, system])
    try await controller.start(requested: [.microphone, .systemAudio])

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
    #expect(sealed.allSatisfy { $0.relativePath.hasSuffix(".partial") })
    #expect(sealed.allSatisfy { FileManager.default.fileExists(atPath: root.appendingPathComponent($0.relativePath).path) })
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

private actor FakeSource: CaptureSourceAdapter {
    nonisolated let source: Source
    private let startError: CaptureSourceError?
    private let pauseError: CaptureSourceError?
    private let resumeError: CaptureSourceError?
    private(set) var startedCount = 0
    private(set) var pausedCount = 0
    private(set) var resumedCount = 0
    private(set) var stoppedCount = 0
    private(set) var sealedCount = 0

    init(
        _ source: Source,
        startError: CaptureSourceError? = nil,
        pauseError: CaptureSourceError? = nil,
        resumeError: CaptureSourceError? = nil
    ) {
        self.source = source
        self.startError = startError
        self.pauseError = pauseError
        self.resumeError = resumeError
    }

    func preflight() async throws {}

    func start() async throws {
        if let startError { throw startError }
        startedCount += 1
    }

    func pause() async throws {
        if let pauseError { throw pauseError }
        pausedCount += 1
    }
    func resume() async throws {
        if let resumeError { throw resumeError }
        resumedCount += 1
    }
    func stop() async { stoppedCount += 1 }
    func sealForDiscontinuity() async { sealedCount += 1 }
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
