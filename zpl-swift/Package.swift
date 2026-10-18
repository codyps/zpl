// swift-tools-version: 5.9
import PackageDescription

// Local development: scripts/swift-package.py builds CZpl.xcframework here.
// Release staging replaces this target with an immutable URL and ZIP checksum.
let package = Package(
    name: "ZPL",
    platforms: [.macOS(.v11), .iOS(.v13)],
    products: [.library(name: "ZPL", targets: ["ZPL"])],
    targets: [
        .binaryTarget(name: "CZpl", path: "CZpl.xcframework"),
        .target(name: "ZPL", dependencies: ["CZpl"], linkerSettings: [.linkedLibrary("iconv")]),
        .testTarget(name: "ZPLTests", dependencies: ["ZPL"], resources: [.copy("Fixtures")]),
    ]
)
