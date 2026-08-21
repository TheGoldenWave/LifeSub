// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "LifeSubCaptureHelper",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "CaptureProtocol", targets: ["CaptureProtocol"]),
        .executable(name: "lifesub-capture-helper", targets: ["CaptureHelper"]),
    ],
    targets: [
        .target(name: "CaptureProtocol"),
        .executableTarget(name: "CaptureHelper", dependencies: ["CaptureProtocol"]),
        .testTarget(
            name: "CaptureProtocolTests",
            dependencies: ["CaptureProtocol"],
            resources: [.process("Fixtures")]
        ),
        .testTarget(name: "CaptureHelperTests", dependencies: ["CaptureHelper", "CaptureProtocol"]),
    ]
)
