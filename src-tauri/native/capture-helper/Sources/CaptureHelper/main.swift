import CaptureProtocol
import Darwin
import Foundation

private func run() async throws {
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
    let connection = try BootstrapChannel.connect(to: socketPath)
    let sink = SocketCaptureEventSink(connection: connection)
    let loop = HelperCommandLoop(
        connection: connection,
        nonceHex: nonceHex,
        controllerFactory: LiveCaptureSessionFactory(events: sink)
    )
    // The authenticated hello is written above; the loop consumes framed control
    // messages until an explicit shutdown or peer close.
    try await loop.run()
}

do {
    try await run()
    exit(EXIT_SUCCESS)
} catch {
    exit(EXIT_FAILURE)
}
