# Codyps.Zpl for .NET

Local ZPL parsing and rendering through the same Rust engine as `zpl`. Install
the NuGet package using PackageReference:

```sh
dotnet add package Codyps.Zpl
```

```csharp
using Codyps.Zpl;

var options = RenderOptions.Create(Profile.Zd621);
options.Width = 812;
options.Height = 1218;
using var document = Renderer.Render(
    "^XA^FO20,20^A0N,40,40^FDHello, C#!^FS^XZ", options);
using var label = document.GetLabel(0);
File.WriteAllBytes("label.png", label.ToPng());
File.WriteAllBytes("labels.pdf", document.ToPdf());
```

The package targets .NET 8, .NET Standard 2.0, and .NET Framework 4.7.2.
.NET Standard allows older .NET consumers to use the same API; it does not add
native support for other operating systems. The package bundles native libraries
for Windows x86/x64/ARM64, Linux x64/ARM64/ARMv7 with either glibc or musl
(including Alpine), and macOS x64/ARM64. Consumers need no Rust compiler. The
managed API uses the `Codyps.Zpl` namespace. Linux GNU libraries target glibc
2.28+; musl libraries require musl 1.2+. macOS binaries target macOS 11 or later.
The installed .NET runtime also has its own OS requirements. Windows uses MSVC
and requires the matching Microsoft Visual C++ runtime. Browser Wasm, mobile,
and RISC-V native libraries are not included. See the
[complete platform matrix](https://github.com/codyps/zpl/blob/main/docs/prebuilt-platforms.md)
for the next release's artifact scope and consumer checks.

Native binaries follow NuGet's `runtimes/<rid>/native` conventions; both ordinary
build/run and RID-specific framework-dependent publish are tested in CI with
.NET 8 and a .NET 6 consumer of the .NET Standard assembly.
See [Microsoft's native package guidance](https://learn.microsoft.com/en-us/nuget/create-packages/native-files-in-net-packages).

### .NET Framework

.NET Framework 4.7.2 and later are supported on **Windows x64**, with either
PackageReference or `packages.config`. Set the application's platform target:

```xml
<PropertyGroup>
  <PlatformTarget>x64</PlatformTarget>
</PropertyGroup>
```

The package's MSBuild targets copy `zpl_c.dll` beside the application, including
through transitive PackageReferences. Keep this DLL with the deployed application.
Enable automatic binding redirects for desktop applications that also depend on
other versions of `System.Memory` or its dependencies:

```xml
<AutoGenerateBindingRedirects>true</AutoGenerateBindingRedirects>
```

The .NET Standard and Framework assemblies depend on `System.Memory` for the
span-based API. CI compiles consumers targeting 4.7.2 and 4.8 and runs them on the
Windows runner's installed Framework runtime, using both NuGet project formats.

## API and ownership

- `Renderer.Render(string)` encodes the string as UTF-8. Select `^CI28` in ZPL
  when using UTF-8 text. Pass `ReadOnlySpan<byte>` for legacy encodings, downloaded
  graphics, binary fonts, and embedded NULs. No network or printer access occurs.
- `RenderOptions.Create(profile)` initializes every field from the native profile.
  The default profile is ZD621; `Specification`, `Zq610Plus`, and 300-DPI `Gx430t` are also available.
  `Compatibility` exposes every C ABI setting, with boolean flags represented by
  byte values 0/1 and optional values by `OptionalUInt32`, `OptionalUInt16`, or
  `OptionalFileId`. Configuration layouts are generated from the C header.
- Omitted options/limits select native bounded defaults. Start overrides with
  `RenderLimits.Create()` and `OutputLimits.Create()`; `default`/`new()` are zero
  configurations, not native defaults. Invalid values raise `ZplException`.
- Dispose `Document`, `Scene`, and `Fonts` with `using`. SafeHandle also releases
  abandoned handles. A scene survives disposal of its document; returned bytes,
  warning strings, parser elements, and error messages are managed copies.
  Concurrent immutable operations and renders are supported; access and disposal
  on the same wrapper are serialized.
- `Scene.Info`, `ToPng()`, `ToSvg()`, `ToPdf()`, and `ToGrayscale()` expose output.
  Grayscale is row-major, one byte per dot, 0 black/255 white, without padding.
  `Document.ToPdf()` includes every label.
- `Parser.Parse()` returns copied lossless elements with byte offsets and final
  syntax. It frames input, not validates or authorizes it. Bound input size before
  parsing; renderer resource limits do not apply to this standalone parser.
- `Fonts.AddTrueType(id, bytes)` and `AddTrueType("R:FONT.TTF", bytes)` copy font
  data. IDs are Unicode scalar values, e.g. `(uint)'Z'`. Pass `fonts:` to Render.
  Resident and ZPL-downloaded fonts retain native behavior. Managed bitmap font
  callbacks are not exposed by this wrapper.
- `ZplException.Status` and `ByteOffset` preserve native diagnostics. Offsets are
  null when unavailable. The wrapper checks C ABI 3 before using native structs.
  `Renderer.LibraryVersion` reports the linked renderer version.

## Building and checking locally

Use Rust, Python 3.11+, and the .NET 8 SDK. From the repository root, on Linux x64:

```sh
python3 zpl-c/generate-config.py --check
python3 zpl-dotnet/generate-config.py --check
cargo build --locked --release -p zpl-c
mkdir -p zpl-dotnet/native/linux-x64/native
cp target/release/libzpl_c.so zpl-dotnet/native/linux-x64/native/
dotnet pack zpl-dotnet/Codyps.Zpl/Codyps.Zpl.csproj -c Release -o zpl-dotnet/dist
python3 scripts/dotnet-release.py verify 0.3.0 zpl-dotnet/dist/Codyps.Zpl.0.3.0.nupkg --rid linux-x64
python3 zpl-dotnet/check-package.py zpl-dotnet/dist 0.3.0 linux-x64
```

Substitute the current project version, RID and library filename on other systems.
Set MSBuild `NativeRoot` to use another staging directory. A local package can
contain just the current platform; release verification requires all five native
libraries. CI assembles one complete package, then tests that exact archive on
each platform, with isolated package caches. Install the .NET 6 SDK as well to
run `check-package.py --framework net6.0`. On Windows, the Framework checks are:

```sh
python3 zpl-dotnet/check-package.py zpl-dotnet/dist VERSION win-x64 --framework net472
python3 zpl-dotnet/check-framework.py zpl-dotnet/dist VERSION --framework net472
```

The classic `packages.config` check also requires `nuget` and Visual Studio
`msbuild` on PATH. Substitute `net48` to test the later Framework target.

NuGet versions follow the renderer. The release pipeline builds/tests before
publishing; see [release setup](../docs/releases.md#nuget-publication).
