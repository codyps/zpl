# ZPL for Swift

Parse Zebra Programming Language and render labels locally to PNG, SVG, PDF, or
grayscale pixels. No printer, external rendering service, or Rust installation is
required. The package wraps the [Rust ZPL library](https://github.com/codyps/zpl)
through its matching C ABI.

Requires Swift 5.9+, macOS 11+ or iOS 13+. Binaries include Intel and Apple Silicon
macOS, ARM64 iOS devices, and Intel/Apple Silicon iOS simulators. Other Apple
platforms, Mac Catalyst, Linux, and Windows are not included.

## Install

In Xcode, add the package URL:

```text
https://github.com/codyps/zpl-swift
```

Or add a released version to your manifest (replace `VERSION` with a published
version from the [renderer releases](https://github.com/codyps/zpl/releases)):

```swift
.package(url: "https://github.com/codyps/zpl-swift", exact: "VERSION")
```

Add `.product(name: "ZPL", package: "zpl-swift")` to your target dependencies.
The first Swift release becomes installable when its corresponding renderer
release has passed CI and published the distribution tag. Existing Rust releases
that predate the Swift binding do not contain this package.

## Render

```swift
import Foundation
import ZPL

let document = try ZPL.render(
    "^XA^PW400^LL200^FO20,20^A0N,32,0^FDHello, Swift!^FS^XZ"
)
let png = try document.label(at: 0).encode(.png)
try png.write(to: URL(fileURLWithPath: "/tmp/label.png"))
let pdf = try document.pdf() // All labels, one PDF page per label.
for warning in try document.warnings {
    print(warning)
}
```

Rendering is synchronous. Run substantial work away from your app's UI thread.
`Document`, `Scene`, and `Fonts` release their native resources automatically.
Scenes can outlive documents, and returned `Data` owns copied bytes. Handles are
not declared `Sendable`; keep them within the task or actor that owns them.

Pass `Data` to preserve legacy encodings, binary graphics, or embedded NULs.
String overloads use UTF-8; select the appropriate ZPL character encoding (`^CI28`
for UTF-8) in your document when needed.

## Profiles, limits, and fonts

```swift
var options = try RenderOptions(profile: .specification)
options.width = 812
options.height = 1218
options.dpi = 203
options.compatibility.qrPrinterMaskSelection = false

var limits = try RenderLimits()
limits.inputBytes = 1_000_000
let document = try ZPL.render(Data("^XA^XZ".utf8), options: options, limits: limits)
var outputLimits = try OutputLimits()
outputLimits.pixels = 4_000_000
let pixels = try document.label(at: 0).encode(.grayscale, limits: outputLimits)
```

Profiles are `.zd621` (the default), `.specification`, and `.zq610Plus`. Every C
compatibility option and render/output budget has a typed Swift property. Always
initialize configuration values using their initializer to obtain native defaults.
Grayscale data has one byte per dot, row-major with no padding: 0 black, 255 white.
Use `try scene.info` for width, height, and DPI. PNG/SVG/PDF are available through
`scene.encode`; `document.pdf()` supports multiple labels.

`Fonts.addTrueType(_:id:hinting:)` and `Fonts.addTrueType(_:filename:hinting:)`
register copied font data by Unicode scalar ID (65 for `A`) or virtual ZPL filename.
Pass the collection as `fonts:` to `ZPL.render`. Bitmap provider callbacks remain
available through the underlying C module rather than the Swift wrapper.

## Parse and handle errors

```swift
let source = Data("^XA^FO20,20^FDHello^FS^XZ".utf8)
let parsed = try ZPL.parse(source)
for element in parsed.elements {
    print(element.kind, element.byteOffset, element.data)
}
// Continue parsing another complete stream with parsed.finalSyntax if needed.
```

Parsing preserves original bytes, unknown commands, byte offsets, and final
syntax. It frames complete streams; bound input size before parsing. It does not
validate commands or authorize sending them to a printer.

Failures throw `ZPLError` with `status`, `message`, and an optional `byteOffset`.
Error messages are copied immediately from native thread-local storage. Rendering
supports a practical ZPL subset, with errors for unsupported commands and warnings
for font approximations. See [renderer coverage](https://github.com/codyps/zpl/blob/main/docs/local-renderer.md)
and [C API contracts](https://github.com/codyps/zpl/blob/main/zpl-c/README.md).

## Versions and license

The Swift tag matches the renderer version. Its manifest references an immutable,
SHA-256-checked XCFramework asset on that renderer's GitHub release. Headers and
native libraries are built together; the wrapper checks the C ABI before use.
`ZPL.libraryVersion` reports the native renderer version, and `release.json` records
the source commit and artifact checksum.

Licensed under OSL-3.0; see [LICENSE](https://github.com/codyps/zpl/blob/main/LICENSE).
Contributor and publication instructions are in the
[release guide](https://github.com/codyps/zpl/blob/main/docs/releases.md#swift-package-publication).
