// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "LifeSubCaptureHelper",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "CaptureProtocol", targets: ["CaptureProtocol"]),
    ],
    targets: [
        .target(name: "CaptureProtocol"),
        .testTarget(
            name: "CaptureProtocolTests",
            dependencies: ["CaptureProtocol"],
            resources: [.process("Fixtures")]
        ),
    ]
)
