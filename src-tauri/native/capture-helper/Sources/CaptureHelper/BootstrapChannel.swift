import Foundation
import Darwin

enum BootstrapChannelError: Error {
    case invalidDescriptor
    case truncatedNonce
    case invalidSocketPath
    case socketConnectFailed
}

extension BootstrapChannel {
    static func connect(to path: String) throws -> FileHandle {
        let bytes = Array(path.utf8)
        var address = sockaddr_un()
        guard bytes.count < MemoryLayout.size(ofValue: address.sun_path) else {
            throw BootstrapChannelError.invalidSocketPath
        }
        address.sun_family = sa_family_t(AF_UNIX)
        address.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)
        withUnsafeMutableBytes(of: &address.sun_path) { destination in
            destination.initializeMemory(as: UInt8.self, repeating: 0)
            destination.copyBytes(from: bytes)
        }
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { throw BootstrapChannelError.socketConnectFailed }
        let result = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(descriptor, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard result == 0 else {
            close(descriptor)
            throw BootstrapChannelError.socketConnectFailed
        }
        return FileHandle(fileDescriptor: descriptor, closeOnDealloc: true)
    }
}

enum BootstrapChannel {
    static let inheritedDescriptor: Int32 = 3
    static let nonceByteCount = 32

    static func readNonce(
        descriptor: Int32 = inheritedDescriptor,
        byteCount: Int = nonceByteCount
    ) throws -> Data {
        guard descriptor >= 0, byteCount > 0 else { throw BootstrapChannelError.invalidDescriptor }
        defer { close(descriptor) }
        var data = Data(count: byteCount)
        let count = data.withUnsafeMutableBytes { buffer -> Int in
            guard let base = buffer.baseAddress else { return -1 }
            var offset = 0
            while offset < byteCount {
                let result = read(descriptor, base.advanced(by: offset), byteCount - offset)
                if result <= 0 { return offset }
                offset += result
            }
            return offset
        }
        guard count == byteCount else { throw BootstrapChannelError.truncatedNonce }
        return data
    }
}
