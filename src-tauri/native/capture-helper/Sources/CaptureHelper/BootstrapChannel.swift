import Foundation

enum BootstrapChannelError: Error {
    case invalidDescriptor
    case truncatedNonce
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
