import Darwin
import CoreAudio
import Foundation
import Testing
import CaptureProtocol
@testable import CaptureHelper

@Suite(.serialized)
struct CommandLoopTests {

@Test func commandLoopDispatchesLifecycleAndTelemetry() async throws {
    let pair = try LocalSocketPair()
    let factory = RecordingControllerFactory()
    let loop = HelperCommandLoop(
        connection: pair.helper,
        nonceHex: String(repeating: "a", count: 64),
        controllerFactory: factory
    )
    let task = Task { try await loop.run() }

    let hello = try await pair.peer.readFrame(timeout: .seconds(2))
    #expect(
        hello == .hello(
            Hello(
                protocolVersion: ProtocolLimits.version,
                helperPid: UInt32(ProcessInfo.processInfo.processIdentifier),
                launchNonce: String(repeating: "a", count: 64),
                supportedSources: [.microphone, .systemAudio],
                capabilities: [.deviceDiscovery, .exactMicrophoneSelection, .microphoneProbe, .permissionDiagnostics]
            )
        )
    )

    try pair.peer.writeFrame(
        .start(
            StartCaptureCommand(
                protocolVersion: ProtocolLimits.version,
                requestedSources: [.microphone, .systemAudio],
                stagingRoot: "/tmp/lifesub-capture-helper-tests",
                microphoneDeviceUid: "mic-1"
            )
        )
    )
    try pair.peer.writeFrame(.pause(PauseCaptureCommand(protocolVersion: ProtocolLimits.version)))
    try pair.peer.writeFrame(.resume(ResumeCaptureCommand(protocolVersion: ProtocolLimits.version)))
    try pair.peer.writeFrame(.stop(StopCaptureCommand(protocolVersion: ProtocolLimits.version)))
    try pair.peer.writeFrame(.shutdown(ShutdownCaptureCommand(protocolVersion: ProtocolLimits.version)))

    let frames = try await pair.peer.readFrames(timeout: .seconds(3), until: { $0.contains(where: { header in
        if case .shutdownAck = header { return true }
        return false
    }) })

    #expect(frames.contains(where: { if case .permissionState = $0 { return true } else { return false } }))
    #expect(frames.contains(where: { if case .sourceStarted = $0 { return true } else { return false } }))
    #expect(frames.contains(where: { if case .sourceStopped = $0 { return true } else { return false } }))
    #expect(frames.contains(where: { if case .shutdownAck = $0 { return true } else { return false } }))
    #expect(await factory.calls == ["start", "pause", "resume", "stop", "shutdown"])
    #expect(await factory.startRequests == [.init(requested: [.microphone, .systemAudio], selectedMicrophoneUID: "mic-1")])

    try await task.value
}

@Test func commandLoopProcessesSingleStartFrameWithoutWaitingForAnotherCommand() async throws {
    let pair = try LocalSocketPair()
    let factory = RecordingControllerFactory()
    let loop = HelperCommandLoop(
        connection: pair.helper,
        nonceHex: String(repeating: "c", count: 64),
        controllerFactory: factory
    )
    let task = Task { try await loop.run() }

    _ = try await pair.peer.readFrame(timeout: .seconds(2))
    try pair.peer.writeFrame(
        .start(
            StartCaptureCommand(
                protocolVersion: ProtocolLimits.version,
                requestedSources: [.microphone],
                stagingRoot: "/tmp/lifesub-capture-helper-single-start",
                microphoneDeviceUid: "mic-1"
            )
        )
    )

    let event = try await pair.peer.readFrame(timeout: .seconds(2))
    #expect({ if case .permissionState = event { return true } else { return false } }())

    try pair.peer.writeFrame(.shutdown(ShutdownCaptureCommand(protocolVersion: ProtocolLimits.version)))
    _ = try await pair.peer.readFrames(timeout: .seconds(3), until: { frames in
        frames.contains(where: { if case .shutdownAck = $0 { return true } else { return false } })
    })
    try await task.value
}

@Test func commandLoopPreservesStableCaptureStartErrors() async throws {
    let cases: [(CaptureSourceError, String)] = [
        (.permissionDenied(source: .microphone), "microphone_permission_denied"),
        (.permissionDenied(source: .systemAudio), "screen_recording_permission_denied"),
        (.permissionRevoked, "microphone_permission_revoked"),
        (.deviceUnavailable, "capture_device_unavailable"),
        (.selectedMicrophoneUnavailable, "selected_microphone_unavailable"),
        (.selectedMicrophoneIdentityMismatch, "selected_microphone_identity_mismatch"),
        (.selectedMicrophoneDisconnected, "selected_microphone_disconnected"),
        (.sourceFailed, "capture_source_failed"),
    ]

    for (error, expectedCode) in cases {
        let pair = try LocalSocketPair()
        let loop = HelperCommandLoop(
            connection: pair.helper,
            nonceHex: String(repeating: "9", count: 64),
            controllerFactory: FailingStartControllerFactory(error: error)
        )
        let task = Task { try await loop.run() }
        _ = try await pair.peer.readFrame(timeout: .seconds(2))
        try pair.peer.writeFrame(
            .start(
                StartCaptureCommand(
                    protocolVersion: ProtocolLimits.version,
                    requestedSources: [.microphone],
                    stagingRoot: "/tmp/lifesub-stable-start-error",
                    microphoneDeviceUid: "mic-1"
                )
            )
        )

        let fatal = try await pair.peer.readFrame(timeout: .seconds(2))
        #expect({
            if case let .fatalError(value) = fatal {
                return value.code == expectedCode
            }
            return false
        }())
        try await task.value
    }
}

@Test func commandLoopListsMicrophonesWithoutStartingController() async throws {
    let pair = try LocalSocketPair()
    let factory = RecordingControllerFactory()
    let deviceCatalog = FakeMicrophoneDeviceCatalog(
        deviceIDs: [11, 12],
        defaultInputDeviceID: 11,
        devices: [
            11: FakeMicrophoneDeviceCatalog.Device(uid: "builtin-1", name: "Built-in Microphone", isAlive: true, transportType: kAudioDeviceTransportTypeBuiltIn, hasInput: true),
            12: FakeMicrophoneDeviceCatalog.Device(uid: "usb-1", name: "USB Mic", isAlive: true, transportType: kAudioDeviceTransportTypeUSB, hasInput: true),
        ]
    )
    let loop = HelperCommandLoop(
        connection: pair.helper,
        nonceHex: String(repeating: "d", count: 64),
        controllerFactory: factory,
        microphoneDeviceCatalog: deviceCatalog,
        microphoneProberFactory: {
            Issue.record("passive listing must not construct a microphone probe")
            return RecordingMicrophoneProber(result: unavailableProbeResult())
        }
    )
    let task = Task { try await loop.run() }

    _ = try await pair.peer.readFrame(timeout: .seconds(2))
    try pair.peer.writeFrame(.listMicrophones(ListMicrophonesCommand(protocolVersion: ProtocolLimits.version)))

    let response = try await pair.peer.readFrame(timeout: .seconds(2))
    #expect(
        response == .microphoneDevices(
            MicrophoneDevicesMessage(
                devices: [
                    MicrophoneDevice(uid: "builtin-1", name: "Built-in Microphone", isDefault: true, isConnected: true, transport: .builtIn),
                    MicrophoneDevice(uid: "usb-1", name: "USB Mic", isDefault: false, isConnected: true, transport: .usb),
                ]
            )
        )
    )
    #expect(try pair.peer.readFrameIfAvailable(timeout: .milliseconds(100)) == nil)
    #expect(await factory.calls.isEmpty)
    #expect(deviceCatalog.operations == [.deviceIDs, .defaultInputDeviceID, .uid(11), .name(11), .alive(11), .hasInput(11), .transport(11), .uid(12), .name(12), .alive(12), .hasInput(12), .transport(12)])

    try pair.peer.writeFrame(.shutdown(ShutdownCaptureCommand(protocolVersion: ProtocolLimits.version)))
    _ = try await pair.peer.readFrames(timeout: .seconds(3), until: { frames in
        frames.contains(where: { if case .shutdownAck = $0 { return true } else { return false } })
    })
    try await task.value
}

@Test func commandLoopFailsClosedWhenMicrophoneEnumerationFails() async throws {
    let pair = try LocalSocketPair()
    let factory = RecordingControllerFactory()
    let deviceCatalog = FakeMicrophoneDeviceCatalog(
        deviceIDs: [21],
        defaultInputDeviceID: 21,
        devices: [
            21: FakeMicrophoneDeviceCatalog.Device(uid: "builtin-1", name: "Built-in Microphone", isAlive: true, transportType: kAudioDeviceTransportTypeBuiltIn, hasInput: true),
        ],
        failingProperties: Set([.deviceIDs])
    )
    let loop = HelperCommandLoop(
        connection: pair.helper,
        nonceHex: String(repeating: "e", count: 64),
        controllerFactory: factory,
        microphoneDeviceCatalog: deviceCatalog
    )
    let task = Task { try await loop.run() }

    _ = try await pair.peer.readFrame(timeout: .seconds(2))
    try pair.peer.writeFrame(.listMicrophones(ListMicrophonesCommand(protocolVersion: ProtocolLimits.version)))

    let fatal = try await pair.peer.readFrame(timeout: .seconds(2))
    #expect({
        if case let .fatalError(value) = fatal {
            return value.code == "microphone_enumeration_failed"
                && value.summary == "microphone enumeration failed"
        }
        return false
    }())
    #expect(await factory.calls.isEmpty)
    #expect(deviceCatalog.operations == [.deviceIDs])

    try await task.value
}

@Test func commandLoopProbesMicrophoneWithoutCreatingCaptureControllerOrStaging() async throws {
    let pair = try LocalSocketPair()
    let factory = RecordingControllerFactory()
    let probeResult = MicrophoneProbeMessage(
        uid: "mic-1",
        name: "Built-in Microphone",
        transport: .builtIn,
        inputSampleRateHz: 44_100,
        inputChannelCount: 1,
        readBackUid: "mic-1",
        firstFrameSampleCount: 128,
        elapsedMs: 12,
        result: .compatible
    )
    let prober = RecordingMicrophoneProber(result: probeResult)
    let loop = HelperCommandLoop(
        connection: pair.helper,
        nonceHex: String(repeating: "f", count: 64),
        controllerFactory: factory,
        microphoneProberFactory: { prober }
    )
    let task = Task { try await loop.run() }

    _ = try await pair.peer.readFrame(timeout: .seconds(2))
    try pair.peer.writeFrame(
        .probeMicrophone(
            ProbeMicrophoneCommand(
                protocolVersion: ProtocolLimits.version,
                microphoneDeviceUid: "mic-1"
            )
        )
    )

    #expect(try await pair.peer.readFrame(timeout: .seconds(2)) == .microphoneProbeResult(probeResult))
    #expect(await prober.uids == ["mic-1"])
    #expect(await factory.calls.isEmpty)
    #expect(await factory.createdControllerUIDs.isEmpty)

    try pair.peer.writeFrame(.shutdown(ShutdownCaptureCommand(protocolVersion: ProtocolLimits.version)))
    #expect(try await pair.peer.readFrame(timeout: .seconds(2)) == .shutdownAck)
    try await task.value
}

@Test func commandLoopReadsPermissionsWithoutRequestingOrStartingCapture() async throws {
    let pair = try LocalSocketPair()
    let factory = RecordingControllerFactory()
    let permissions = RecordingPermissionStateProvider(
        value: PermissionStateMessage(microphone: .denied, screenRecording: .notGranted)
    )
    let loop = HelperCommandLoop(
        connection: pair.helper,
        nonceHex: String(repeating: "9", count: 64),
        controllerFactory: factory,
        permissionStateProvider: permissions
    )
    let task = Task { try await loop.run() }

    _ = try await pair.peer.readFrame(timeout: .seconds(2))
    try pair.peer.writeFrame(
        .getPermissionState(GetPermissionStateCommand(protocolVersion: ProtocolLimits.version))
    )

    #expect(try await pair.peer.readFrame(timeout: .seconds(2)) == .permissionState(
        PermissionStateMessage(microphone: .denied, screenRecording: .notGranted)
    ))
    #expect(permissions.readCount == 1)
    #expect(await factory.calls.isEmpty)
    #expect(await factory.createdControllerUIDs.isEmpty)

    try pair.peer.writeFrame(.shutdown(ShutdownCaptureCommand(protocolVersion: ProtocolLimits.version)))
    #expect(try await pair.peer.readFrame(timeout: .seconds(2)) == .shutdownAck)
    try await task.value
}

@Test func commandLoopFailsClosedWhenUsableMicrophoneCountExceedsProtocolBound() async throws {
    let pair = try LocalSocketPair()
    let factory = RecordingControllerFactory()
    let deviceIDs = (1 ... 33).map(AudioDeviceID.init)
    let devices = Dictionary(uniqueKeysWithValues: deviceIDs.map { deviceID in
        (
            deviceID,
            FakeMicrophoneDeviceCatalog.Device(
                uid: "mic-\(deviceID)",
                name: "Microphone \(deviceID)",
                isAlive: true,
                transportType: kAudioDeviceTransportTypeUSB,
                hasInput: true
            )
        )
    })
    let deviceCatalog = FakeMicrophoneDeviceCatalog(
        deviceIDs: deviceIDs,
        defaultInputDeviceID: deviceIDs.first,
        devices: devices
    )
    let loop = HelperCommandLoop(
        connection: pair.helper,
        nonceHex: String(repeating: "f", count: 64),
        controllerFactory: factory,
        microphoneDeviceCatalog: deviceCatalog
    )
    let task = Task { try await loop.run() }

    _ = try await pair.peer.readFrame(timeout: .seconds(2))
    try pair.peer.writeFrame(.listMicrophones(ListMicrophonesCommand(protocolVersion: ProtocolLimits.version)))

    let frames = try await pair.peer.readFrames(timeout: .seconds(3), until: { frames in
        frames.contains(where: { if case .fatalError = $0 { return true } else { return false } })
    })
    #expect(frames.count == 1)
    #expect({
        if case let .fatalError(value) = frames.first {
            return value.code == "microphone_enumeration_failed"
                && value.summary == "microphone enumeration failed"
        }
        return false
    }())
    #expect(!frames.contains(where: { if case .microphoneDevices = $0 { return true } else { return false } }))
    #expect(await factory.calls.isEmpty)

    try await task.value
}

@Test func commandLoopRejectsDuplicateStartWithoutCreatingSecondController() async throws {
    let pair = try LocalSocketPair()
    let factory = RecordingControllerFactory()
    let loop = HelperCommandLoop(
        connection: pair.helper,
        nonceHex: String(repeating: "1", count: 64),
        controllerFactory: factory
    )
    let task = Task { try await loop.run() }
    let start = CaptureHeader.start(
        StartCaptureCommand(
            protocolVersion: ProtocolLimits.version,
            requestedSources: [.microphone],
            stagingRoot: "/tmp/lifesub-duplicate-start",
            microphoneDeviceUid: "mic-1"
        )
    )

    _ = try await pair.peer.readFrame(timeout: .seconds(2))
    try pair.peer.writeFrame(start)
    try pair.peer.writeFrame(start)

    let frames = try await pair.peer.readFrames(timeout: .seconds(3), until: { frames in
        frames.contains(where: { if case .fatalError = $0 { return true } else { return false } })
    })
    #expect(stableInvalidControlFrameCount(in: frames) == 1)
    #expect(await factory.calls == ["start", "shutdown"])
    try await task.value
}

@Test func commandLoopAllowsOnlyShutdownAfterMicrophoneDiscovery() async throws {
    let pair = try LocalSocketPair()
    let factory = RecordingControllerFactory()
    let deviceCatalog = FakeMicrophoneDeviceCatalog(
        deviceIDs: [31],
        defaultInputDeviceID: 31,
        devices: [
            31: FakeMicrophoneDeviceCatalog.Device(uid: "mic-31", name: "Microphone 31", isAlive: true, transportType: kAudioDeviceTransportTypeUSB, hasInput: true),
        ]
    )
    let loop = HelperCommandLoop(
        connection: pair.helper,
        nonceHex: String(repeating: "2", count: 64),
        controllerFactory: factory,
        microphoneDeviceCatalog: deviceCatalog
    )
    let task = Task { try await loop.run() }

    _ = try await pair.peer.readFrame(timeout: .seconds(2))
    let list = CaptureHeader.listMicrophones(ListMicrophonesCommand(protocolVersion: ProtocolLimits.version))
    try pair.peer.writeFrame(list)
    _ = try await pair.peer.readFrame(timeout: .seconds(2))
    try pair.peer.writeFrame(list)

    let fatal = try await pair.peer.readFrame(timeout: .seconds(2))
    #expect(stableInvalidControlFrameCount(in: [fatal]) == 1)
    #expect(await factory.calls.isEmpty)
    #expect(deviceCatalog.operations.filter { $0 == .deviceIDs }.count == 1)
    try await task.value
}

@Test func commandLoopRejectsDiscoveryAfterStartAndShutsDownControllerOnce() async throws {
    let pair = try LocalSocketPair()
    let factory = RecordingControllerFactory()
    let loop = HelperCommandLoop(
        connection: pair.helper,
        nonceHex: String(repeating: "3", count: 64),
        controllerFactory: factory
    )
    let task = Task { try await loop.run() }

    _ = try await pair.peer.readFrame(timeout: .seconds(2))
    try pair.peer.writeFrame(
        .start(
            StartCaptureCommand(
                protocolVersion: ProtocolLimits.version,
                requestedSources: [.microphone],
                stagingRoot: "/tmp/lifesub-list-after-start",
                microphoneDeviceUid: "mic-1"
            )
        )
    )
    try pair.peer.writeFrame(.listMicrophones(ListMicrophonesCommand(protocolVersion: ProtocolLimits.version)))

    let frames = try await pair.peer.readFrames(timeout: .seconds(3), until: { frames in
        frames.contains(where: { if case .fatalError = $0 { return true } else { return false } })
    })
    #expect(stableInvalidControlFrameCount(in: frames) == 1)
    #expect(!frames.contains(where: { if case .microphoneDevices = $0 { return true } else { return false } }))
    #expect(await factory.calls == ["start", "shutdown"])
    try await task.value
}

@Test func outboundWriterSerializesTelemetryResponsesAndAcknowledgements() async throws {
    let destination = ChunkingCaptureFrameDestination()
    let writer = OutboundCaptureFrameWriter(destination: destination)
    let sink = SocketCaptureEventSink(writer: writer)

    try await withThrowingTaskGroup(of: Void.self) { group in
        for sequence in 0 ..< 20 {
            group.addTask {
                await sink.emit(
                    .sourceStopped(
                        source: .microphone,
                        finalSequence: UInt64(sequence),
                        startHostTime: UInt64(sequence),
                        endHostTime: UInt64(sequence + 1)
                    )
                )
            }
            group.addTask {
                try await writer.send(.shutdownAck)
            }
            group.addTask {
                try await writer.send(.microphoneDevices(MicrophoneDevicesMessage(devices: [])))
            }
        }
        try await group.waitForAll()
    }

    var bytes = destination.bytes
    var frames: [CaptureHeader] = []
    while !bytes.isEmpty {
        let decoded = try FrameCodec.decodePrefix(bytes)
        frames.append(decoded.frame.header)
        bytes = Data(bytes.dropFirst(decoded.consumedBytes))
    }
    #expect(frames.count == 60)
    #expect(frames.filter { if case .sourceStopped = $0 { return true } else { return false } }.count == 20)
    #expect(frames.filter { if case .shutdownAck = $0 { return true } else { return false } }.count == 20)
    #expect(frames.filter { if case .microphoneDevices = $0 { return true } else { return false } }.count == 20)
}

@Test func commandLoopFailsClosedOnUnknownCommandVersionAndSource() async throws {
    let pair = try LocalSocketPair()
    let factory = RecordingControllerFactory()
    let loop = HelperCommandLoop(
        connection: pair.helper,
        nonceHex: String(repeating: "b", count: 64),
        controllerFactory: factory
    )
    let task = Task { try await loop.run() }

    _ = try await pair.peer.readFrame(timeout: .seconds(3))
    try pair.peer.writeRawFrame(
        header: #"{"microphone_device_uid":"mic-1","protocol_version":999,"requested_sources":["microphone"],"staging_root":"/tmp/x","type":"start"}"#,
        payload: Data()
    )

    let fatal = try await pair.peer.readFrame(timeout: .seconds(3))
    #expect({
        if case let .fatalError(value) = fatal {
            return value.code == "invalid_control_frame"
        }
        return false
    }())

    try await task.value
}

}

private struct FakeMicrophoneDeviceCatalog: AudioInputDeviceCatalog {
    struct Device: Sendable {
        let uid: String
        let name: String
        let isAlive: Bool
        let transportType: UInt32
        let hasInput: Bool
    }

    enum Operation: Equatable, Hashable, Sendable {
        case deviceIDs
        case defaultInputDeviceID
        case uid(AudioDeviceID)
        case name(AudioDeviceID)
        case alive(AudioDeviceID)
        case transport(AudioDeviceID)
        case hasInput(AudioDeviceID)
    }

    let listedDeviceIDs: [AudioDeviceID]
    let listedDefaultInputDeviceID: AudioDeviceID?
    let devices: [AudioDeviceID: Device]
    let failingProperties: Set<Operation>
    private let recordedOperations = Locked<[Operation]>([])

    init(
        deviceIDs: [AudioDeviceID],
        defaultInputDeviceID: AudioDeviceID?,
        devices: [AudioDeviceID: Device],
        failingProperties: Set<Operation> = []
    ) {
        listedDeviceIDs = deviceIDs
        listedDefaultInputDeviceID = defaultInputDeviceID
        self.devices = devices
        self.failingProperties = failingProperties
    }

    var operations: [Operation] { recordedOperations.value }

    func deviceIDs() throws -> [AudioDeviceID] {
        try recordAndThrowIfNeeded(.deviceIDs)
        return listedDeviceIDs
    }

    func defaultInputDeviceID() throws -> AudioDeviceID? {
        try recordAndThrowIfNeeded(.defaultInputDeviceID)
        return listedDefaultInputDeviceID
    }

    func deviceUID(_ deviceID: AudioDeviceID) throws -> String {
        try recordAndThrowIfNeeded(.uid(deviceID))
        return try device(for: deviceID).uid
    }

    func deviceName(_ deviceID: AudioDeviceID) throws -> String {
        try recordAndThrowIfNeeded(.name(deviceID))
        return try device(for: deviceID).name
    }

    func deviceIsAlive(_ deviceID: AudioDeviceID) throws -> Bool {
        try recordAndThrowIfNeeded(.alive(deviceID))
        return try device(for: deviceID).isAlive
    }

    func deviceTransportType(_ deviceID: AudioDeviceID) throws -> UInt32 {
        try recordAndThrowIfNeeded(.transport(deviceID))
        return try device(for: deviceID).transportType
    }

    func deviceHasInput(_ deviceID: AudioDeviceID) throws -> Bool {
        try recordAndThrowIfNeeded(.hasInput(deviceID))
        return try device(for: deviceID).hasInput
    }

    private func device(for deviceID: AudioDeviceID) throws -> Device {
        guard let device = devices[deviceID] else {
            throw POSIXError(.ENODEV)
        }
        return device
    }

    private func recordAndThrowIfNeeded(_ operation: Operation) throws {
        recordedOperations.withValue { $0.append(operation) }
        if failingProperties.contains(operation) {
            throw POSIXError(.EIO)
        }
    }
}

private struct LocalSocketPair {
    let helper: FileHandle
    let peer: FileHandle

    init() throws {
        var fds = [Int32](repeating: -1, count: 2)
        guard socketpair(AF_UNIX, SOCK_STREAM, 0, &fds) == 0 else {
            throw POSIXError(.ENOTSOCK)
        }
        helper = FileHandle(fileDescriptor: fds[0], closeOnDealloc: true)
        peer = FileHandle(fileDescriptor: fds[1], closeOnDealloc: true)
    }
}

private actor RecordingControllerFactory: CaptureSessionControllerFactory {
    struct StartRequest: Equatable, Sendable {
        let requested: Set<Source>
        let selectedMicrophoneUID: String?
    }

    private(set) var calls: [String] = []
    private(set) var createdControllerUIDs: [String?] = []
    private(set) var startRequests: [StartRequest] = []

    func makeController(
        stagingRoot: URL,
        eventSink: any CaptureEventSink,
        selectedMicrophoneUID: String?
    ) throws -> any HelperCaptureSessionControlling {
        createdControllerUIDs.append(selectedMicrophoneUID)
        return RecordingController(calls: self, eventSink: eventSink, stagingRoot: stagingRoot)
    }

    private actor RecordingController: HelperCaptureSessionControlling {
        let calls: RecordingControllerFactory
        let eventSink: any CaptureEventSink
        let stagingRoot: URL

        init(calls: RecordingControllerFactory, eventSink: any CaptureEventSink, stagingRoot: URL) {
            self.calls = calls
            self.eventSink = eventSink
            self.stagingRoot = stagingRoot
        }

        func start(requested: Set<Source>, selectedMicrophoneUID: String?) async throws {
            await calls.record("start")
            await calls.recordStartRequest(.init(requested: requested, selectedMicrophoneUID: selectedMicrophoneUID))
            await eventSink.emit(.permissionState(microphone: .granted, screenRecording: .granted))
            await eventSink.emit(
                .sourceStarted(
                    source: requested.contains(.microphone) ? .microphone : .systemAudio,
                    deviceId: "device",
                    deviceName: "Test Device",
                    sampleRate: 16_000,
                    channelCount: 1,
                    hostTime: 1
                )
            )
            await eventSink.emit(.sourceStopped(source: .microphone, finalSequence: 1, startHostTime: 1, endHostTime: 2))
        }

        func pause() async throws { await calls.record("pause") }
        func resume() async throws { await calls.record("resume") }
        func stop() async throws { await calls.record("stop") }
        func shutdown() async { await calls.record("shutdown") }
    }

    func record(_ call: String) {
        calls.append(call)
    }

    func recordStartRequest(_ request: StartRequest) {
        startRequests.append(request)
    }
}

private actor FailingStartControllerFactory: CaptureSessionControllerFactory {
    private let error: CaptureSourceError

    init(error: CaptureSourceError) {
        self.error = error
    }

    func makeController(
        stagingRoot _: URL,
        eventSink _: any CaptureEventSink,
        selectedMicrophoneUID _: String?
    ) throws -> any HelperCaptureSessionControlling {
        FailingStartController(error: error)
    }

    private actor FailingStartController: HelperCaptureSessionControlling {
        let error: CaptureSourceError

        init(error: CaptureSourceError) {
            self.error = error
        }

        func start(requested _: Set<Source>, selectedMicrophoneUID _: String?) async throws {
            throw error
        }

        func pause() async throws {}
        func resume() async throws {}
        func stop() async throws {}
        func shutdown() async {}
    }
}

private actor RecordingMicrophoneProber: MicrophoneProbing {
    let result: MicrophoneProbeMessage
    private(set) var uids: [String] = []

    init(result: MicrophoneProbeMessage) {
        self.result = result
    }

    func probe(uid: String) async -> MicrophoneProbeMessage {
        uids.append(uid)
        return result
    }
}

private final class RecordingPermissionStateProvider: PermissionStateProviding, @unchecked Sendable {
    let value: PermissionStateMessage
    private let reads = LockedCounter()

    init(value: PermissionStateMessage) { self.value = value }
    var readCount: Int { reads.value }
    func currentPermissionState() -> PermissionStateMessage {
        reads.increment()
        return value
    }
}

private final class LockedCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0
    var value: Int { lock.withLock { count } }
    func increment() { lock.withLock { count += 1 } }
}

private func unavailableProbeResult() -> MicrophoneProbeMessage {
    MicrophoneProbeMessage(
        uid: "unavailable",
        name: "Unavailable Microphone",
        transport: .unknown,
        inputSampleRateHz: nil,
        inputChannelCount: nil,
        readBackUid: nil,
        firstFrameSampleCount: 0,
        elapsedMs: 0,
        result: .unavailable
    )
}

private extension FileHandle {
    func writeFrame(_ header: CaptureHeader) throws {
        let bytes = try FrameCodec.encode(header: header, payload: Data())
        try writeRawFrame(bytes)
    }

    func writeRawFrame(header: String, payload: Data) throws {
        var bytes = Data()
        bytes.appendUInt32BE(UInt32(header.utf8.count))
        bytes.append(Data(header.utf8))
        bytes.appendUInt32BE(UInt32(payload.count))
        bytes.append(payload)
        try writeRawFrame(bytes)
    }

    func writeRawFrame(_ bytes: Data) throws {
        try write(contentsOf: bytes)
    }

    func readFrame(timeout: Duration) async throws -> CaptureHeader {
        let frames = try await readFrames(timeout: timeout, until: { !$0.isEmpty })
        return frames.first!
    }

    func readFrameIfAvailable(timeout: Duration) throws -> CaptureHeader? {
        let deadline = ContinuousClock.now.advanced(by: timeout)
        do {
            try waitUntilReadable(deadline: deadline)
        } catch is TimeoutError {
            return nil
        }
        return try readProtocolFrame()
    }

    func readFrames(timeout: Duration, until predicate: @escaping ([CaptureHeader]) -> Bool) async throws -> [CaptureHeader] {
        let deadline = ContinuousClock.now.advanced(by: timeout)
        var frames: [CaptureHeader] = []
        while !predicate(frames) {
            if ContinuousClock.now >= deadline {
                throw TimeoutError()
            }
            try waitUntilReadable(deadline: deadline)
            guard let frame = try readProtocolFrame() else { break }
            frames.append(frame)
        }
        return frames
    }

    func readProtocolFrame() throws -> CaptureHeader? {
        guard let headerLengthBytes = try readExactly(4) else { return nil }
        let headerLength = Int(headerLengthBytes.reduce(UInt32(0)) { ($0 << 8) | UInt32($1) })
        guard headerLength <= ProtocolLimits.maxHeaderBytes,
              let header = try readExactly(headerLength),
              let payloadLengthBytes = try readExactly(4)
        else { throw CaptureProtocolError.truncatedFrame }
        let payloadLength = Int(payloadLengthBytes.reduce(UInt32(0)) { ($0 << 8) | UInt32($1) })
        guard payloadLength <= ProtocolLimits.maxPayloadBytes,
              let payload = try readExactly(payloadLength)
        else { throw CaptureProtocolError.truncatedFrame }

        var frame = Data(capacity: 8 + header.count + payload.count)
        frame.append(headerLengthBytes)
        frame.append(header)
        frame.append(payloadLengthBytes)
        frame.append(payload)
        return try FrameCodec.decode(frame).header
    }

    func readExactly(_ count: Int) throws -> Data? {
        if count == 0 { return Data() }
        var result = Data(capacity: count)
        while result.count < count {
            guard let bytes = try read(upToCount: count - result.count), !bytes.isEmpty else {
                if result.isEmpty { return nil }
                throw CaptureProtocolError.truncatedFrame
            }
            result.append(bytes)
        }
        return result
    }
}

private struct TimeoutError: Error {}

private func stableInvalidControlFrameCount(in frames: [CaptureHeader]) -> Int {
    frames.filter { frame in
        if case let .fatalError(value) = frame {
            return value.code == "invalid_control_frame" && value.summary == "helper command failed"
        }
        return false
    }.count
}

private final class ChunkingCaptureFrameDestination: CaptureFrameDestination, @unchecked Sendable {
    private let storage = Locked(Data())

    var bytes: Data { storage.value }

    func write(_ data: Data) throws {
        for byte in data {
            storage.withValue { $0.append(byte) }
            sched_yield()
        }
    }

    func failClosed() {}
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
        try lock.withLock { try body(&storage) }
    }
}

private extension FileHandle {
    func waitUntilReadable(deadline: ContinuousClock.Instant) throws {
        while true {
            let remaining = ContinuousClock.now.duration(to: deadline).components
            if remaining.seconds < 0 || (remaining.seconds == 0 && remaining.attoseconds <= 0) {
                throw TimeoutError()
            }
            var descriptor = pollfd(fd: fileDescriptor, events: Int16(POLLIN), revents: 0)
            let milliseconds = max(
                1,
                min(
                    Int32.max,
                    Int32(remaining.seconds * 1_000)
                        + Int32(remaining.attoseconds / 1_000_000_000_000_000)
                )
            )
            let result = withUnsafeMutablePointer(to: &descriptor) {
                poll($0, 1, milliseconds)
            }
            if result > 0 { return }
            if result == 0 { throw TimeoutError() }
            if errno == EINTR { continue }
            throw POSIXError(.EIO)
        }
    }
}

private extension Data {
    mutating func appendUInt32BE(_ value: UInt32) {
        append(UInt8((value >> 24) & 0xff))
        append(UInt8((value >> 16) & 0xff))
        append(UInt8((value >> 8) & 0xff))
        append(UInt8(value & 0xff))
    }
}
