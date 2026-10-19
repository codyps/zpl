// swift-tools-version: 5.9
import PackageDescription

// Apple consumers retain the XCFramework and Swift 5.9 support. SwiftPM 6.2
// adds static-library artifact bundles for other destinations (SE-0482).
var nativeTargets: [Target] = [.binaryTarget(name: "CZpl", path: "CZpl.xcframework")]
var nativeDependencies: [Target.Dependency] = [
    .target(name: "CZpl", condition: .when(platforms: [.macOS, .iOS]))
]
#if compiler(>=6.2)
nativeTargets.append(.binaryTarget(name: "CZplNative", path: "CZpl.artifactbundle"))
nativeDependencies.append(.target(name: "CZplNative", condition: .when(platforms: [.linux, .windows])))
#endif

let package = Package(
    name: "ZPL",
    platforms: [.macOS(.v11), .iOS(.v13)],
    products: [.library(name: "ZPL", targets: ["ZPL"])],
    targets: nativeTargets + [
        .target(name: "ZPL", dependencies: nativeDependencies, linkerSettings: [
            .linkedLibrary("iconv", .when(platforms: [.macOS, .iOS])),
            .linkedLibrary("m", .when(platforms: [.linux])),
            .linkedLibrary("pthread", .when(platforms: [.linux])),
            .linkedLibrary("dl", .when(platforms: [.linux])),
            .linkedLibrary("kernel32", .when(platforms: [.windows])),
            .linkedLibrary("dbghelp", .when(platforms: [.windows])),
            .linkedLibrary("advapi32", .when(platforms: [.windows])),
            .linkedLibrary("bcrypt", .when(platforms: [.windows])),
            .linkedLibrary("ntdll", .when(platforms: [.windows])),
            .linkedLibrary("userenv", .when(platforms: [.windows])),
            .linkedLibrary("ws2_32", .when(platforms: [.windows])),
            .linkedLibrary("synchronization", .when(platforms: [.windows])),
        ]),
        .testTarget(name: "ZPLTests", dependencies: ["ZPL"], resources: [.copy("Fixtures")]),
    ]
)
