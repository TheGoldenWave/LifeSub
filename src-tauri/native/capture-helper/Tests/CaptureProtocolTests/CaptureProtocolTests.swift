import Foundation
import Testing
@testable import CaptureProtocol

private let hello = CaptureHeader.hello(
    Hello(
        protocolVersion: 1,
        helperPid: 4242,
        launchNonce: "0123456789abcdef",
        supportedSources: [.microphone, .systemAudio]
    )
)

@Test func canonicalHelloFrameMatchesRustBytes() throws {
    let encoded = try FrameCodec.encode(header: hello, payload: Data())
    let json = #"{"helper_pid":4242,"launch_nonce":"0123456789abcdef","protocol_version":1,"supported_sources":["microphone","system_audio"],"type":"hello"}"#.data(using: .utf8)!
    var expected = Data()
    expected.appendUInt32BE(UInt32(json.count))
    expected.append(json)
    expected.appendUInt32BE(0)

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
        #"{"helper_pid":4242,"launch_nonce":"n","protocol_version":1,"supported_sources":["microphone"],"type":"hello","unexpected":true}"#,
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

private extension Data {
    mutating func appendUInt32BE(_ value: UInt32) {
        append(UInt8((value >> 24) & 0xff))
        append(UInt8((value >> 16) & 0xff))
        append(UInt8((value >> 8) & 0xff))
        append(UInt8(value & 0xff))
    }
}
