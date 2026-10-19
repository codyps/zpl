# Prebuilt package platforms

These matrices describe the next packages built from this checkout. Published
versions retain their original artifacts; adding platforms requires a new release.

Elixir starts with all ten defaults in
[RustlerPrecompiled 0.9](https://hexdocs.pm/rustler_precompiled/0.9.0/RustlerPrecompiled.html#module-options).
Python maps these to CPython wheel tags and uses MSVC on Windows. .NET maps them
to [portable runtime identifiers](https://learn.microsoft.com/en-us/dotnet/core/rid-catalog)
where a supported .NET runtime exists. Both also include Windows x86/ARM64 and
Linux ARMv7 musl.

| OS / libc | Architecture | Elixir NIF | Python wheel | .NET native RID |
| --- | --- | --- | --- | --- |
| Linux / glibc | x86_64 | `x86_64-unknown-linux-gnu` | `manylinux_2_28_x86_64` | `linux-x64` |
| Linux / glibc | ARM64 | `aarch64-unknown-linux-gnu` | `manylinux_2_28_aarch64` | `linux-arm64` |
| Linux / glibc | ARM hard-float | `arm-unknown-linux-gnueabihf` (ARMv6+) | `manylinux_2_28_armv7l` | `linux-arm` (ARMv7+) |
| Linux / glibc | RISC-V 64 | `riscv64gc-unknown-linux-gnu` | `manylinux_2_39_riscv64` | Not shipped |
| Linux / musl (Alpine) | x86_64 | `x86_64-unknown-linux-musl` | `musllinux_1_2_x86_64` | `linux-musl-x64` |
| Linux / musl (Alpine) | ARM64 | `aarch64-unknown-linux-musl` | `musllinux_1_2_aarch64` | `linux-musl-arm64` |
| Linux / musl (Alpine) | ARMv7 hard-float | Not shipped | `musllinux_1_2_armv7l` | `linux-musl-arm` |
| macOS | Intel | `x86_64-apple-darwin` | `macosx_*_x86_64` | `osx-x64` |
| macOS | Apple Silicon | `aarch64-apple-darwin` | `macosx_*_arm64` | `osx-arm64` |
| Windows / MSVC | x86_64 | `x86_64-pc-windows-msvc` | `win_amd64` | `win-x64` |
| Windows / GNU | x86_64 | `x86_64-pc-windows-gnu` | Uses MSVC | Uses MSVC |
| Windows / MSVC | x86 | Not shipped | `win32` | `win-x86` |
| Windows / MSVC | ARM64 | Not shipped | `win_arm64` | `win-arm64` |

Linux GNU NIFs and NuGet libraries target glibc 2.28 through Zig, independently
of the runner's installed libc. Python GNU wheels target glibc 2.28, except
RISC-V, which follows PyPA's `manylinux_2_39` baseline. Musl wheels target
`musllinux_1_2`; use musl 1.2+ with a runtime-supported Alpine release. Musl
shared libraries link libc dynamically (`-C target-feature=-crt-static`). They
are separate artifacts from GNU builds; no glibc compatibility layer is required.
See [PyPA's Linux images](https://github.com/pypa/manylinux) and
[Maturin's distribution rules](https://www.maturin.rs/distribution.html).

Python wheels use `cp310-abi3` for CPython 3.10+ with the GIL. Elixir uses NIF
ABI 2.15 with Elixir 1.15+/OTP 25+. .NET ships .NET 8, .NET Standard 2.0, and
.NET Framework 4.7.2 assemblies; Framework consumers currently require Windows
x64. A native RID does not extend the selected .NET runtime's OS support.
The .NET and Elixir Apple libraries target macOS 11+. Windows MSVC libraries require the matching Visual C++ runtime.

## Other packages

- Node (`@codyps/zpl`) embeds a portable Wasm binary, so it needs no separate
  native architecture or libc package. Its Node.js 22+ requirement still applies.
- Go's default wazero backend embeds Wasm and does not require libc or cgo.
  The experimental cgo/purego backends require a separately built native library;
  they do not distribute native prebuilts.
- Swift distributes an XCFramework with macOS Intel/ARM64, iOS ARM64 device,
  and iOS Intel/ARM64 simulator slices (macOS 11 / iOS 13 minimum). A separate
  static-library artifact bundle covers Linux glibc/musl and Windows on x86_64
  and ARM64. Linux requires Swift 6.2+, Windows Swift 6.3.3+; musl consumers use
  the Swift static Linux SDK and run on Alpine. Catalyst, watchOS, tvOS, and
  visionOS remain outside the matrix.
- The C ABI crate is built for NuGet and Swift packaging; there is no separate
  registry package of C shared-library prebuilts.

## Release checks

[`scripts/prebuilt_targets.py`](../scripts/prebuilt_targets.py) defines the
required artifact sets. Release gates reject missing, extra, duplicate-platform,
and wrong-ABI Python artifacts, missing NuGet RIDs, and incomplete Elixir NIF
archives. Matrix regression tests keep CI and Elixir download selection aligned
with those gates. Existing immutable publication and checksum checks still apply.

CI installs the exact wheels and NuGet package into independent consumers. Linux
musl consumers run on Alpine for x86_64/ARM64 (and Python/.NET ARMv7); foreign
ARM and RISC-V consumers use QEMU. Elixir checks downloads and Mix releases with
Cargo/rustc blocked, including both Windows ABIs and the Alpine/foreign consumers.
Swift tests macOS/Linux/Windows on both desktop architectures and the iOS
simulator, plus static SDK consumers on Alpine x86_64/ARM64.
Cross-compilation success alone does not establish runtime compatibility.
