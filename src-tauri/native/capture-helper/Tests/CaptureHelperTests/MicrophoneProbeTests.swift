@preconcurrency import AVFoundation
import CaptureProtocol
import Foundation
import Testing
@testable import CaptureHelper

@Test func liveMicrophoneAudioEngineDoesNotCreateSystemEngineUntilUsed() {
    let engine = LiveMicrophoneAudioEngine()

    let ownsSystemEngine = Mirror(reflecting: engine).children.contains {
        $0.value is AVAudioEngine
    }

    #expect(!ownsSystemEngine)
}

@Test func liveMicrophoneAudioEngineDelaysUsedSystemEngineRelease() async throws {
    weak var inputNode: AVAudioInputNode?

    do {
        let engine = LiveMicrophoneAudioEngine()
        inputNode = engine.inputNode as? AVAudioInputNode
    }

    #expect(inputNode != nil)
    try await Task.sleep(for: .milliseconds(500))
    #expect(inputNode == nil)
}

@Test func microphoneProbeBindsExactUIDAndRequiresARealFrame() async {
    let input = ProbeInputNode(format: probeFormat())
    let engine = ProbeEngine(input: input, deliverFrameOnStart: true)
    let manager = ProbeSelectedInputManager(readBackUID: "mic-1")
    let probe = MicrophoneProbe(
        engine: engine,
        selectedAudioInput: manager,
        listDevices: { [probeDevice()] },
        authorize: { true },
        firstFrameTimeout: .milliseconds(50),
        formatSettleTimeout: .milliseconds(50)
    )

    let result = await probe.probe(uid: "mic-1")

    #expect(result.result == .compatible)
    #expect(result.uid == "mic-1")
    #expect(result.readBackUid == "mic-1")
    #expect(result.inputSampleRateHz == 44_100)
    #expect(result.inputChannelCount == 1)
    #expect(result.firstFrameSampleCount == 128)
    #expect(manager.operations == ["resolve:mic-1", "bind:101", "readback", "format:101"])
    #expect(engine.startCount == 1)
    #expect(engine.stopCount == 1)
    #expect(input.removeTapCount == 1)
}

@Test func microphoneProbeRejectsReadbackMismatchBeforeStartingEngine() async {
    let input = ProbeInputNode(format: probeFormat())
    let engine = ProbeEngine(input: input, deliverFrameOnStart: false)
    let manager = ProbeSelectedInputManager(readBackUID: "mic-2")
    let probe = MicrophoneProbe(
        engine: engine,
        selectedAudioInput: manager,
        listDevices: { [probeDevice()] },
        authorize: { true },
        firstFrameTimeout: .milliseconds(20),
        formatSettleTimeout: .milliseconds(20)
    )

    let result = await probe.probe(uid: "mic-1")

    #expect(result.result == .identityMismatch)
    #expect(result.readBackUid == "mic-2")
    #expect(result.firstFrameSampleCount == 0)
    #expect(engine.startCount == 0)
    #expect(input.installTapCount == 0)
}

@Test func microphoneProbeReportsNoFramesAndCleansUpWithoutAnEncoder() async {
    let input = ProbeInputNode(format: probeFormat())
    let engine = ProbeEngine(input: input, deliverFrameOnStart: false)
    let probe = MicrophoneProbe(
        engine: engine,
        selectedAudioInput: ProbeSelectedInputManager(readBackUID: "mic-1"),
        listDevices: { [probeDevice()] },
        authorize: { true },
        firstFrameTimeout: .milliseconds(5),
        formatSettleTimeout: .milliseconds(20)
    )

    let result = await probe.probe(uid: "mic-1")

    #expect(result.result == .noAudioFrames)
    #expect(result.firstFrameSampleCount == 0)
    #expect(engine.startCount == 1)
    #expect(engine.stopCount == 1)
    #expect(input.removeTapCount == 1)
}

private func probeDevice() -> MicrophoneDevice {
    MicrophoneDevice(
        uid: "mic-1",
        name: "Built-in Microphone",
        isDefault: true,
        isConnected: true,
        transport: .builtIn
    )
}

private func probeFormat() -> AVAudioFormat {
    AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 44_100, channels: 1, interleaved: false)!
}

private final class ProbeInputNode: MicrophoneAudioInputNode, @unchecked Sendable {
    let format: AVAudioFormat
    var tap: AVAudioNodeTapBlock?
    private(set) var installTapCount = 0
    private(set) var removeTapCount = 0

    init(format: AVAudioFormat) {
        self.format = format
    }

    func hardwareInputFormat(forBus _: AVAudioNodeBus) -> AVAudioFormat { format }

    func installTap(
        onBus _: AVAudioNodeBus,
        bufferSize _: AVAudioFrameCount,
        format _: AVAudioFormat?,
        block: @escaping AVAudioNodeTapBlock
    ) {
        installTapCount += 1
        tap = block
    }

    func removeTap(onBus _: AVAudioNodeBus) {
        removeTapCount += 1
        tap = nil
    }

    func deliverFrame() {
        let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 128)!
        buffer.frameLength = 128
        tap?(buffer, AVAudioTime(hostTime: 42))
    }
}

private final class ProbeEngine: MicrophoneAudioEngine, @unchecked Sendable {
    let probeInput: ProbeInputNode
    let deliverFrameOnStart: Bool
    private(set) var startCount = 0
    private(set) var stopCount = 0

    init(input: ProbeInputNode, deliverFrameOnStart: Bool) {
        probeInput = input
        self.deliverFrameOnStart = deliverFrameOnStart
    }

    var inputNode: any MicrophoneAudioInputNode { probeInput }

    func start() throws {
        startCount += 1
        if deliverFrameOnStart { probeInput.deliverFrame() }
    }

    func stop() { stopCount += 1 }
    func pause() {}
    func addConfigurationChangeObserver(
        _: @escaping @Sendable () -> Void
    ) -> any SelectedAudioInputObserver {
        ProbeObserver()
    }
}

private final class ProbeSelectedInputManager: SelectedAudioInputManaging, @unchecked Sendable {
    let readBackUID: String
    private(set) var operations: [String] = []

    init(readBackUID: String) {
        self.readBackUID = readBackUID
    }

    func resolveSelectedInput(uid: String) throws -> ResolvedSelectedAudioInput {
        operations.append("resolve:\(uid)")
        return ResolvedSelectedAudioInput(deviceID: 101, uid: uid, name: "Built-in Microphone")
    }

    func bindSelectedInput(deviceID: AudioDeviceID) throws {
        operations.append("bind:\(deviceID)")
    }

    func currentBoundInput() throws -> ResolvedSelectedAudioInput {
        operations.append("readback")
        return ResolvedSelectedAudioInput(deviceID: readBackUID == "mic-1" ? 101 : 202, uid: readBackUID, name: "Readback")
    }

    func expectedInputFormat(deviceID: AudioDeviceID) throws -> SelectedAudioInputFormat {
        operations.append("format:\(deviceID)")
        return SelectedAudioInputFormat(sampleRate: 44_100, channelCount: 1)
    }

    func addChangeObservers(
        selectedDeviceID _: AudioDeviceID,
        handler _: @escaping @Sendable () -> Void
    ) throws -> [any SelectedAudioInputObserver] {
        []
    }
}

private struct ProbeObserver: SelectedAudioInputObserver {
    func cancel() {}
}
