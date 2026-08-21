import CaptureProtocol
import Darwin
import Foundation

private func run() throws {
    guard let socketPath = ProcessInfo.processInfo.environment["LIFESUB_CAPTURE_SOCKET"] else {
        throw BootstrapChannelError.invalidSocketPath
    }
    var nonce = try BootstrapChannel.readNonce()
    defer { nonce.resetBytes(in: 0 ..< nonce.count) }
    var nonceHex = nonce.map { String(format: "%02x", $0) }.joined()
    defer {
        nonceHex.removeAll(keepingCapacity: true)
        nonceHex.append(String(repeating: "0", count: 64))
    }
    let hello = CaptureHeader.hello(
        Hello(
            protocolVersion: ProtocolLimits.version,
            helperPid: UInt32(getpid()),
            launchNonce: nonceHex,
            supportedSources: [.microphone, .systemAudio]
        )
    )
    var frame = try FrameCodec.encode(header: hello, payload: Data())
    defer { frame.resetBytes(in: 0 ..< frame.count) }
    let connection = try BootstrapChannel.connect(to: socketPath)
    try connection.write(contentsOf: frame)
    while let data = try connection.read(upToCount: 1), !data.isEmpty {}
}

do {
    try run()
    exit(EXIT_SUCCESS)
} catch {
    exit(EXIT_FAILURE)
}
