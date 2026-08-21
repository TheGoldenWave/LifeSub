import Foundation

public enum ProtocolLimits {
    public static let version: UInt16 = 1
    public static let maxHeaderBytes = 64 * 1024
    public static let maxPayloadBytes = 4 * 1024 * 1024
}

public enum Source: String, Codable, CaseIterable, Hashable, Sendable {
    case microphone
    case systemAudio = "system_audio"
}

public enum PcmFormat: String, Codable, Sendable {
    case s16Le = "s16_le"
}

public enum DiscontinuityFlag: String, Codable, Sendable {
    case gap
    case deviceChange = "device_change"
    case permissionRevoked = "permission_revoked"
    case clockReset = "clock_reset"
    case droppedBuffers = "dropped_buffers"
}

public enum Permission: String, Codable, Sendable {
    case notDetermined = "not_determined"
    case denied
    case restricted
    case granted
}

public struct Hello: Codable, Equatable, Sendable {
    public let protocolVersion: UInt16
    public let helperPid: UInt32
    public let launchNonce: String
    public let supportedSources: [Source]

    public init(protocolVersion: UInt16, helperPid: UInt32, launchNonce: String, supportedSources: [Source]) {
        self.protocolVersion = protocolVersion
        self.helperPid = helperPid
        self.launchNonce = launchNonce
        self.supportedSources = supportedSources
    }
}

public struct PermissionStateMessage: Codable, Equatable, Sendable {
    public let microphone: Permission
    public let screenRecording: Permission

    public init(microphone: Permission, screenRecording: Permission) {
        self.microphone = microphone
        self.screenRecording = screenRecording
    }
}

public struct SourceStarted: Codable, Equatable, Sendable {
    public let source: Source
    public let deviceId: String
    public let sampleRate: UInt32
    public let channelCount: UInt16
    public let hostTime: UInt64

    public init(source: Source, deviceId: String, sampleRate: UInt32, channelCount: UInt16, hostTime: UInt64) {
        self.source = source
        self.deviceId = deviceId
        self.sampleRate = sampleRate
        self.channelCount = channelCount
        self.hostTime = hostTime
    }
}

public struct AudioFrame: Codable, Equatable, Sendable {
    public let source: Source
    public let sequence: UInt64
    public let samplePosition: UInt64
    public let hostTime: UInt64
    public let format: PcmFormat
    public let channelCount: UInt16
    public let discontinuityFlags: [DiscontinuityFlag]

    public init(source: Source, sequence: UInt64, samplePosition: UInt64, hostTime: UInt64, format: PcmFormat, channelCount: UInt16, discontinuityFlags: [DiscontinuityFlag] = []) {
        self.source = source
        self.sequence = sequence
        self.samplePosition = samplePosition
        self.hostTime = hostTime
        self.format = format
        self.channelCount = channelCount
        self.discontinuityFlags = discontinuityFlags
    }
}

public struct Level: Codable, Equatable, Sendable {
    public let source: Source
    public let rms: Float
    public let peak: Float

    public init(source: Source, rms: Float, peak: Float) {
        self.source = source
        self.rms = rms
        self.peak = peak
    }
}

public struct ChunkSealed: Codable, Equatable, Sendable {
    public let source: Source
    public let relativeStagingPath: String
    public let byteLength: UInt64
    public let durationMs: UInt64
    public let startSamplePosition: UInt64
    public let endSamplePosition: UInt64
    public let startHostTime: UInt64
    public let endHostTime: UInt64

    public init(source: Source, relativeStagingPath: String, byteLength: UInt64, durationMs: UInt64, startSamplePosition: UInt64, endSamplePosition: UInt64, startHostTime: UInt64, endHostTime: UInt64) {
        self.source = source
        self.relativeStagingPath = relativeStagingPath
        self.byteLength = byteLength
        self.durationMs = durationMs
        self.startSamplePosition = startSamplePosition
        self.endSamplePosition = endSamplePosition
        self.startHostTime = startHostTime
        self.endHostTime = endHostTime
    }
}

public struct SourceInterrupted: Codable, Equatable, Sendable {
    public let source: Source
    public let reason: String
    public let recoverable: Bool

    public init(source: Source, reason: String, recoverable: Bool) {
        self.source = source
        self.reason = reason
        self.recoverable = recoverable
    }
}

public struct SourceStopped: Codable, Equatable, Sendable {
    public let source: Source
    public let finalSequence: UInt64
    public let startHostTime: UInt64
    public let endHostTime: UInt64

    public init(source: Source, finalSequence: UInt64, startHostTime: UInt64, endHostTime: UInt64) {
        self.source = source
        self.finalSequence = finalSequence
        self.startHostTime = startHostTime
        self.endHostTime = endHostTime
    }
}

public struct FatalError: Codable, Equatable, Sendable {
    public let code: String
    public let summary: String

    public init(code: String, summary: String) {
        self.code = code
        self.summary = summary
    }
}

public enum CaptureHeader: Equatable, Sendable {
    case hello(Hello)
    case permissionState(PermissionStateMessage)
    case sourceStarted(SourceStarted)
    case audioFrame(AudioFrame)
    case level(Level)
    case chunkSealed(ChunkSealed)
    case sourceInterrupted(SourceInterrupted)
    case sourceStopped(SourceStopped)
    case fatalError(FatalError)
    case shutdownAck
}

public struct CaptureFrame: Equatable, Sendable {
    public let header: CaptureHeader
    public let payload: Data
}

public enum CaptureProtocolError: Error, Equatable {
    case headerTooLarge
    case payloadTooLarge
    case truncatedFrame
    case invalidHeader
    case unexpectedPayload
    case invalidAudioPayload
    case helloRequired
    case helloReplay
    case unsupportedVersion
    case nonMonotonicSequence
}

public enum CanonicalJSON {
    public static func encode(_ header: CaptureHeader) throws -> Data {
        let object = try object(for: header)
        guard JSONSerialization.isValidJSONObject(object) else {
            throw CaptureProtocolError.invalidHeader
        }
        return try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
    }

    public static func decode(_ data: Data) throws -> CaptureHeader {
        guard
            let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
            let type = object["type"] as? String,
            let allowedKeys = allowedKeysByType[type],
            Set(object.keys) == allowedKeys
        else {
            throw CaptureProtocolError.invalidHeader
        }

        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let normalized = try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
        do {
            switch type {
            case "hello": return .hello(try decoder.decode(Hello.self, from: normalized))
            case "permission_state": return .permissionState(try decoder.decode(PermissionStateMessage.self, from: normalized))
            case "source_started": return .sourceStarted(try decoder.decode(SourceStarted.self, from: normalized))
            case "audio_frame": return .audioFrame(try decoder.decode(AudioFrame.self, from: normalized))
            case "level": return .level(try decoder.decode(Level.self, from: normalized))
            case "chunk_sealed": return .chunkSealed(try decoder.decode(ChunkSealed.self, from: normalized))
            case "source_interrupted": return .sourceInterrupted(try decoder.decode(SourceInterrupted.self, from: normalized))
            case "source_stopped": return .sourceStopped(try decoder.decode(SourceStopped.self, from: normalized))
            case "fatal_error": return .fatalError(try decoder.decode(FatalError.self, from: normalized))
            case "shutdown_ack": return .shutdownAck
            default: throw CaptureProtocolError.invalidHeader
            }
        } catch let error as CaptureProtocolError {
            throw error
        } catch {
            throw CaptureProtocolError.invalidHeader
        }
    }

    private static func object<T: Encodable>(for value: T, type: String) throws -> [String: Any] {
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        let data = try encoder.encode(value)
        guard var object = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw CaptureProtocolError.invalidHeader
        }
        object["type"] = type
        return object
    }

    private static func object(for header: CaptureHeader) throws -> [String: Any] {
        switch header {
        case let .hello(value): return try object(for: value, type: "hello")
        case let .permissionState(value): return try object(for: value, type: "permission_state")
        case let .sourceStarted(value): return try object(for: value, type: "source_started")
        case let .audioFrame(value): return try object(for: value, type: "audio_frame")
        case let .level(value): return try object(for: value, type: "level")
        case let .chunkSealed(value): return try object(for: value, type: "chunk_sealed")
        case let .sourceInterrupted(value): return try object(for: value, type: "source_interrupted")
        case let .sourceStopped(value): return try object(for: value, type: "source_stopped")
        case let .fatalError(value): return try object(for: value, type: "fatal_error")
        case .shutdownAck: return ["type": "shutdown_ack"]
        }
    }

    private static let allowedKeysByType: [String: Set<String>] = [
        "hello": ["type", "protocol_version", "helper_pid", "launch_nonce", "supported_sources"],
        "permission_state": ["type", "microphone", "screen_recording"],
        "source_started": ["type", "source", "device_id", "sample_rate", "channel_count", "host_time"],
        "audio_frame": ["type", "source", "sequence", "sample_position", "host_time", "format", "channel_count", "discontinuity_flags"],
        "level": ["type", "source", "rms", "peak"],
        "chunk_sealed": ["type", "source", "relative_staging_path", "byte_length", "duration_ms", "start_sample_position", "end_sample_position", "start_host_time", "end_host_time"],
        "source_interrupted": ["type", "source", "reason", "recoverable"],
        "source_stopped": ["type", "source", "final_sequence", "start_host_time", "end_host_time"],
        "fatal_error": ["type", "code", "summary"],
        "shutdown_ack": ["type"],
    ]
}

public enum FrameCodec {
    public struct PrefixDecodeResult: Sendable {
        public let frame: CaptureFrame
        public let consumedBytes: Int
    }

    public static func encode(header: CaptureHeader, payload: Data) throws -> Data {
        let headerData = try CanonicalJSON.encode(header)
        guard headerData.count <= ProtocolLimits.maxHeaderBytes else {
            throw CaptureProtocolError.headerTooLarge
        }
        guard payload.count <= ProtocolLimits.maxPayloadBytes else {
            throw CaptureProtocolError.payloadTooLarge
        }
        guard case .audioFrame = header else {
            if !payload.isEmpty { throw CaptureProtocolError.unexpectedPayload }
            return framed(header: headerData, payload: payload)
        }
        try validateAudioPayload(header: header, payload: payload)
        return framed(header: headerData, payload: payload)
    }

    public static func decode(_ data: Data) throws -> CaptureFrame {
        let decoded = try decodePrefix(data)
        guard decoded.consumedBytes == data.count else {
            throw CaptureProtocolError.invalidHeader
        }
        return decoded.frame
    }

    public static func decodePrefix(_ data: Data) throws -> PrefixDecodeResult {
        var offset = 0
        let headerLength = try readLength(data, offset: &offset)
        guard headerLength <= ProtocolLimits.maxHeaderBytes else {
            throw CaptureProtocolError.headerTooLarge
        }
        let headerData = try read(data, count: headerLength, offset: &offset)
        let header = try CanonicalJSON.decode(headerData)
        let payloadLength = try readLength(data, offset: &offset)
        guard payloadLength <= ProtocolLimits.maxPayloadBytes else {
            throw CaptureProtocolError.payloadTooLarge
        }
        if case .audioFrame = header {
            // Audio payload is allowed only for audio_frame.
        } else if payloadLength != 0 {
            throw CaptureProtocolError.unexpectedPayload
        }
        let payload = try read(data, count: payloadLength, offset: &offset)
        try validateAudioPayload(header: header, payload: payload)
        return PrefixDecodeResult(
            frame: CaptureFrame(header: header, payload: payload),
            consumedBytes: offset
        )
    }

    private static func framed(header: Data, payload: Data) -> Data {
        var result = Data(capacity: 8 + header.count + payload.count)
        result.appendUInt32BE(UInt32(header.count))
        result.append(header)
        result.appendUInt32BE(UInt32(payload.count))
        result.append(payload)
        return result
    }

    private static func readLength(_ data: Data, offset: inout Int) throws -> Int {
        let bytes = try read(data, count: 4, offset: &offset)
        return bytes.reduce(0) { ($0 << 8) | Int($1) }
    }

    private static func read(_ data: Data, count: Int, offset: inout Int) throws -> Data {
        guard count >= 0, offset <= data.count, count <= data.count - offset else {
            throw CaptureProtocolError.truncatedFrame
        }
        let result = data.subdata(in: offset ..< offset + count)
        offset += count
        return result
    }

    private static func validateAudioPayload(header: CaptureHeader, payload: Data) throws {
        guard case let .audioFrame(frame) = header else { return }
        let bytesPerSample: Int
        switch frame.format {
        case .s16Le: bytesPerSample = 2
        }
        let (stride, overflow) = bytesPerSample.multipliedReportingOverflow(by: Int(frame.channelCount))
        guard !overflow, stride > 0, !payload.isEmpty, payload.count.isMultiple(of: stride) else {
            throw CaptureProtocolError.invalidAudioPayload
        }
    }
}

public struct ProtocolValidator: Sendable {
    private var helloSeen = false
    private var lastSequenceBySource: [Source: UInt64] = [:]

    public init() {}

    public mutating func observe(_ header: CaptureHeader) throws {
        switch header {
        case let .hello(value):
            guard !helloSeen else { throw CaptureProtocolError.helloReplay }
            guard value.protocolVersion == ProtocolLimits.version else {
                throw CaptureProtocolError.unsupportedVersion
            }
            helloSeen = true
        case let .audioFrame(value):
            guard helloSeen else { throw CaptureProtocolError.helloRequired }
            if let previous = lastSequenceBySource[value.source], value.sequence <= previous {
                throw CaptureProtocolError.nonMonotonicSequence
            }
            lastSequenceBySource[value.source] = value.sequence
        default:
            guard helloSeen else { throw CaptureProtocolError.helloRequired }
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
