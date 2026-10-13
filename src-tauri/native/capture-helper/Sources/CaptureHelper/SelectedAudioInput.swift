@preconcurrency import AVFoundation
import AudioToolbox
import CoreAudio
import Foundation
import os

struct ResolvedSelectedAudioInput: Equatable, Sendable {
    let deviceID: AudioDeviceID
    let uid: String
    let name: String
}

struct SelectedAudioInputFormat: Equatable, Sendable {
    let sampleRate: Double
    let channelCount: AVAudioChannelCount
}

enum SelectedAudioInputError: Error, Equatable, Sendable {
    case unavailable
    case identityMismatch
    case listenerRegistrationFailed
}

protocol SelectedAudioInputObserver: Sendable {
    func cancel()
}

protocol MicrophoneAudioInputNode {
    func hardwareInputFormat(forBus bus: AVAudioNodeBus) -> AVAudioFormat
    func installTap(
        onBus bus: AVAudioNodeBus,
        bufferSize: AVAudioFrameCount,
        format: AVAudioFormat?,
        block: @escaping AVAudioNodeTapBlock
    )
    func removeTap(onBus bus: AVAudioNodeBus)
}

protocol MicrophoneAudioEngine: Sendable {
    var inputNode: any MicrophoneAudioInputNode { get }
    func start() throws
    func stop()
    func pause()
    func addConfigurationChangeObserver(
        _ handler: @escaping @Sendable () -> Void
    ) -> any SelectedAudioInputObserver
}

protocol SelectedAudioInputManaging: Sendable {
    func resolveSelectedInput(uid: String) throws -> ResolvedSelectedAudioInput
    func bindSelectedInput(deviceID: AudioDeviceID) throws
    func currentBoundInput() throws -> ResolvedSelectedAudioInput
    func expectedInputFormat(deviceID: AudioDeviceID) throws -> SelectedAudioInputFormat
    func addChangeObservers(
        selectedDeviceID: AudioDeviceID,
        handler: @escaping @Sendable () -> Void
    ) throws -> [any SelectedAudioInputObserver]
}

enum MicrophoneInputFormatSettleError: Error, Equatable, Sendable {
    case timeout
}

func settledMicrophoneInputFormat(
    inputNode: any MicrophoneAudioInputNode,
    expected: SelectedAudioInputFormat,
    timeout: Duration,
    pollInterval: Duration = .milliseconds(10)
) async throws -> AVAudioFormat {
    let clock = ContinuousClock()
    let deadline = clock.now.advanced(by: timeout)
    while true {
        let format = inputNode.hardwareInputFormat(forBus: 0)
        if abs(format.sampleRate - expected.sampleRate) < 1,
           format.channelCount == expected.channelCount {
            return format
        }
        guard clock.now < deadline else {
            throw MicrophoneInputFormatSettleError.timeout
        }
        try await Task.sleep(for: pollInterval)
    }
}

extension AVAudioInputNode: MicrophoneAudioInputNode {
    func hardwareInputFormat(forBus bus: AVAudioNodeBus) -> AVAudioFormat {
        inputFormat(forBus: bus)
    }
}

final class LiveMicrophoneAudioEngine: MicrophoneAudioEngine, @unchecked Sendable {
    private static let teardownRetention = DispatchTimeInterval.milliseconds(300)
    private static let teardownRetainer = DelayedReleaseRetainer<AVAudioEngine>(
        label: "com.lifesub.capture.microphone-engine-teardown"
    )
    private let engineState = OSAllocatedUnfairLock(initialState: AVAudioEngine?.none)

    private var engine: AVAudioEngine {
        engineState.withLock { engine in
            if let engine { return engine }
            let created = AVAudioEngine()
            engine = created
            return created
        }
    }

    deinit {
        guard let engine = engineState.withLock({ $0 }) else { return }
        Self.teardownRetainer.retain(engine, for: Self.teardownRetention)
    }

    var inputNode: any MicrophoneAudioInputNode { engine.inputNode }

    func start() throws {
        try engine.start()
    }

    func stop() {
        engine.stop()
    }

    func pause() {
        engine.pause()
    }

    func addConfigurationChangeObserver(
        _ handler: @escaping @Sendable () -> Void
    ) -> any SelectedAudioInputObserver {
        let token = NotificationCenter.default.addObserver(
            forName: .AVAudioEngineConfigurationChange,
            object: engine,
            queue: nil
        ) { _ in
            handler()
        }
        return NotificationCenterSelectedAudioInputObserver(token: token)
    }

    func audioUnit() throws -> AudioUnit {
        guard let audioUnit = engine.inputNode.audioUnit else {
            throw SelectedAudioInputError.unavailable
        }
        return audioUnit
    }
}

private final class DelayedReleaseRetainer<Object: AnyObject>: @unchecked Sendable {
    private final class Box: @unchecked Sendable {
        let object: Object

        init(_ object: Object) {
            self.object = object
        }
    }

    private let queue: DispatchQueue

    init(label: String) {
        queue = DispatchQueue(label: label, qos: .utility)
    }

    func retain(_ object: Object, for delay: DispatchTimeInterval) {
        let box = Box(object)
        queue.asyncAfter(deadline: .now() + delay) {
            withExtendedLifetime(box) {}
        }
    }
}

struct CoreAudioSelectedAudioInputManager: SelectedAudioInputManaging {
    private let catalog: any AudioInputDeviceCatalog
    private let audioUnitProvider: @Sendable () throws -> AudioUnit
    private let listenerQueue: DispatchQueue

    init(
        engine: LiveMicrophoneAudioEngine,
        catalog: any AudioInputDeviceCatalog = CoreAudioInputDeviceCatalog(),
        listenerQueue: DispatchQueue = DispatchQueue(label: "com.lifesub.capture.microphone.listeners")
    ) {
        self.catalog = catalog
        audioUnitProvider = { try engine.audioUnit() }
        self.listenerQueue = listenerQueue
    }

    func resolveSelectedInput(uid: String) throws -> ResolvedSelectedAudioInput {
        let deviceIDs: [AudioDeviceID]
        do {
            deviceIDs = try catalog.deviceIDs()
        } catch {
            throw SelectedAudioInputError.unavailable
        }

        for deviceID in deviceIDs {
            guard (try? catalog.deviceUID(deviceID)) == uid else { continue }
            guard (try? catalog.deviceIsAlive(deviceID)) == true,
                  (try? catalog.deviceHasInput(deviceID)) == true,
                  let name = try? catalog.deviceName(deviceID)
            else {
                throw SelectedAudioInputError.unavailable
            }
            return ResolvedSelectedAudioInput(deviceID: deviceID, uid: uid, name: name)
        }
        throw SelectedAudioInputError.unavailable
    }

    func bindSelectedInput(deviceID: AudioDeviceID) throws {
        var mutableDeviceID = deviceID
        let status = AudioUnitSetProperty(
            try audioUnitProvider(),
            kAudioOutputUnitProperty_CurrentDevice,
            kAudioUnitScope_Global,
            0,
            &mutableDeviceID,
            UInt32(MemoryLayout<AudioDeviceID>.size)
        )
        guard status == noErr else {
            throw SelectedAudioInputError.unavailable
        }
    }

    func currentBoundInput() throws -> ResolvedSelectedAudioInput {
        var deviceID = AudioDeviceID(kAudioObjectUnknown)
        var byteCount = UInt32(MemoryLayout<AudioDeviceID>.size)
        let status = AudioUnitGetProperty(
            try audioUnitProvider(),
            kAudioOutputUnitProperty_CurrentDevice,
            kAudioUnitScope_Global,
            0,
            &deviceID,
            &byteCount
        )
        guard status == noErr, deviceID != kAudioObjectUnknown else {
            throw SelectedAudioInputError.unavailable
        }
        guard let uid = try? catalog.deviceUID(deviceID),
              let name = try? catalog.deviceName(deviceID)
        else {
            throw SelectedAudioInputError.unavailable
        }
        return ResolvedSelectedAudioInput(deviceID: deviceID, uid: uid, name: name)
    }

    func expectedInputFormat(deviceID: AudioDeviceID) throws -> SelectedAudioInputFormat {
        var sampleRate = Float64(0)
        var sampleRateSize = UInt32(MemoryLayout<Float64>.size)
        var sampleRateAddress = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyNominalSampleRate,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        guard AudioObjectGetPropertyData(
            deviceID,
            &sampleRateAddress,
            0,
            nil,
            &sampleRateSize,
            &sampleRate
        ) == noErr,
        sampleRate > 0
        else { throw SelectedAudioInputError.unavailable }

        var streamAddress = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyStreamConfiguration,
            mScope: kAudioObjectPropertyScopeInput,
            mElement: kAudioObjectPropertyElementMain
        )
        let channelCount = try CoreAudioVariablePropertyReader.readValue(
            minimumByteCount: MemoryLayout<AudioBufferList>.size,
            alignment: MemoryLayout<AudioBufferList>.alignment,
            size: {
                var dataSize: UInt32 = 0
                guard AudioObjectGetPropertyDataSize(
                    deviceID,
                    &streamAddress,
                    0,
                    nil,
                    &dataSize
                ) == noErr else { throw SelectedAudioInputError.unavailable }
                return dataSize
            },
            read: { storage, dataSize in
                AudioObjectGetPropertyData(
                    deviceID,
                    &streamAddress,
                    0,
                    nil,
                    &dataSize,
                    storage
                )
            },
            decode: { storage, _ in
                let list = storage.assumingMemoryBound(to: AudioBufferList.self)
                return UnsafeMutableAudioBufferListPointer(list).reduce(0) {
                    $0 + Int($1.mNumberChannels)
                }
            }
        )
        guard channelCount > 0, channelCount <= Int(UInt16.max) else {
            throw SelectedAudioInputError.unavailable
        }
        return SelectedAudioInputFormat(
            sampleRate: sampleRate,
            channelCount: AVAudioChannelCount(channelCount)
        )
    }

    func addChangeObservers(
        selectedDeviceID: AudioDeviceID,
        handler: @escaping @Sendable () -> Void
    ) throws -> [any SelectedAudioInputObserver] {
        do {
            let deviceList = try AudioObjectPropertyObserver(
                objectID: AudioObjectID(kAudioObjectSystemObject),
                address: AudioObjectPropertyAddress(
                    mSelector: kAudioHardwarePropertyDevices,
                    mScope: kAudioObjectPropertyScopeGlobal,
                    mElement: kAudioObjectPropertyElementMain
                ),
                queue: listenerQueue,
                handler: handler
            )
            let selectedAlive = try AudioObjectPropertyObserver(
                objectID: selectedDeviceID,
                address: AudioObjectPropertyAddress(
                    mSelector: kAudioDevicePropertyDeviceIsAlive,
                    mScope: kAudioObjectPropertyScopeGlobal,
                    mElement: kAudioObjectPropertyElementMain
                ),
                queue: listenerQueue,
                handler: handler
            )
            let currentDevice = try AudioUnitCurrentDeviceObserver(
                audioUnit: try audioUnitProvider(),
                handler: handler
            )
            return [deviceList, selectedAlive, currentDevice]
        } catch {
            throw SelectedAudioInputError.listenerRegistrationFailed
        }
    }
}

private final class NotificationCenterSelectedAudioInputObserver: SelectedAudioInputObserver, @unchecked Sendable {
    private let token: NSObjectProtocol

    init(token: NSObjectProtocol) {
        self.token = token
    }

    func cancel() {
        NotificationCenter.default.removeObserver(token)
    }
}

private final class AudioObjectPropertyObserver: SelectedAudioInputObserver, @unchecked Sendable {
    private let objectID: AudioObjectID
    private let address: AudioObjectPropertyAddress
    private let queue: DispatchQueue
    private let block: AudioObjectPropertyListenerBlock
    private let cancelled = NSLock()
    private var isCancelled = false

    init(
        objectID: AudioObjectID,
        address: AudioObjectPropertyAddress,
        queue: DispatchQueue,
        handler: @escaping @Sendable () -> Void
    ) throws {
        self.objectID = objectID
        self.address = address
        self.queue = queue
        block = { _, _ in
            handler()
        }
        var mutableAddress = address
        let status = AudioObjectAddPropertyListenerBlock(
            objectID,
            &mutableAddress,
            queue,
            block
        )
        guard status == noErr else {
            throw SelectedAudioInputError.listenerRegistrationFailed
        }
    }

    deinit {
        cancel()
    }

    func cancel() {
        cancelled.lock()
        defer { cancelled.unlock() }
        guard !isCancelled else { return }
        isCancelled = true
        var mutableAddress = address
        AudioObjectRemovePropertyListenerBlock(objectID, &mutableAddress, queue, block)
    }
}

private final class AudioUnitCurrentDeviceObserver: SelectedAudioInputObserver, @unchecked Sendable {
    private final class CallbackBox: @unchecked Sendable {
        let handler: @Sendable () -> Void

        init(handler: @escaping @Sendable () -> Void) {
            self.handler = handler
        }
    }

    private let audioUnit: AudioUnit
    private let userData: UnsafeMutableRawPointer
    private let cancelled = NSLock()
    private var isCancelled = false

    init(audioUnit: AudioUnit, handler: @escaping @Sendable () -> Void) throws {
        self.audioUnit = audioUnit
        let box = CallbackBox(handler: handler)
        userData = Unmanaged.passRetained(box).toOpaque()
        let status = AudioUnitAddPropertyListener(
            audioUnit,
            kAudioOutputUnitProperty_CurrentDevice,
            Self.listener,
            userData
        )
        guard status == noErr else {
            Unmanaged<CallbackBox>.fromOpaque(userData).release()
            throw SelectedAudioInputError.listenerRegistrationFailed
        }
    }

    deinit {
        cancel()
    }

    func cancel() {
        cancelled.lock()
        defer { cancelled.unlock() }
        guard !isCancelled else { return }
        isCancelled = true
        AudioUnitRemovePropertyListenerWithUserData(
            audioUnit,
            kAudioOutputUnitProperty_CurrentDevice,
            Self.listener,
            userData
        )
        Unmanaged<CallbackBox>.fromOpaque(userData).release()
    }

    private static let listener: AudioUnitPropertyListenerProc = { userData, _, _, _, _ in
        let box = Unmanaged<CallbackBox>.fromOpaque(userData).takeUnretainedValue()
        box.handler()
    }
}
