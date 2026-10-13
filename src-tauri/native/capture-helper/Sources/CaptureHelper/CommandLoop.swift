import CaptureProtocol
import AVFoundation
import Foundation

protocol CaptureFrameDestination: Sendable {
    func write(_ data: Data) throws
    func failClosed()
}

private struct FileHandleCaptureFrameDestination: CaptureFrameDestination, @unchecked Sendable {
    private let connection: FileHandle

    init(connection: FileHandle) {
        self.connection = connection
    }

    func write(_ data: Data) throws {
        try connection.write(contentsOf: data)
    }

    func failClosed() {
        connection.closeFile()
    }
}

private enum CaptureFrameWriteError: Error {
    case destinationFailed
}

actor OutboundCaptureFrameWriter {
    private let destination: any CaptureFrameDestination
    private var failed = false

    init(connection: FileHandle) {
        destination = FileHandleCaptureFrameDestination(connection: connection)
    }

    init(destination: any CaptureFrameDestination) {
        self.destination = destination
    }

    func send(_ header: CaptureHeader) async throws {
        guard !failed else {
            throw CaptureFrameWriteError.destinationFailed
        }
        do {
            try destination.write(FrameCodec.encode(header: header, payload: Data()))
        } catch {
            failed = true
            destination.failClosed()
            throw CaptureFrameWriteError.destinationFailed
        }
    }
}

private protocol OutboundCaptureFrameWriterProviding: Sendable {
    var outboundFrameWriter: OutboundCaptureFrameWriter { get }
}

struct SocketCaptureEventSink: CaptureEventSink {
    let writer: OutboundCaptureFrameWriter

    init(connection: FileHandle) {
        writer = OutboundCaptureFrameWriter(connection: connection)
    }

    init(writer: OutboundCaptureFrameWriter) {
        self.writer = writer
    }

    func emit(_ event: CaptureControllerEvent) async {
        let header: CaptureHeader
        switch event {
        case let .permissionState(microphone, screenRecording):
            header = .permissionState(PermissionStateMessage(microphone: microphone, screenRecording: screenRecording))
        case let .sourceStarted(source, deviceId, deviceName, sampleRate, channelCount, hostTime):
            header = .sourceStarted(SourceStarted(source: source, deviceId: deviceId, deviceName: deviceName, sampleRate: sampleRate, channelCount: channelCount, hostTime: hostTime))
        case let .sourceStopped(source, sequence, start, end):
            header = .sourceStopped(SourceStopped(source: source, finalSequence: sequence, startHostTime: start, endHostTime: end))
        case let .chunkSealed(source, path, length, sha256, duration, sampleStart, sampleEnd, hostStart, hostEnd):
            header = .chunkSealed(ChunkSealed(source: source, relativeStagingPath: path, byteLength: length, sha256: sha256, durationMs: duration, startSamplePosition: sampleStart, endSamplePosition: sampleEnd, startHostTime: hostStart, endHostTime: hostEnd))
        case let .sourceInterrupted(source, reason, recoverable):
            header = .sourceInterrupted(SourceInterrupted(source: source, reason: reason, recoverable: recoverable))
        case let .sourceRecovering(source, generation, reason):
            header = .sourceRecovering(
                SourceRecovering(source: source, generation: generation, reason: reason)
            )
        case let .sourceRecovered(source, generation, deviceId, firstHostTime):
            header = .sourceRecovered(
                SourceRecovered(
                    source: source,
                    generation: generation,
                    deviceId: deviceId,
                    firstHostTime: firstHostTime
                )
            )
        case let .permissionRevoked(source):
            header = .sourceInterrupted(SourceInterrupted(source: source, reason: "permission_revoked", recoverable: false))
        case let .fatalError(code, summary):
            header = .fatalError(FatalError(code: code, summary: summary))
        }
        do {
            try await writer.send(header)
        } catch {
            // The writer closes the duplex socket, which wakes the command loop so
            // it can shut down any active controller instead of continuing headless.
        }
    }
}

extension SocketCaptureEventSink: OutboundCaptureFrameWriterProviding {
    var outboundFrameWriter: OutboundCaptureFrameWriter { writer }
}

actor SocketChunkSink: SealedChunkSink {
    let events: any CaptureEventSink
    init(events: any CaptureEventSink) { self.events = events }
    func didSeal(_ chunk: SealedChunk) async {
        let duration = chunk.frameCount.multipliedReportingOverflow(by: 1_000).partialValue / 16_000
        await events.emit(.chunkSealed(
            source: chunk.source,
            relativePath: chunk.relativePath,
            byteLength: chunk.byteLength,
            sha256: chunk.sha256,
            durationMs: duration,
            startSamplePosition: chunk.startSamplePosition,
            endSamplePosition: chunk.endSamplePosition,
            startHostTime: chunk.startHostTime,
            endHostTime: chunk.endHostTime
        ))
    }
}

struct LiveCaptureSessionFactory: CaptureSessionControllerFactory, OutboundCaptureFrameWriterProviding {
    let outboundFrameWriter: OutboundCaptureFrameWriter

    init(events: SocketCaptureEventSink) {
        outboundFrameWriter = events.outboundFrameWriter
    }

    func makeController(
        stagingRoot: URL,
        eventSink: any CaptureEventSink,
        selectedMicrophoneUID _: String?
    ) async throws -> any HelperCaptureSessionControlling {
        let sink = SocketChunkSink(events: eventSink)
        let frameFailure: @Sendable (CaptureFailure) -> Void = { failure in
            Task { await eventSink.emit(.fatalError(code: failure.rawValue, summary: failure.rawValue)) }
        }
        let signalRouter = CaptureSourceSignalRouter()
        let microphoneSignal: @Sendable (CaptureSourceSignal) async -> Void = { signal in
            await signalRouter.handle(signal)
        }
        let systemSignal: @Sendable (CaptureSourceSignal) -> Void = { signal in
            Task { await signalRouter.handle(signal) }
        }
        var adapters: [any CaptureSourceAdapter] = []
        let root = stagingRoot.standardizedFileURL
        let microEncoder = try ChunkEncoder(source: .microphone, stagingRoot: root, sampleRate: 16_000, channelCount: 1, maxFramesPerChunk: 16_000 * 30, sink: sink)
        let systemEncoder = try ChunkEncoder(source: .systemAudio, stagingRoot: root, sampleRate: 16_000, channelCount: 1, maxFramesPerChunk: 16_000 * 30, sink: sink)
        adapters.append(MicrophoneCapture(encoder: microEncoder, failureHandler: frameFailure, signalHandler: microphoneSignal))
        adapters.append(SystemAudioCapture(encoder: systemEncoder, failureHandler: frameFailure, signalHandler: systemSignal))
        let controller = CaptureSessionController(sources: adapters, eventSink: eventSink)
        await signalRouter.install(controller: controller)
        return controller
    }
}

private actor CaptureSourceSignalRouter {
    private weak var controller: CaptureSessionController?

    func install(controller: CaptureSessionController) {
        self.controller = controller
    }

    func handle(_ signal: CaptureSourceSignal) async {
        guard let controller else { return }
        switch signal {
        case let .interrupted(source, reason, recoverable):
            await controller.sourceInterrupted(source, reason: reason, recoverable: recoverable)
        case let .permissionRevoked(source):
            await controller.permissionRevoked(source)
        case let .recovering(source, generation, reason):
            await controller.sourceRecovering(source, generation: generation, reason: reason)
        case let .recovered(source, generation, deviceId, firstHostTime):
            await controller.sourceRecovered(
                source,
                generation: generation,
                deviceId: deviceId,
                firstHostTime: firstHostTime
            )
        }
    }
}

struct HelperCommandLoop: Sendable {
    let connection: FileHandle
    let nonceHex: String
    let controllerFactory: any CaptureSessionControllerFactory
    let microphoneDeviceCatalog: any AudioInputDeviceCatalog
    let microphoneProberFactory: @Sendable () -> any MicrophoneProbing
    let permissionStateProvider: any PermissionStateProviding
    let outboundFrameWriter: OutboundCaptureFrameWriter

    init(
        connection: FileHandle,
        nonceHex: String,
        controllerFactory: any CaptureSessionControllerFactory,
        microphoneDeviceCatalog: any AudioInputDeviceCatalog = CoreAudioInputDeviceCatalog(),
        microphoneProberFactory: @escaping @Sendable () -> any MicrophoneProbing = { MicrophoneProbe() },
        permissionStateProvider: any PermissionStateProviding = LivePermissionStateProvider()
    ) {
        self.connection = connection
        self.nonceHex = nonceHex
        self.controllerFactory = controllerFactory
        self.microphoneDeviceCatalog = microphoneDeviceCatalog
        self.microphoneProberFactory = microphoneProberFactory
        self.permissionStateProvider = permissionStateProvider
        outboundFrameWriter = (controllerFactory as? any OutboundCaptureFrameWriterProviding)?.outboundFrameWriter
            ?? OutboundCaptureFrameWriter(connection: connection)
    }

    func run() async throws {
        let writer = outboundFrameWriter
        try await writer.send(
            .hello(
                Hello(
                    protocolVersion: ProtocolLimits.version,
                    helperPid: UInt32(ProcessInfo.processInfo.processIdentifier),
                    launchNonce: nonceHex,
                    supportedSources: [.microphone, .systemAudio],
                    capabilities: [.deviceDiscovery, .exactMicrophoneSelection, .microphoneProbe, .permissionDiagnostics]
                )
            )
        )
        await Task.yield()
        let sink = SocketCaptureEventSink(writer: writer)
        var controller: (any HelperCaptureSessionControlling)?
        var lifecycleState = HelperCommandLifecycleState.awaitingFirstCommand
        while true {
            let header: CaptureHeader
            do {
                guard let next = try readCommandFrame() else { break }
                header = next
            } catch {
                await controller?.shutdown()
                controller = nil
                await sink.emit(.fatalError(code: "invalid_control_frame", summary: "invalid helper command"))
                return
            }
            do {
                switch header {
                    case let .start(command):
                        guard lifecycleState == .awaitingFirstCommand,
                              command.protocolVersion == ProtocolLimits.version,
                              !command.requestedSources.isEmpty
                        else { throw CaptureProtocolError.invalidHeader }
                        let value = try await controllerFactory.makeController(
                            stagingRoot: URL(fileURLWithPath: command.stagingRoot),
                            eventSink: sink,
                            selectedMicrophoneUID: command.microphoneDeviceUid
                        )
                        controller = value
                        try await value.start(
                            requested: Set(command.requestedSources),
                            selectedMicrophoneUID: command.microphoneDeviceUid
                        )
                        lifecycleState = .capturing
                    case let .listMicrophones(command):
                        guard lifecycleState == .awaitingFirstCommand,
                              command.protocolVersion == ProtocolLimits.version
                        else {
                            throw CaptureProtocolError.invalidHeader
                        }
                        lifecycleState = .discovering
                        do {
                            let devices = try AudioInputDevices(catalog: microphoneDeviceCatalog).listMicrophones()
                            try await writer.send(.microphoneDevices(MicrophoneDevicesMessage(devices: devices)))
                            lifecycleState = .stopped
                        } catch {
                            await sink.emit(
                                .fatalError(
                                    code: "microphone_enumeration_failed",
                                    summary: "microphone enumeration failed"
                                )
                            )
                            return
                        }
                    case let .probeMicrophone(command):
                        guard lifecycleState == .awaitingFirstCommand,
                              command.protocolVersion == ProtocolLimits.version
                        else {
                            throw CaptureProtocolError.invalidHeader
                        }
                        lifecycleState = .probing
                        let result = await microphoneProberFactory().probe(uid: command.microphoneDeviceUid)
                        try await writer.send(.microphoneProbeResult(result))
                        lifecycleState = .stopped
                    case let .getPermissionState(command):
                        guard lifecycleState == .awaitingFirstCommand,
                              command.protocolVersion == ProtocolLimits.version
                        else { throw CaptureProtocolError.invalidHeader }
                        lifecycleState = .diagnosing
                        try await writer.send(
                            .permissionState(permissionStateProvider.currentPermissionState())
                        )
                        lifecycleState = .stopped
                    case let .pause(command):
                        guard lifecycleState == .capturing,
                              command.protocolVersion == ProtocolLimits.version,
                              let controller
                        else { throw CaptureProtocolError.invalidHeader }
                        try await controller.pause()
                    case let .resume(command):
                        guard lifecycleState == .capturing,
                              command.protocolVersion == ProtocolLimits.version,
                              let controller
                        else { throw CaptureProtocolError.invalidHeader }
                        try await controller.resume()
                    case let .stop(command):
                        guard lifecycleState == .capturing,
                              command.protocolVersion == ProtocolLimits.version,
                              let controller
                        else { throw CaptureProtocolError.invalidHeader }
                        try await controller.stop()
                        lifecycleState = .stopped
                    case let .shutdown(command):
                        guard command.protocolVersion == ProtocolLimits.version else { throw CaptureProtocolError.invalidHeader }
                        await controller?.shutdown()
                        controller = nil
                        lifecycleState = .stopped
                        try await writer.send(.shutdownAck)
                        return
                    default: throw CaptureProtocolError.invalidHeader
                }
            } catch {
                await controller?.shutdown()
                controller = nil
                if let sourceError = error as? CaptureSourceError {
                    let fatal = fatalError(for: sourceError)
                    await sink.emit(.fatalError(code: fatal.code, summary: fatal.summary))
                } else {
                    await sink.emit(.fatalError(code: "invalid_control_frame", summary: "helper command failed"))
                }
                return
            }
        }
        await controller?.shutdown()
    }

    private func fatalError(for error: CaptureSourceError) -> (code: String, summary: String) {
        switch error {
            case .permissionDenied(source: .microphone):
                ("microphone_permission_denied", "麦克风权限未授权。")
            case .permissionDenied(source: .systemAudio):
                ("screen_recording_permission_denied", "录屏与系统录音权限未授权。")
            case .permissionRevoked:
                ("microphone_permission_revoked", "麦克风权限已被撤销。")
            case .deviceUnavailable:
                ("capture_device_unavailable", "录音设备当前不可用。")
            case .selectedMicrophoneUnavailable:
                ("selected_microphone_unavailable", "所选麦克风当前不可用。")
            case .selectedMicrophoneIdentityMismatch:
                ("selected_microphone_identity_mismatch", "所选麦克风与实际启动设备不一致。")
            case .selectedMicrophoneDisconnected:
                ("selected_microphone_disconnected", "所选麦克风已断开连接。")
            case .sourceFailed:
                ("capture_source_failed", "录音源启动失败。")
        }
    }

    private func readCommandFrame() throws -> CaptureHeader? {
        guard let headerLengthBytes = try readExactly(4) else { return nil }
        let headerLength = Int(headerLengthBytes.reduce(UInt32(0)) { ($0 << 8) | UInt32($1) })
        guard headerLength <= ProtocolLimits.maxHeaderBytes else {
            throw CaptureProtocolError.headerTooLarge
        }
        guard let header = try readExactly(headerLength),
              let payloadLengthBytes = try readExactly(4)
        else { throw CaptureProtocolError.truncatedFrame }
        let payloadLength = Int(payloadLengthBytes.reduce(UInt32(0)) { ($0 << 8) | UInt32($1) })
        guard payloadLength <= ProtocolLimits.maxPayloadBytes else {
            throw CaptureProtocolError.payloadTooLarge
        }
        guard let payload = try readExactly(payloadLength) else {
            throw CaptureProtocolError.truncatedFrame
        }

        var frame = Data(capacity: 8 + header.count + payload.count)
        frame.append(headerLengthBytes)
        frame.append(header)
        frame.append(payloadLengthBytes)
        frame.append(payload)
        return try FrameCodec.decode(frame).header
    }

    private func readExactly(_ count: Int) throws -> Data? {
        if count == 0 { return Data() }
        var result = Data(capacity: count)
        while result.count < count {
            guard let bytes = try connection.read(upToCount: count - result.count), !bytes.isEmpty else {
                if result.isEmpty { return nil }
                throw CaptureProtocolError.truncatedFrame
            }
            result.append(bytes)
        }
        return result
    }
}

private enum HelperCommandLifecycleState {
    case awaitingFirstCommand
    case discovering
    case probing
    case diagnosing
    case capturing
    case stopped
}
