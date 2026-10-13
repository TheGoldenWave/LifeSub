import Foundation

public enum ProtocolLimits {
    public static let version: UInt16 = 2
    public static let maxHeaderBytes = 64 * 1024
    public static let maxPayloadBytes = 4 * 1024 * 1024
    public static let maxMicrophoneDevices = 32
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
    case notGranted = "not_granted"
    case denied
    case restricted
    case granted
}

public enum HelloCapability: String, Codable, CaseIterable, Hashable, Sendable {
    case deviceDiscovery = "device_discovery"
    case exactMicrophoneSelection = "exact_microphone_selection"
    case microphoneProbe = "microphone_probe"
    case permissionDiagnostics = "permission_diagnostics"
}

public enum MicrophoneTransport: String, Codable, CaseIterable, Hashable, Sendable {
    case builtIn = "built_in"
    case usb
    case bluetooth
    case aggregate
    case virtual
    case unknown
}

public struct Hello: Codable, Equatable, Sendable {
    public let protocolVersion: UInt16
    public let helperPid: UInt32
    public let launchNonce: String
    public let supportedSources: [Source]
    public let capabilities: [HelloCapability]

    public init(
        protocolVersion: UInt16,
        helperPid: UInt32,
        launchNonce: String,
        supportedSources: [Source],
        capabilities: [HelloCapability] = [.deviceDiscovery, .exactMicrophoneSelection, .microphoneProbe, .permissionDiagnostics]
    ) {
        self.protocolVersion = protocolVersion
        self.helperPid = helperPid
        self.launchNonce = launchNonce
        self.supportedSources = supportedSources
        self.capabilities = capabilities
    }
}

public struct ListMicrophonesCommand: Codable, Equatable, Sendable {
    public let protocolVersion: UInt16

    public init(protocolVersion: UInt16) {
        self.protocolVersion = protocolVersion
    }
}

public struct ProbeMicrophoneCommand: Codable, Equatable, Sendable {
    public let protocolVersion: UInt16
    public let microphoneDeviceUid: String

    public init(protocolVersion: UInt16, microphoneDeviceUid: String) {
        self.protocolVersion = protocolVersion
        self.microphoneDeviceUid = microphoneDeviceUid
    }
}

public struct GetPermissionStateCommand: Codable, Equatable, Sendable {
    public let protocolVersion: UInt16
    public init(protocolVersion: UInt16) { self.protocolVersion = protocolVersion }
}

public struct MicrophoneDevice: Codable, Equatable, Sendable {
    public let uid: String
    public let name: String
    public let isDefault: Bool
    public let isConnected: Bool
    public let transport: MicrophoneTransport

    public init(uid: String, name: String, isDefault: Bool, isConnected: Bool, transport: MicrophoneTransport) {
        self.uid = uid
        self.name = name
        self.isDefault = isDefault
        self.isConnected = isConnected
        self.transport = transport
    }

    enum CodingKeys: String, CodingKey {
        case uid
        case name
        case isDefault
        case isConnected
        case transport
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let uid = try container.decode(String.self, forKey: .uid)
        guard !uid.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw CaptureProtocolError.invalidHeader
        }
        self.uid = uid
        let name = try container.decode(String.self, forKey: .name)
        guard !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw CaptureProtocolError.invalidHeader
        }
        self.name = name
        isDefault = try container.decode(Bool.self, forKey: .isDefault)
        let isConnected = try container.decode(Bool.self, forKey: .isConnected)
        guard isConnected else { throw CaptureProtocolError.invalidHeader }
        self.isConnected = isConnected
        transport = try container.decode(MicrophoneTransport.self, forKey: .transport)
    }
}

public struct MicrophoneDevicesMessage: Codable, Equatable, Sendable {
    public let devices: [MicrophoneDevice]

    public init(devices: [MicrophoneDevice]) {
        self.devices = devices
    }

    enum CodingKeys: String, CodingKey { case devices }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let decoded = try container.decode([MicrophoneDevice].self, forKey: .devices)
        guard decoded.count <= ProtocolLimits.maxMicrophoneDevices else {
            throw CaptureProtocolError.invalidHeader
        }
        var seen = Set<String>()
        for device in decoded {
            guard seen.insert(device.uid).inserted else {
                throw CaptureProtocolError.invalidHeader
            }
        }
        devices = decoded
    }
}

public enum MicrophoneProbeResult: String, Codable, CaseIterable, Sendable {
    case compatible
    case permissionDenied = "permission_denied"
    case unavailable
    case identityMismatch = "identity_mismatch"
    case invalidFormat = "invalid_format"
    case noAudioFrames = "no_audio_frames"
    case routeConflict = "route_conflict"
    case timeout
}

public struct MicrophoneProbeMessage: Codable, Equatable, Sendable {
    public let uid: String
    public let name: String
    public let transport: MicrophoneTransport
    public let inputSampleRateHz: UInt32?
    public let inputChannelCount: UInt16?
    public let readBackUid: String?
    public let firstFrameSampleCount: UInt64
    public let elapsedMs: UInt64
    public let result: MicrophoneProbeResult

    public init(
        uid: String,
        name: String,
        transport: MicrophoneTransport,
        inputSampleRateHz: UInt32?,
        inputChannelCount: UInt16?,
        readBackUid: String?,
        firstFrameSampleCount: UInt64,
        elapsedMs: UInt64,
        result: MicrophoneProbeResult
    ) {
        self.uid = uid
        self.name = name
        self.transport = transport
        self.inputSampleRateHz = inputSampleRateHz
        self.inputChannelCount = inputChannelCount
        self.readBackUid = readBackUid
        self.firstFrameSampleCount = firstFrameSampleCount
        self.elapsedMs = elapsedMs
        self.result = result
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
    public let deviceName: String
    public let sampleRate: UInt32
    public let channelCount: UInt16
    public let hostTime: UInt64

    public init(source: Source, deviceId: String, deviceName: String, sampleRate: UInt32, channelCount: UInt16, hostTime: UInt64) {
        self.source = source
        self.deviceId = deviceId
        self.deviceName = deviceName
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
    public let sha256: String
    public let durationMs: UInt64
    public let startSamplePosition: UInt64
    public let endSamplePosition: UInt64
    public let startHostTime: UInt64
    public let endHostTime: UInt64

    public init(source: Source, relativeStagingPath: String, byteLength: UInt64, sha256: String, durationMs: UInt64, startSamplePosition: UInt64, endSamplePosition: UInt64, startHostTime: UInt64, endHostTime: UInt64) {
        self.source = source
        self.relativeStagingPath = relativeStagingPath
        self.byteLength = byteLength
        self.sha256 = sha256
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

public struct SourceRecovering: Codable, Equatable, Sendable {
    public let source: Source
    public let generation: UInt64
    public let reason: String

    public init(source: Source, generation: UInt64, reason: String) {
        self.source = source
        self.generation = generation
        self.reason = reason
    }
}

public struct SourceRecovered: Codable, Equatable, Sendable {
    public let source: Source
    public let generation: UInt64
    public let deviceId: String
    public let firstHostTime: UInt64

    public init(source: Source, generation: UInt64, deviceId: String, firstHostTime: UInt64) {
        self.source = source
        self.generation = generation
        self.deviceId = deviceId
        self.firstHostTime = firstHostTime
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
    case start(StartCaptureCommand)
    case listMicrophones(ListMicrophonesCommand)
    case probeMicrophone(ProbeMicrophoneCommand)
    case getPermissionState(GetPermissionStateCommand)
    case pause(PauseCaptureCommand)
    case resume(ResumeCaptureCommand)
    case stop(StopCaptureCommand)
    case shutdown(ShutdownCaptureCommand)
    case hello(Hello)
    case permissionState(PermissionStateMessage)
    case sourceStarted(SourceStarted)
    case microphoneDevices(MicrophoneDevicesMessage)
    case microphoneProbeResult(MicrophoneProbeMessage)
    case audioFrame(AudioFrame)
    case level(Level)
    case chunkSealed(ChunkSealed)
    case sourceInterrupted(SourceInterrupted)
    case sourceRecovering(SourceRecovering)
    case sourceRecovered(SourceRecovered)
    case sourceStopped(SourceStopped)
    case fatalError(FatalError)
    case shutdownAck
}

public struct StartCaptureCommand: Codable, Equatable, Sendable {
    public let protocolVersion: UInt16
    public let requestedSources: [Source]
    public let stagingRoot: String
    public let microphoneDeviceUid: String?
    public init(protocolVersion: UInt16, requestedSources: [Source], stagingRoot: String, microphoneDeviceUid: String? = nil) {
        self.protocolVersion = protocolVersion
        self.requestedSources = requestedSources
        self.stagingRoot = stagingRoot
        self.microphoneDeviceUid = microphoneDeviceUid
    }
}
public struct PauseCaptureCommand: Codable, Equatable, Sendable {
    public let protocolVersion: UInt16
    public init(protocolVersion: UInt16) { self.protocolVersion = protocolVersion }
}
public struct ResumeCaptureCommand: Codable, Equatable, Sendable {
    public let protocolVersion: UInt16
    public init(protocolVersion: UInt16) { self.protocolVersion = protocolVersion }
}
public struct StopCaptureCommand: Codable, Equatable, Sendable {
    public let protocolVersion: UInt16
    public init(protocolVersion: UInt16) { self.protocolVersion = protocolVersion }
}
public struct ShutdownCaptureCommand: Codable, Equatable, Sendable {
    public let protocolVersion: UInt16
    public init(protocolVersion: UInt16) { self.protocolVersion = protocolVersion }
}

public typealias CaptureCommand = CaptureHeader

public enum CommandJSON {
    public static func encode(_ command: CaptureCommand) throws -> Data {
        switch command {
        case .start, .listMicrophones, .probeMicrophone, .getPermissionState, .pause, .resume, .stop, .shutdown:
            return try CanonicalJSON.encode(command)
        default:
            throw CaptureProtocolError.invalidHeader
        }
    }

    public static func decode(_ data: Data) throws -> CaptureCommand {
        let command = try CanonicalJSON.decode(data)
        switch command {
        case .start, .listMicrophones, .probeMicrophone, .getPermissionState, .pause, .resume, .stop, .shutdown:
            return command
        default:
            throw CaptureProtocolError.invalidHeader
        }
    }
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
        try validate(header)
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
            let schema = schemasByType[type],
            schema.accepts(Set(object.keys))
        else {
            throw CaptureProtocolError.invalidHeader
        }

        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        if type == "microphone_devices" {
            try validateMicrophoneDeviceWireKeys(object)
        }
        let normalized = try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
        do {
            let header: CaptureHeader
            switch type {
            case "start": header = .start(try decoder.decode(StartCaptureCommand.self, from: normalized))
            case "list_microphones": header = .listMicrophones(try decoder.decode(ListMicrophonesCommand.self, from: normalized))
            case "probe_microphone": header = .probeMicrophone(try decoder.decode(ProbeMicrophoneCommand.self, from: normalized))
            case "get_permission_state": header = .getPermissionState(try decoder.decode(GetPermissionStateCommand.self, from: normalized))
            case "pause": header = .pause(try decoder.decode(PauseCaptureCommand.self, from: normalized))
            case "resume": header = .resume(try decoder.decode(ResumeCaptureCommand.self, from: normalized))
            case "stop": header = .stop(try decoder.decode(StopCaptureCommand.self, from: normalized))
            case "shutdown": header = .shutdown(try decoder.decode(ShutdownCaptureCommand.self, from: normalized))
            case "hello": header = .hello(try decoder.decode(Hello.self, from: normalized))
            case "permission_state": header = .permissionState(try decoder.decode(PermissionStateMessage.self, from: normalized))
            case "source_started": header = .sourceStarted(try decoder.decode(SourceStarted.self, from: normalized))
            case "microphone_devices": header = .microphoneDevices(try decoder.decode(MicrophoneDevicesMessage.self, from: normalized))
            case "microphone_probe_result": header = .microphoneProbeResult(try decoder.decode(MicrophoneProbeMessage.self, from: normalized))
            case "audio_frame": header = .audioFrame(try decoder.decode(AudioFrame.self, from: normalized))
            case "level": header = .level(try decoder.decode(Level.self, from: normalized))
            case "chunk_sealed": header = .chunkSealed(try decoder.decode(ChunkSealed.self, from: normalized))
            case "source_interrupted": header = .sourceInterrupted(try decoder.decode(SourceInterrupted.self, from: normalized))
            case "source_recovering": header = .sourceRecovering(try decoder.decode(SourceRecovering.self, from: normalized))
            case "source_recovered": header = .sourceRecovered(try decoder.decode(SourceRecovered.self, from: normalized))
            case "source_stopped": header = .sourceStopped(try decoder.decode(SourceStopped.self, from: normalized))
            case "fatal_error": header = .fatalError(try decoder.decode(FatalError.self, from: normalized))
            case "shutdown_ack": header = .shutdownAck
            default: throw CaptureProtocolError.invalidHeader
            }
            try validate(header)
            return header
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
        case let .start(value): return try object(for: value, type: "start")
        case let .listMicrophones(value): return try object(for: value, type: "list_microphones")
        case let .probeMicrophone(value): return try object(for: value, type: "probe_microphone")
        case let .getPermissionState(value): return try object(for: value, type: "get_permission_state")
        case let .pause(value): return try object(for: value, type: "pause")
        case let .resume(value): return try object(for: value, type: "resume")
        case let .stop(value): return try object(for: value, type: "stop")
        case let .shutdown(value): return try object(for: value, type: "shutdown")
        case let .hello(value): return try object(for: value, type: "hello")
        case let .permissionState(value): return try object(for: value, type: "permission_state")
        case let .sourceStarted(value): return try object(for: value, type: "source_started")
        case let .microphoneDevices(value): return try object(for: value, type: "microphone_devices")
        case let .microphoneProbeResult(value): return try object(for: value, type: "microphone_probe_result")
        case let .audioFrame(value): return try object(for: value, type: "audio_frame")
        case let .level(value): return try object(for: value, type: "level")
        case let .chunkSealed(value): return try object(for: value, type: "chunk_sealed")
        case let .sourceInterrupted(value): return try object(for: value, type: "source_interrupted")
        case let .sourceRecovering(value): return try object(for: value, type: "source_recovering")
        case let .sourceRecovered(value): return try object(for: value, type: "source_recovered")
        case let .sourceStopped(value): return try object(for: value, type: "source_stopped")
        case let .fatalError(value): return try object(for: value, type: "fatal_error")
        case .shutdownAck: return ["type": "shutdown_ack"]
        }
    }

    private struct KeySchema {
        let required: Set<String>
        let allowed: Set<String>

        init(_ required: Set<String>, optional: Set<String> = []) {
            self.required = required
            allowed = required.union(optional)
        }

        func accepts(_ keys: Set<String>) -> Bool {
            required.isSubset(of: keys) && keys.isSubset(of: allowed)
        }
    }

    private static let schemasByType: [String: KeySchema] = [
        "start": KeySchema(["type", "protocol_version", "requested_sources", "staging_root"], optional: ["microphone_device_uid"]),
        "list_microphones": KeySchema(["type", "protocol_version"]),
        "probe_microphone": KeySchema(["type", "protocol_version", "microphone_device_uid"]),
        "get_permission_state": KeySchema(["type", "protocol_version"]),
        "pause": KeySchema(["type", "protocol_version"]),
        "resume": KeySchema(["type", "protocol_version"]),
        "stop": KeySchema(["type", "protocol_version"]),
        "shutdown": KeySchema(["type", "protocol_version"]),
        "hello": KeySchema(["type", "protocol_version", "helper_pid", "launch_nonce", "supported_sources", "capabilities"]),
        "permission_state": KeySchema(["type", "microphone", "screen_recording"]),
        "source_started": KeySchema(["type", "source", "device_id", "device_name", "sample_rate", "channel_count", "host_time"]),
        "microphone_devices": KeySchema(["type", "devices"]),
        "microphone_probe_result": KeySchema(
            ["type", "uid", "name", "transport", "first_frame_sample_count", "elapsed_ms", "result"],
            optional: ["input_sample_rate_hz", "input_channel_count", "read_back_uid"]
        ),
        "audio_frame": KeySchema(["type", "source", "sequence", "sample_position", "host_time", "format", "channel_count", "discontinuity_flags"]),
        "level": KeySchema(["type", "source", "rms", "peak"]),
        "chunk_sealed": KeySchema(["type", "source", "relative_staging_path", "byte_length", "sha256", "duration_ms", "start_sample_position", "end_sample_position", "start_host_time", "end_host_time"]),
        "source_interrupted": KeySchema(["type", "source", "reason", "recoverable"]),
        "source_recovering": KeySchema(["type", "source", "generation", "reason"]),
        "source_recovered": KeySchema(["type", "source", "generation", "device_id", "first_host_time"]),
        "source_stopped": KeySchema(["type", "source", "final_sequence", "start_host_time", "end_host_time"]),
        "fatal_error": KeySchema(["type", "code", "summary"]),
        "shutdown_ack": KeySchema(["type"]),
    ]

    private static func validate(_ header: CaptureHeader) throws {
        switch header {
        case let .start(command):
            try validateVersion(command.protocolVersion)
            try validateStart(command)
        case let .listMicrophones(command): try validateVersion(command.protocolVersion)
        case let .probeMicrophone(command):
            try validateVersion(command.protocolVersion)
            guard !command.microphoneDeviceUid.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                throw CaptureProtocolError.invalidHeader
            }
        case let .getPermissionState(command): try validateVersion(command.protocolVersion)
        case let .pause(command): try validateVersion(command.protocolVersion)
        case let .resume(command): try validateVersion(command.protocolVersion)
        case let .stop(command): try validateVersion(command.protocolVersion)
        case let .shutdown(command): try validateVersion(command.protocolVersion)
        case let .hello(value):
            try validateVersion(value.protocolVersion)
            let capabilities = Set(value.capabilities)
            guard capabilities.contains(.deviceDiscovery),
                  capabilities.contains(.exactMicrophoneSelection),
                  capabilities.contains(.microphoneProbe),
                  capabilities.contains(.permissionDiagnostics)
            else {
                throw CaptureProtocolError.invalidHeader
            }
        case let .microphoneDevices(message):
            guard message.devices.count <= ProtocolLimits.maxMicrophoneDevices else {
                throw CaptureProtocolError.invalidHeader
            }
            var uids = Set<String>()
            for device in message.devices {
                guard !device.uid.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
                      !device.name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
                      device.isConnected,
                      uids.insert(device.uid).inserted
                else { throw CaptureProtocolError.invalidHeader }
            }
        case let .microphoneProbeResult(message):
            try validateProbeResult(message)
        case let .sourceRecovering(value):
            guard value.generation > 0, !value.reason.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                throw CaptureProtocolError.invalidHeader
            }
        case let .sourceRecovered(value):
            guard value.generation > 0,
                  !value.deviceId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
                  value.firstHostTime > 0
            else { throw CaptureProtocolError.invalidHeader }
        default:
            break
        }
    }

    private static func validateProbeResult(_ message: MicrophoneProbeMessage) throws {
        guard !message.uid.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !message.name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else { throw CaptureProtocolError.invalidHeader }
        switch message.result {
        case .compatible:
            guard let rate = message.inputSampleRateHz, rate > 0,
                  let channels = message.inputChannelCount, channels > 0,
                  message.readBackUid == message.uid,
                  message.firstFrameSampleCount > 0
            else { throw CaptureProtocolError.invalidHeader }
        case .identityMismatch:
            guard let readBack = message.readBackUid,
                  !readBack.isEmpty,
                  readBack != message.uid,
                  message.firstFrameSampleCount == 0
            else { throw CaptureProtocolError.invalidHeader }
        case .noAudioFrames:
            guard let rate = message.inputSampleRateHz, rate > 0,
                  let channels = message.inputChannelCount, channels > 0,
                  message.readBackUid == message.uid,
                  message.firstFrameSampleCount == 0
            else { throw CaptureProtocolError.invalidHeader }
        default:
            guard message.firstFrameSampleCount == 0 else {
                throw CaptureProtocolError.invalidHeader
            }
        }
    }

    private static func validateMicrophoneDeviceWireKeys(_ object: [String: Any]) throws {
        guard let devices = object["devices"] as? [Any] else {
            throw CaptureProtocolError.invalidHeader
        }
        let expectedKeys: Set<String> = ["uid", "name", "is_default", "is_connected", "transport"]
        for value in devices {
            guard let device = value as? [String: Any], Set(device.keys) == expectedKeys else {
                throw CaptureProtocolError.invalidHeader
            }
        }
    }
}

private func validateStart(_ command: StartCaptureCommand) throws {
    guard !command.requestedSources.isEmpty else { throw CaptureProtocolError.invalidHeader }
    let microphoneRequested = command.requestedSources.contains(.microphone)
    switch (microphoneRequested, command.microphoneDeviceUid) {
    case (true, let uid?):
        guard !uid.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw CaptureProtocolError.invalidHeader
        }
    case (true, nil), (false, _?):
        throw CaptureProtocolError.invalidHeader
    case (false, nil):
        break
    }
}

private func validateVersion(_ version: UInt16) throws {
    guard version == ProtocolLimits.version else {
        throw CaptureProtocolError.unsupportedVersion
    }
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
            try validateVersion(value.protocolVersion)
            let capabilities = Set(value.capabilities)
        guard capabilities.contains(.deviceDiscovery),
              capabilities.contains(.exactMicrophoneSelection),
              capabilities.contains(.microphoneProbe),
              capabilities.contains(.permissionDiagnostics)
        else {
                throw CaptureProtocolError.invalidHeader
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
