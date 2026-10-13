import CaptureProtocol
import CoreAudio
import Foundation
import Testing
@testable import CaptureHelper

@Test func listMicrophonesFiltersDeadAndOutputOnlyDevicesAndSortsDefaultFirst() throws {
    let catalog = FakeAudioInputDeviceCatalog(
        deviceIDs: [11, 12, 13, 14, 15],
        defaultInputDeviceID: 14,
        devices: [
            11: FakeAudioInputDeviceCatalog.Device(uid: "usb-1", name: "USB Mic", isAlive: true, transportType: kAudioDeviceTransportTypeUSB, hasInput: true),
            12: FakeAudioInputDeviceCatalog.Device(uid: "output-only", name: "Speakers", isAlive: true, transportType: kAudioDeviceTransportTypeBuiltIn, hasInput: false),
            13: FakeAudioInputDeviceCatalog.Device(uid: "dead-1", name: "Offline Mic", isAlive: false, transportType: kAudioDeviceTransportTypeBluetooth, hasInput: true),
            14: FakeAudioInputDeviceCatalog.Device(uid: "builtin-1", name: "Built-in Microphone", isAlive: true, transportType: kAudioDeviceTransportTypeBuiltIn, hasInput: true),
            15: FakeAudioInputDeviceCatalog.Device(uid: "bt-1", name: "Bluetooth Headset", isAlive: true, transportType: kAudioDeviceTransportTypeBluetoothLE, hasInput: true),
        ]
    )

    let devices = try AudioInputDevices(catalog: catalog).listMicrophones()

    #expect(
        devices == [
            MicrophoneDevice(uid: "builtin-1", name: "Built-in Microphone", isDefault: true, isConnected: true, transport: .builtIn),
            MicrophoneDevice(uid: "bt-1", name: "Bluetooth Headset", isDefault: false, isConnected: true, transport: .bluetooth),
            MicrophoneDevice(uid: "usb-1", name: "USB Mic", isDefault: false, isConnected: true, transport: .usb),
        ]
    )
    let expectedOperations: [FakeAudioInputDeviceCatalog.Operation] = [
        .deviceIDs,
        .defaultInputDeviceID,
        .uid(11), .name(11), .alive(11), .hasInput(11), .transport(11),
        .uid(12), .name(12), .alive(12), .hasInput(12),
        .uid(13), .name(13), .alive(13),
        .uid(14), .name(14), .alive(14), .hasInput(14), .transport(14),
        .uid(15), .name(15), .alive(15), .hasInput(15), .transport(15),
    ]
    #expect(catalog.operations == expectedOperations)
}

@Test func listMicrophonesSkipsDevicesWithNonIdentityPropertyFailures() throws {
    let catalog = FakeAudioInputDeviceCatalog(
        deviceIDs: [21, 22, 23],
        defaultInputDeviceID: 21,
        devices: [
            21: FakeAudioInputDeviceCatalog.Device(uid: "builtin-1", name: "Built-in Microphone", isAlive: true, transportType: kAudioDeviceTransportTypeBuiltIn, hasInput: true),
            22: FakeAudioInputDeviceCatalog.Device(uid: "aggregate-1", name: "Aggregate Input", isAlive: true, transportType: kAudioDeviceTransportTypeAggregate, hasInput: true),
            23: FakeAudioInputDeviceCatalog.Device(uid: "virtual-1", name: "Loopback", isAlive: true, transportType: kAudioDeviceTransportTypeVirtual, hasInput: true),
        ],
        failingProperties: Set([
            .transport(22),
            .alive(23),
        ])
    )

    let devices = try AudioInputDevices(catalog: catalog).listMicrophones()

    #expect(
        devices == [
            MicrophoneDevice(uid: "builtin-1", name: "Built-in Microphone", isDefault: true, isConnected: true, transport: .builtIn),
        ]
    )
}

@Test func listMicrophonesFailsClosedOnDuplicateUid() throws {
    let catalog = FakeAudioInputDeviceCatalog(
        deviceIDs: [31, 32],
        defaultInputDeviceID: 31,
        devices: [
            31: FakeAudioInputDeviceCatalog.Device(uid: "dup-1", name: "Built-in Microphone", isAlive: true, transportType: kAudioDeviceTransportTypeBuiltIn, hasInput: true),
            32: FakeAudioInputDeviceCatalog.Device(uid: "dup-1", name: "USB Mic", isAlive: true, transportType: kAudioDeviceTransportTypeUSB, hasInput: true),
        ]
    )

    #expect(throws: AudioInputDeviceEnumerationError.self) {
        try AudioInputDevices(catalog: catalog).listMicrophones()
    }
}

@Test func listMicrophonesFailsClosedOnMissingIdentityOrTopLevelEnumerationFailure() throws {
    let missingIdentityCatalog = FakeAudioInputDeviceCatalog(
        deviceIDs: [41],
        defaultInputDeviceID: 41,
        devices: [
            41: FakeAudioInputDeviceCatalog.Device(uid: "", name: "Built-in Microphone", isAlive: true, transportType: kAudioDeviceTransportTypeBuiltIn, hasInput: true),
        ]
    )
    #expect(throws: AudioInputDeviceEnumerationError.self) {
        try AudioInputDevices(catalog: missingIdentityCatalog).listMicrophones()
    }

    let topLevelFailureCatalog = FakeAudioInputDeviceCatalog(
        deviceIDs: [51],
        defaultInputDeviceID: 51,
        devices: [
            51: FakeAudioInputDeviceCatalog.Device(uid: "builtin-1", name: "Built-in Microphone", isAlive: true, transportType: kAudioDeviceTransportTypeBuiltIn, hasInput: true),
        ],
        failingProperties: Set([.deviceIDs])
    )
    #expect(throws: AudioInputDeviceEnumerationError.self) {
        try AudioInputDevices(catalog: topLevelFailureCatalog).listMicrophones()
    }
}

@Test func listMicrophonesFailsBeforeAddingThirtyThirdUsableDevice() throws {
    let count = ProtocolLimits.maxMicrophoneDevices + 1
    let deviceIDs = (1 ... count).map(AudioDeviceID.init)
    let devices = Dictionary(uniqueKeysWithValues: deviceIDs.map { deviceID in
        (
            deviceID,
            FakeAudioInputDeviceCatalog.Device(
                uid: "mic-\(deviceID)",
                name: "Microphone \(deviceID)",
                isAlive: true,
                transportType: kAudioDeviceTransportTypeUSB,
                hasInput: true
            )
        )
    })
    let catalog = FakeAudioInputDeviceCatalog(
        deviceIDs: deviceIDs,
        defaultInputDeviceID: deviceIDs.first,
        devices: devices
    )

    #expect(throws: AudioInputDeviceEnumerationError.self) {
        try AudioInputDevices(catalog: catalog).listMicrophones()
    }
    #expect(catalog.operations.last == .transport(deviceIDs.last!))
}

@Test func listMicrophonesMapsKnownAndUnknownCoreAudioTransports() throws {
    let cases: [(UInt32, MicrophoneTransport)] = [
        (kAudioDeviceTransportTypeBuiltIn, .builtIn),
        (kAudioDeviceTransportTypeUSB, .usb),
        (kAudioDeviceTransportTypeBluetooth, .bluetooth),
        (kAudioDeviceTransportTypeBluetoothLE, .bluetooth),
        (kAudioDeviceTransportTypeAggregate, .aggregate),
        (kAudioDeviceTransportTypeVirtual, .virtual),
        (UInt32.max, .unknown),
    ]

    for (index, transportCase) in cases.enumerated() {
        let deviceID = AudioDeviceID(index + 1)
        let catalog = FakeAudioInputDeviceCatalog(
            deviceIDs: [deviceID],
            defaultInputDeviceID: nil,
            devices: [
                deviceID: FakeAudioInputDeviceCatalog.Device(
                    uid: "mic-\(deviceID)",
                    name: "Microphone \(deviceID)",
                    isAlive: true,
                    transportType: transportCase.0,
                    hasInput: true
                ),
            ]
        )

        let result = try AudioInputDevices(catalog: catalog).listMicrophones()
        #expect(result.map(\.transport) == [transportCase.1])
    }
}

@Test func listMicrophonesFailsClosedOnUidOrNamePropertyFailure() throws {
    let device = FakeAudioInputDeviceCatalog.Device(
        uid: "mic-1",
        name: "Microphone",
        isAlive: true,
        transportType: kAudioDeviceTransportTypeBuiltIn,
        hasInput: true
    )
    let uidFailure = FakeAudioInputDeviceCatalog(
        deviceIDs: [61],
        defaultInputDeviceID: 61,
        devices: [61: device],
        failingProperties: [.uid(61)]
    )
    let nameFailure = FakeAudioInputDeviceCatalog(
        deviceIDs: [62],
        defaultInputDeviceID: 62,
        devices: [62: device],
        failingProperties: [.name(62)]
    )

    #expect(throws: AudioInputDeviceEnumerationError.self) {
        try AudioInputDevices(catalog: uidFailure).listMicrophones()
    }
    #expect(uidFailure.operations == [.deviceIDs, .defaultInputDeviceID, .uid(61)])

    #expect(throws: AudioInputDeviceEnumerationError.self) {
        try AudioInputDevices(catalog: nameFailure).listMicrophones()
    }
    #expect(nameFailure.operations == [.deviceIDs, .defaultInputDeviceID, .uid(62), .name(62)])
}

@Test func variablePropertyReaderUsesOnlyTheReturnedInitializedPrefix() throws {
    let values = try CoreAudioVariablePropertyReader.readAudioDeviceIDs(
        size: { UInt32(2 * MemoryLayout<AudioDeviceID>.stride) },
        read: { storage, returnedSize in
            let devices = storage.assumingMemoryBound(to: AudioDeviceID.self)
            devices[0] = 71
            devices[1] = 999
            returnedSize = UInt32(MemoryLayout<AudioDeviceID>.stride)
            return noErr
        }
    )

    #expect(values == [71])
}

@Test func variablePropertyReaderRetriesWhenThePropertyGrowsBetweenReads() throws {
    var sizeReads = 0
    var dataReads = 0
    let values = try CoreAudioVariablePropertyReader.readAudioDeviceIDs(
        size: {
            sizeReads += 1
            return UInt32(sizeReads == 1 ? 1 : 2) * UInt32(MemoryLayout<AudioDeviceID>.stride)
        },
        read: { storage, returnedSize in
            dataReads += 1
            if dataReads == 1 {
                returnedSize = UInt32(2 * MemoryLayout<AudioDeviceID>.stride)
                return kAudioHardwareBadPropertySizeError
            }
            let devices = storage.assumingMemoryBound(to: AudioDeviceID.self)
            devices[0] = 81
            devices[1] = 82
            return noErr
        }
    )

    #expect(values == [81, 82])
    #expect(sizeReads == 2)
    #expect(dataReads == 2)
}

private struct FakeAudioInputDeviceCatalog: AudioInputDeviceCatalog {
    struct Device: Sendable {
        let uid: String
        let name: String
        let isAlive: Bool
        let transportType: UInt32
        let hasInput: Bool
    }

    enum Operation: Hashable, Sendable {
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
            throw FakeCatalogError.missingDevice
        }
        return device
    }

    private func recordAndThrowIfNeeded(_ operation: Operation) throws {
        recordedOperations.withValue { $0.append(operation) }
        if failingProperties.contains(operation) {
            throw FakeCatalogError.injectedFailure
        }
    }
}

private enum FakeCatalogError: Error {
    case injectedFailure
    case missingDevice
}

private final class Locked<Value>: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: Value

    init(_ value: Value) {
        storage = value
    }

    var value: Value {
        lock.lock()
        defer { lock.unlock() }
        return storage
    }

    func withValue(_ update: (inout Value) -> Void) {
        lock.lock()
        defer { lock.unlock() }
        update(&storage)
    }
}
