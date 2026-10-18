// swift-tools-version: 5.9
import PackageDescription
import Foundation

// CI supplies a staged package outside this checkout. Published-package checks
// instead set ZPL_PACKAGE_URL and ZPL_PACKAGE_VERSION to exercise Git resolution.
let dependency: Package.Dependency
if let url = ProcessInfo.processInfo.environment["ZPL_PACKAGE_URL"],
   let version = ProcessInfo.processInfo.environment["ZPL_PACKAGE_VERSION"] {
    dependency = .package(url: url, exact: Version(version)!)
} else {
    dependency = .package(path: ProcessInfo.processInfo.environment["ZPL_PACKAGE_PATH"]!)
}
let package = Package(
    name: "Consumer", platforms: [.macOS(.v11), .iOS(.v13)],
    dependencies: [dependency],
    targets: [.executableTarget(name: "Consumer", dependencies: [.product(name: "ZPL", package: "zpl-swift")])]
)
