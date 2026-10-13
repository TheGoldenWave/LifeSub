import CaptureProtocol
import CoreAudio
import Foundation

protocol AudioInputDeviceCatalog: Sendable {
    func deviceIDs() throws -> [AudioDeviceID]
    func defaultInputDeviceID() throws -> AudioDeviceID?
    func deviceUID(_ deviceID: AudioDeviceID) throws -> String
    func deviceName(_ deviceID: AudioDeviceID) throws -> String
    func deviceIsAlive(_ deviceID: AudioDeviceID) throws -> Bool
    func deviceTransportType(_ deviceID: AudioDeviceID) throws -> UInt32
    func deviceHasInput(_ deviceID: AudioDeviceID) throws -> Bool
}

enum AudioInputDeviceEnumerationError: Error, Equatable, Sendable {
    case enumerationFailed
}

struct AudioInputDevices: Sendable {
    private let catalog: any AudioInputDeviceCatalog

    init(catalog: any AudioInputDeviceCatalog = CoreAudioInputDeviceCatalog()) {
        self.catalog = catalog
    }

    func listMicrophones() throws -> [MicrophoneDevice] {
        let deviceIDs: [AudioDeviceID]
        let defaultInputDeviceID: AudioDeviceID?
        do {
            deviceIDs = try catalog.deviceIDs()
            defaultInputDeviceID = try catalog.defaultInputDeviceID()
        } catch {
            throw AudioInputDeviceEnumerationError.enumerationFailed
        }

        var seenUIDs = Set<String>()
        var devices: [MicrophoneDevice] = []
        devices.reserveCapacity(min(deviceIDs.count, ProtocolLimits.maxMicrophoneDevices))

        for deviceID in deviceIDs {
            let uid: String
            let name: String
            do {
                uid = try catalog.deviceUID(deviceID)
                name = try catalog.deviceName(deviceID)
            } catch {
                throw AudioInputDeviceEnumerationError.enumerationFailed
            }

            guard !uid.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
                  !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
                  seenUIDs.insert(uid).inserted
            else {
                throw AudioInputDeviceEnumerationError.enumerationFailed
            }

            let device: MicrophoneDevice
            do {
                guard try catalog.deviceIsAlive(deviceID),
                      try catalog.deviceHasInput(deviceID)
                else { continue }

                device = MicrophoneDevice(
                    uid: uid,
                    name: name,
                    isDefault: deviceID == defaultInputDeviceID,
                    isConnected: true,
                    transport: microphoneTransport(for: try catalog.deviceTransportType(deviceID))
                )
            } catch {
                continue
            }
            guard devices.count < ProtocolLimits.maxMicrophoneDevices else {
                throw AudioInputDeviceEnumerationError.enumerationFailed
            }
            devices.append(device)
        }

        return devices.sorted { left, right in
            if left.isDefault != right.isDefault {
                return left.isDefault
            }
            let nameOrder = left.name.localizedStandardCompare(right.name)
            if nameOrder != .orderedSame {
                return nameOrder == .orderedAscending
            }
            return left.uid < right.uid
        }
    }

    private func microphoneTransport(for transportType: UInt32) -> MicrophoneTransport {
        switch transportType {
        case kAudioDeviceTransportTypeBuiltIn:
            return .builtIn
        case kAudioDeviceTransportTypeUSB:
            return .usb
        case kAudioDeviceTransportTypeBluetooth, kAudioDeviceTransportTypeBluetoothLE:
            return .bluetooth
        case kAudioDeviceTransportTypeAggregate:
            return .aggregate
        case kAudioDeviceTransportTypeVirtual:
            return .virtual
        default:
            return .unknown
        }
    }
}

struct CoreAudioInputDeviceCatalog: AudioInputDeviceCatalog {
    func deviceIDs() throws -> [AudioDeviceID] {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        return try CoreAudioVariablePropertyReader.readAudioDeviceIDs(
            size: {
                var dataSize: UInt32 = 0
                try check(
                    AudioObjectGetPropertyDataSize(
                        AudioObjectID(kAudioObjectSystemObject),
                        &address,
                        0,
                        nil,
                        &dataSize
                    )
                )
                return dataSize
            },
            read: { storage, dataSize in
                AudioObjectGetPropertyData(
                    AudioObjectID(kAudioObjectSystemObject),
                    &address,
                    0,
                    nil,
                    &dataSize,
                    storage
                )
            }
        )
    }

    func defaultInputDeviceID() throws -> AudioDeviceID? {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultInputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        var deviceID = AudioDeviceID(kAudioObjectUnknown)
        var dataSize = UInt32(MemoryLayout<AudioDeviceID>.size)
        try check(
            AudioObjectGetPropertyData(
                AudioObjectID(kAudioObjectSystemObject),
                &address,
                0,
                nil,
                &dataSize,
                &deviceID
            )
        )
        return deviceID == kAudioObjectUnknown ? nil : deviceID
    }

    func deviceUID(_ deviceID: AudioDeviceID) throws -> String {
        try stringProperty(kAudioDevicePropertyDeviceUID, deviceID: deviceID)
    }

    func deviceName(_ deviceID: AudioDeviceID) throws -> String {
        try stringProperty(kAudioObjectPropertyName, deviceID: deviceID)
    }

    func deviceIsAlive(_ deviceID: AudioDeviceID) throws -> Bool {
        try uint32Property(kAudioDevicePropertyDeviceIsAlive, deviceID: deviceID) != 0
    }

    func deviceTransportType(_ deviceID: AudioDeviceID) throws -> UInt32 {
        try uint32Property(kAudioDevicePropertyTransportType, deviceID: deviceID)
    }

    func deviceHasInput(_ deviceID: AudioDeviceID) throws -> Bool {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyStreamConfiguration,
            mScope: kAudioObjectPropertyScopeInput,
            mElement: kAudioObjectPropertyElementMain
        )
        return try CoreAudioVariablePropertyReader.readValue(
            minimumByteCount: MemoryLayout<UInt32>.size,
            alignment: MemoryLayout<AudioBufferList>.alignment,
            size: {
                var dataSize: UInt32 = 0
                try check(AudioObjectGetPropertyDataSize(deviceID, &address, 0, nil, &dataSize))
                return dataSize
            },
            read: { storage, dataSize in
                AudioObjectGetPropertyData(deviceID, &address, 0, nil, &dataSize, storage)
            },
            decode: { storage, returnedByteCount in
                let bufferCount = Int(storage.load(as: UInt32.self))
                guard bufferCount > 0 else { return false }

                let (additionalBytes, overflow) = (bufferCount - 1).multipliedReportingOverflow(
                    by: MemoryLayout<AudioBuffer>.stride
                )
                let (requiredByteCount, totalOverflow) = MemoryLayout<AudioBufferList>.size
                    .addingReportingOverflow(additionalBytes)
                guard !overflow, !totalOverflow, requiredByteCount <= returnedByteCount else {
                    throw CoreAudioCatalogError.invalidPropertyValue
                }
                let audioBufferList = storage.assumingMemoryBound(to: AudioBufferList.self)
                return UnsafeMutableAudioBufferListPointer(audioBufferList).contains { buffer in
                    buffer.mNumberChannels > 0
                }
            }
        )
    }

    private func stringProperty(
        _ selector: AudioObjectPropertySelector,
        deviceID: AudioDeviceID
    ) throws -> String {
        var address = AudioObjectPropertyAddress(
            mSelector: selector,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        var value: Unmanaged<CFString>?
        var dataSize = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
        try check(AudioObjectGetPropertyData(deviceID, &address, 0, nil, &dataSize, &value))
        guard let value else {
            throw CoreAudioCatalogError.invalidPropertyValue
        }
        return value.takeRetainedValue() as String
    }

    private func uint32Property(
        _ selector: AudioObjectPropertySelector,
        deviceID: AudioDeviceID
    ) throws -> UInt32 {
        var address = AudioObjectPropertyAddress(
            mSelector: selector,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        var value: UInt32 = 0
        var dataSize = UInt32(MemoryLayout<UInt32>.size)
        try check(AudioObjectGetPropertyData(deviceID, &address, 0, nil, &dataSize, &value))
        return value
    }

    private func check(_ status: OSStatus) throws {
        guard status == noErr else {
            throw CoreAudioCatalogError.propertyReadFailed
        }
    }
}

enum CoreAudioVariablePropertyReader {
    private static let maximumAttempts = 3

    static func readAudioDeviceIDs(
        size: () throws -> UInt32,
        read: (UnsafeMutableRawPointer, inout UInt32) -> OSStatus
    ) throws -> [AudioDeviceID] {
        for _ in 0 ..< maximumAttempts {
            let requestedByteCount = try size()
            guard requestedByteCount.isMultiple(of: UInt32(MemoryLayout<AudioDeviceID>.stride)) else {
                throw CoreAudioCatalogError.invalidPropertyValue
            }
            guard requestedByteCount > 0 else { return [] }

            var values = [AudioDeviceID](
                repeating: kAudioObjectUnknown,
                count: Int(requestedByteCount) / MemoryLayout<AudioDeviceID>.stride
            )
            var returnedByteCount = requestedByteCount
            let status = values.withUnsafeMutableBytes { bytes in
                read(bytes.baseAddress!, &returnedByteCount)
            }
            guard status == noErr,
                  returnedByteCount <= requestedByteCount,
                  returnedByteCount.isMultiple(of: UInt32(MemoryLayout<AudioDeviceID>.stride))
            else { continue }

            let returnedCount = Int(returnedByteCount) / MemoryLayout<AudioDeviceID>.stride
            return Array(values.prefix(returnedCount))
        }
        throw CoreAudioCatalogError.propertyReadFailed
    }

    static func readValue<Result>(
        minimumByteCount: Int,
        alignment: Int,
        size: () throws -> UInt32,
        read: (UnsafeMutableRawPointer, inout UInt32) -> OSStatus,
        decode: (UnsafeMutableRawPointer, Int) throws -> Result
    ) throws -> Result {
        for _ in 0 ..< maximumAttempts {
            let requestedByteCount = try size()
            guard requestedByteCount >= minimumByteCount else {
                throw CoreAudioCatalogError.invalidPropertyValue
            }

            let storage = UnsafeMutableRawPointer.allocate(
                byteCount: Int(requestedByteCount),
                alignment: alignment
            )
            defer { storage.deallocate() }
            var returnedByteCount = requestedByteCount
            let status = read(storage, &returnedByteCount)
            guard status == noErr, returnedByteCount <= requestedByteCount else { continue }
            return try decode(storage, Int(returnedByteCount))
        }
        throw CoreAudioCatalogError.propertyReadFailed
    }
}

private enum CoreAudioCatalogError: Error {
    case propertyReadFailed
    case invalidPropertyValue
}
