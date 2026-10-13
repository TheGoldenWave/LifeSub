import Foundation
import Testing
@testable import CaptureProtocol

private let hello = CaptureHeader.hello(
    Hello(
        protocolVersion: 2,
        helperPid: 4242,
        launchNonce: "0123456789abcdef",
        supportedSources: [.microphone, .systemAudio],
        capabilities: [.deviceDiscovery, .exactMicrophoneSelection, .microphoneProbe, .permissionDiagnostics]
    )
)

@Test func controlCommandsRoundTripAndRejectUnknownFields() throws {
    let commands: [CaptureCommand] = [
        .start(StartCaptureCommand(protocolVersion: 2, requestedSources: [.microphone], stagingRoot: "/tmp/capture", microphoneDeviceUid: "mic-1")),
        .start(StartCaptureCommand(protocolVersion: 2, requestedSources: [.systemAudio], stagingRoot: "/tmp/capture")),
        .listMicrophones(ListMicrophonesCommand(protocolVersion: 2)),
        .probeMicrophone(ProbeMicrophoneCommand(protocolVersion: 2, microphoneDeviceUid: "mic-1")),
        .getPermissionState(GetPermissionStateCommand(protocolVersion: 2)),
        .pause(PauseCaptureCommand(protocolVersion: 2)),
        .resume(ResumeCaptureCommand(protocolVersion: 2)),
        .stop(StopCaptureCommand(protocolVersion: 2)),
        .shutdown(ShutdownCaptureCommand(protocolVersion: 2)),
    ]
    for command in commands {
        let encoded = try CommandJSON.encode(command)
        #expect(try CommandJSON.decode(encoded) == command)
    }
    #expect(throws: CaptureProtocolError.invalidHeader) {
        try CommandJSON.decode(Data(#"{"type":"start","protocol_version":2,"requested_sources":["microphone"],"staging_root":"/tmp/capture","microphone_device_uid":"mic-1","extra":true}"#.utf8))
    }
    #expect(throws: CaptureProtocolError.invalidHeader) {
        try CommandJSON.decode(Data(#"{"type":"start","protocol_version":2,"requested_sources":["microphone"],"staging_root":"/tmp/capture"}"#.utf8))
    }
    #expect(throws: CaptureProtocolError.invalidHeader) {
        try CommandJSON.decode(Data(#"{"type":"start","protocol_version":2,"requested_sources":["system_audio"],"staging_root":"/tmp/capture","microphone_device_uid":"mic-1"}"#.utf8))
    }
    for incompatibleJSON in [
        #"{"type":"start","protocol_version":1,"requested_sources":["microphone"],"staging_root":"/tmp/capture","microphone_device_uid":"mic-1"}"#,
        #"{"type":"list_microphones","protocol_version":1}"#,
        #"{"type":"probe_microphone","protocol_version":1,"microphone_device_uid":"mic-1"}"#,
        #"{"type":"get_permission_state","protocol_version":1}"#,
        #"{"type":"pause","protocol_version":1}"#,
        #"{"type":"resume","protocol_version":1}"#,
        #"{"type":"stop","protocol_version":1}"#,
        #"{"type":"shutdown","protocol_version":1}"#,
    ] {
        #expect(throws: CaptureProtocolError.unsupportedVersion) {
            try CommandJSON.decode(Data(incompatibleJSON.utf8))
        }
    }
    #expect(throws: CaptureProtocolError.invalidHeader) {
        try CommandJSON.decode(Data(#"{"type":"explode"}"#.utf8))
    }
}

@Test func probeContractRoundTripsAndRejectsUntruthfulResults() throws {
    let command = CaptureHeader.probeMicrophone(
        ProbeMicrophoneCommand(protocolVersion: 2, microphoneDeviceUid: "mic-1")
    )
    #expect(try CommandJSON.decode(CommandJSON.encode(command)) == command)

    let compatible = CaptureHeader.microphoneProbeResult(
        MicrophoneProbeMessage(
            uid: "mic-1",
            name: "Built-in Microphone",
            transport: .builtIn,
            inputSampleRateHz: 44_100,
            inputChannelCount: 1,
            readBackUid: "mic-1",
            firstFrameSampleCount: 371,
            elapsedMs: 42,
            result: .compatible
        )
    )
    #expect(try CanonicalJSON.decode(CanonicalJSON.encode(compatible)) == compatible)

    for invalidJSON in [
        #"{"type":"probe_microphone","protocol_version":2,"microphone_device_uid":""}"#,
        #"{"type":"microphone_probe_result","uid":"mic-1","name":"Built-in Microphone","transport":"built_in","input_sample_rate_hz":44100,"input_channel_count":1,"read_back_uid":"mic-1","first_frame_sample_count":0,"elapsed_ms":42,"result":"compatible"}"#,
        #"{"type":"microphone_probe_result","uid":"mic-1","name":"Built-in Microphone","transport":"built_in","input_sample_rate_hz":44100,"input_channel_count":1,"read_back_uid":"mic-1","first_frame_sample_count":0,"elapsed_ms":42,"result":"identity_mismatch"}"#,
        #"{"type":"microphone_probe_result","uid":"mic-1","name":"Built-in Microphone","transport":"built_in","input_sample_rate_hz":44100,"input_channel_count":1,"read_back_uid":"mic-1","first_frame_sample_count":371,"elapsed_ms":42,"result":"invented"}"#,
        #"{"type":"microphone_probe_result","uid":"mic-1","name":"Built-in Microphone","transport":"built_in","input_sample_rate_hz":44100,"input_channel_count":1,"read_back_uid":"mic-1","first_frame_sample_count":371,"elapsed_ms":42,"result":"compatible","extra":true}"#,
    ] {
        #expect(throws: CaptureProtocolError.invalidHeader) {
            try CanonicalJSON.decode(Data(invalidJSON.utf8))
        }
    }
}

@Test func recoveryEventsRoundTripAndRejectInvalidGenerationOrIdentity() throws {
    let recovering = CaptureHeader.sourceRecovering(
        SourceRecovering(source: .microphone, generation: 7, reason: "device_configuration_changed")
    )
    let recovered = CaptureHeader.sourceRecovered(
        SourceRecovered(source: .microphone, generation: 7, deviceId: "mic-1", firstHostTime: 42)
    )
    #expect(try CanonicalJSON.decode(CanonicalJSON.encode(recovering)) == recovering)
    #expect(try CanonicalJSON.decode(CanonicalJSON.encode(recovered)) == recovered)

    for invalidJSON in [
        #"{"type":"source_recovering","source":"microphone","generation":0,"reason":"device_configuration_changed"}"#,
        #"{"type":"source_recovered","source":"microphone","generation":0,"device_id":"mic-1","first_host_time":42}"#,
        #"{"type":"source_recovered","source":"microphone","generation":7,"device_id":"","first_host_time":42}"#,
        #"{"type":"source_recovered","source":"microphone","generation":7,"device_id":"mic-1","first_host_time":0}"#,
    ] {
        #expect(throws: CaptureProtocolError.invalidHeader) {
            try CanonicalJSON.decode(Data(invalidJSON.utf8))
        }
    }
}

@Test func canonicalHelloFrameMatchesRustBytes() throws {
    let encoded = try FrameCodec.encode(header: hello, payload: Data())
    let fixtureURL = try #require(
        Bundle.module.url(forResource: "canonical-hello-frame", withExtension: "hex")
    )
    let expected = try Data(hex: String(contentsOf: fixtureURL, encoding: .utf8))

    #expect(encoded == expected)
    #expect(try FrameCodec.decode(encoded).header == hello)
}

@Test func rejectsOversizedAndMalformedFrames() throws {
    var oversizedHeader = Data()
    oversizedHeader.appendUInt32BE(UInt32(ProtocolLimits.maxHeaderBytes + 1))
    #expect(throws: CaptureProtocolError.headerTooLarge) {
        try FrameCodec.decode(oversizedHeader)
    }

    for json in [
        #"{"capabilities":["device_discovery","exact_microphone_selection"],"helper_pid":4242,"launch_nonce":"n","protocol_version":2,"supported_sources":["microphone"],"type":"hello","unexpected":true}"#,
        #"{"type":"invented"}"#,
        #"{"channel_count":1,"device_id":"x","host_time":1,"sample_rate":16000,"source":"mixed","type":"source_started"}"#,
        #"{"channel_count":1,"discontinuity_flags":[],"format":"float64","host_time":1,"sample_position":0,"sequence":1,"source":"microphone","type":"audio_frame"}"#,
    ] {
        let header = json.data(using: .utf8)!
        var frame = Data()
        frame.appendUInt32BE(UInt32(header.count))
        frame.append(header)
        frame.appendUInt32BE(0)
        #expect(throws: CaptureProtocolError.invalidHeader) {
            try FrameCodec.decode(frame)
        }
    }
}

@Test func decodesListMicrophonesAndRejectsInvalidDeviceLists() throws {
    let listHeader = try CanonicalJSON.decode(
        Data(#"{"protocol_version":2,"type":"list_microphones"}"#.utf8)
    )
    #expect(listHeader == .listMicrophones(ListMicrophonesCommand(protocolVersion: 2)))

    let devicesHeader = try CanonicalJSON.decode(
        Data(
            #"{"devices":[{"is_connected":true,"is_default":true,"name":"Built-in Microphone","transport":"built_in","uid":"mic-1"},{"is_connected":true,"is_default":false,"name":"USB Mic","transport":"usb","uid":"mic-2"}],"type":"microphone_devices"}"#.utf8
        )
    )
    #expect(
        devicesHeader == .microphoneDevices(
            MicrophoneDevicesMessage(
                devices: [
                    MicrophoneDevice(
                        uid: "mic-1",
                        name: "Built-in Microphone",
                        isDefault: true,
                        isConnected: true,
                        transport: .builtIn
                    ),
                    MicrophoneDevice(
                        uid: "mic-2",
                        name: "USB Mic",
                        isDefault: false,
                        isConnected: true,
                        transport: .usb
                    ),
                ]
            )
        )
    )

    #expect(throws: CaptureProtocolError.invalidHeader) {
        try CanonicalJSON.decode(
            Data(
                #"{"devices":[{"is_connected":true,"is_default":true,"name":"Built-in Microphone","transport":"built_in","uid":"mic-1"},{"is_connected":true,"is_default":false,"name":"USB Mic","transport":"usb","uid":"mic-1"}],"type":"microphone_devices"}"#.utf8
            )
        )
    }

    #expect(throws: CaptureProtocolError.invalidHeader) {
        try CanonicalJSON.decode(
            Data(
                #"{"devices":[{"is_connected":true,"is_default":true,"name":"Built-in Microphone","transport":"built_in","uid":""}],"type":"microphone_devices"}"#.utf8
            )
        )
    }

    for invalidJSON in [
        #"{"devices":[{"is_connected":true,"is_default":true,"name":"","transport":"built_in","uid":"mic-1"}],"type":"microphone_devices"}"#,
        #"{"devices":[{"extra":true,"is_connected":true,"is_default":true,"name":"Built-in Microphone","transport":"built_in","uid":"mic-1"}],"type":"microphone_devices"}"#,
        #"{"devices":[{"is_connected":false,"is_default":true,"name":"Built-in Microphone","transport":"built_in","uid":"mic-1"}],"type":"microphone_devices"}"#,
        #"{"devices":[{"is_connected":true,"is_default":true,"name":"Built-in Microphone","transport":"airplay","uid":"mic-1"}],"type":"microphone_devices"}"#,
        #"{"devices":[{"is_connected":true,"isDefault":true,"name":"Built-in Microphone","transport":"built_in","uid":"mic-1"}],"type":"microphone_devices"}"#,
        #"{"devices":[{"is_connected":true,"is__default":true,"name":"Built-in Microphone","transport":"built_in","uid":"mic-1"}],"type":"microphone_devices"}"#,
    ] {
        #expect(throws: CaptureProtocolError.invalidHeader) {
            try CanonicalJSON.decode(Data(invalidJSON.utf8))
        }
    }

    for (wireValue, expected) in [
        ("built_in", MicrophoneTransport.builtIn),
        ("usb", .usb),
        ("bluetooth", .bluetooth),
        ("aggregate", .aggregate),
        ("virtual", .virtual),
        ("unknown", .unknown),
    ] {
        let decoded = try CanonicalJSON.decode(
            Data(
                #"{"devices":[{"is_connected":true,"is_default":false,"name":"Test Microphone","transport":"\#(wireValue)","uid":"mic-1"}],"type":"microphone_devices"}"#.utf8
            )
        )
        #expect(
            decoded == .microphoneDevices(
                MicrophoneDevicesMessage(
                    devices: [
                        MicrophoneDevice(
                            uid: "mic-1",
                            name: "Test Microphone",
                            isDefault: false,
                            isConnected: true,
                            transport: expected
                        ),
                    ]
                )
            )
        )
    }

    let oversizedDevices = (0 ... ProtocolLimits.maxMicrophoneDevices).map { index in
        MicrophoneDevice(
            uid: "mic-\(index)",
            name: "Microphone \(index)",
            isDefault: index == 0,
            isConnected: true,
            transport: .unknown
        )
    }
    #expect(throws: CaptureProtocolError.invalidHeader) {
        try CanonicalJSON.encode(.microphoneDevices(MicrophoneDevicesMessage(devices: oversizedDevices)))
    }
}

@Test func rejectsOversizedPayloadBeforeReadingIt() throws {
    let audio = CaptureHeader.audioFrame(
        AudioFrame(source: .microphone, sequence: 1, samplePosition: 0, hostTime: 1, format: .s16Le, channelCount: 1)
    )
    let header = try CanonicalJSON.encode(audio)
    var frame = Data()
    frame.appendUInt32BE(UInt32(header.count))
    frame.append(header)
    frame.appendUInt32BE(UInt32(ProtocolLimits.maxPayloadBytes + 1))

    #expect(throws: CaptureProtocolError.payloadTooLarge) {
        try FrameCodec.decode(frame)
    }
}

@Test func rejectsPayloadForNonAudioMessages() throws {
    let payload = Data([0])
    #expect(throws: CaptureProtocolError.unexpectedPayload) {
        try FrameCodec.encode(header: hello, payload: payload)
    }

    let header = try CanonicalJSON.encode(hello)
    var frame = Data()
    frame.appendUInt32BE(UInt32(header.count))
    frame.append(header)
    frame.appendUInt32BE(UInt32(payload.count))
    frame.append(payload)
    #expect(throws: CaptureProtocolError.unexpectedPayload) {
        try FrameCodec.decode(frame)
    }
}

@Test func rejectsTruncatedDeclaredLengths() throws {
    var truncatedHeader = Data()
    truncatedHeader.appendUInt32BE(10)
    truncatedHeader.append(Data("{}".utf8))
    #expect(throws: CaptureProtocolError.truncatedFrame) {
        try FrameCodec.decode(truncatedHeader)
    }

    let audio = CaptureHeader.audioFrame(
        AudioFrame(source: .microphone, sequence: 1, samplePosition: 0, hostTime: 1, format: .s16Le, channelCount: 1)
    )
    let header = try CanonicalJSON.encode(audio)
    var truncatedPayload = Data()
    truncatedPayload.appendUInt32BE(UInt32(header.count))
    truncatedPayload.append(header)
    truncatedPayload.appendUInt32BE(4)
    truncatedPayload.append(contentsOf: [0, 1])
    #expect(throws: CaptureProtocolError.truncatedFrame) {
        try FrameCodec.decode(truncatedPayload)
    }
}

@Test func rejectsHelloReplayAndNonMonotonicAudio() throws {
    var validator = ProtocolValidator()
    try validator.observe(hello)
    #expect(throws: CaptureProtocolError.helloReplay) {
        try validator.observe(hello)
    }

    #expect(throws: CaptureProtocolError.unsupportedVersion) {
        var incompatibleValidator = ProtocolValidator()
        try incompatibleValidator.observe(
            .hello(
                Hello(
                    protocolVersion: 1,
                    helperPid: 4242,
                    launchNonce: "0123456789abcdef",
                    supportedSources: [.microphone, .systemAudio],
                    capabilities: [.deviceDiscovery]
                )
            )
        )
    }

    #expect(throws: CaptureProtocolError.invalidHeader) {
        var missingCapabilityValidator = ProtocolValidator()
        try missingCapabilityValidator.observe(
            .hello(
                Hello(
                    protocolVersion: 2,
                    helperPid: 4242,
                    launchNonce: "0123456789abcdef",
                    supportedSources: [.microphone, .systemAudio],
                    capabilities: [.deviceDiscovery]
                )
            )
        )
    }

    let first = CaptureHeader.audioFrame(
        AudioFrame(source: .microphone, sequence: 7, samplePosition: 0, hostTime: 10, format: .s16Le, channelCount: 1)
    )
    let replay = CaptureHeader.audioFrame(
        AudioFrame(source: .microphone, sequence: 7, samplePosition: 160, hostTime: 20, format: .s16Le, channelCount: 1)
    )
    try validator.observe(first)
    #expect(throws: CaptureProtocolError.nonMonotonicSequence) {
        try validator.observe(replay)
    }
}

@Test func rejectsEmptyAndSampleMisalignedPcm() throws {
    let header = CaptureHeader.audioFrame(
        AudioFrame(source: .microphone, sequence: 1, samplePosition: 0, hostTime: 1, format: .s16Le, channelCount: 2)
    )
    for payload in [Data(), Data([0, 1, 2])] {
        #expect(throws: CaptureProtocolError.invalidAudioPayload) {
            try FrameCodec.encode(header: header, payload: payload)
        }
        let headerData = try CanonicalJSON.encode(header)
        var bytes = Data()
        bytes.appendUInt32BE(UInt32(headerData.count))
        bytes.append(headerData)
        bytes.appendUInt32BE(UInt32(payload.count))
        bytes.append(payload)
        #expect(throws: CaptureProtocolError.invalidAudioPayload) {
            try FrameCodec.decode(bytes)
        }
    }
}

@Test func prefixDecoderConsumesOneCoalescedSocketFrame() throws {
    let first = try FrameCodec.encode(header: hello, payload: Data())
    let second = try FrameCodec.encode(header: .shutdownAck, payload: Data())
    let decoded = try FrameCodec.decodePrefix(first + second)

    #expect(decoded.frame.header == hello)
    #expect(decoded.consumedBytes == first.count)
    #expect((first + second).dropFirst(decoded.consumedBytes) == second)
}

private extension Data {
    init(hex: String) throws {
        let characters = Array(hex.trimmingCharacters(in: .whitespacesAndNewlines))
        guard characters.count.isMultiple(of: 2) else {
            throw CaptureProtocolError.invalidHeader
        }
        self.init()
        reserveCapacity(characters.count / 2)
        for index in stride(from: 0, to: characters.count, by: 2) {
            guard let byte = UInt8(String(characters[index ... index + 1]), radix: 16) else {
                throw CaptureProtocolError.invalidHeader
            }
            append(byte)
        }
    }

    mutating func appendUInt32BE(_ value: UInt32) {
        append(UInt8((value >> 24) & 0xff))
        append(UInt8((value >> 16) & 0xff))
        append(UInt8((value >> 8) & 0xff))
        append(UInt8(value & 0xff))
    }
}
