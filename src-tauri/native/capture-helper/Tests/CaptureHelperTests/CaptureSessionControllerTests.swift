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

private actor FakeSource: CaptureSourceAdapter {
    nonisolated let source: Source
    private let startError: CaptureSourceError?
    private(set) var startedCount = 0
    private(set) var pausedCount = 0
    private(set) var resumedCount = 0
    private(set) var stoppedCount = 0
    private(set) var sealedCount = 0

    init(_ source: Source, startError: CaptureSourceError? = nil) {
        self.source = source
        self.startError = startError
    }

    func preflight() async throws {}

    func start() async throws {
        if let startError { throw startError }
        startedCount += 1
    }

    func pause() async throws { pausedCount += 1 }
    func resume() async throws { resumedCount += 1 }
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
