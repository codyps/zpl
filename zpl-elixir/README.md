# Zpl for Elixir

Elixir bindings for the [Rust `zpl` crate](https://github.com/codyps/zpl), using
[Rustler 0.38](https://hexdocs.pm/rustler/). Parsing, rendering, and output encoding
run locally on BEAM dirty CPU schedulers. No printer or external service is used.

## Installation

Requires Elixir 1.15 or newer and Erlang/OTP 25 or newer. Add `zpl` to the
`deps` list in your application's `mix.exs`:

```elixir
{:zpl, "~> 0.2.1"}
```

Then run `mix deps.get`. The Mix application is `:zpl` and the public module
is `Zpl`.

**Release 0.2.1 and earlier compile from source** and require a stable Rust
toolchain and a native linker. Later releases use checksum-verified precompiled
NIFs on the following targets:

| Platform | Minimum build baseline |
| --- | --- |
| Linux x86_64 (GNU libc) | Ubuntu 22.04 / glibc 2.35 |
| Linux ARM64 (GNU libc) | Ubuntu 24.04 / glibc 2.39 |
| macOS Intel / Apple Silicon | macOS 15 |
| Windows x86_64 | MSVC, Windows Server 2025 build runner |

Prebuilts use NIF ABI 2.15, compatible with the supported OTP 25+ runtimes.
Releases with prebuilts need no Rust toolchain on these targets. Downloads
come from the matching `zpl-v<version>` GitHub release and are verified against
SHA-256 checksums inside the Hex package.
An unavailable target, failed download, or checksum mismatch fails explicitly;
it does not silently switch to compiling Rust.

To disable prebuilts (also required for other targets or older libc/macOS):

```sh
ZPL_BUILD=true mix deps.compile zpl --force
```

Set `ZPL_BUILD=true` throughout fresh builds, or put this in `config/config.exs`:

```elixir
config :rustler_precompiled, :force_build, zpl: true
```

The standard `RUSTLER_PRECOMPILED_FORCE_BUILD_ALL=true` override also works.
Source builds require Rust and a native linker. For an offline install, populate
`RUSTLER_PRECOMPILED_GLOBAL_CACHE_PATH` with the matching `.tar.gz` release asset;
checksums are still enforced. A mirror can be selected at compile time with
`config :zpl, :precompiled_base_url, "https://mirror.example/zpl-v<version>/"`.

## Quick start

```elixir
document = Zpl.render!(
  "^XA^FO20,20^A0N,32,0^FDHello, ZPL!^FS^XZ",
  profile: :specification, width: 400, height: 200, dpi: 203
)

for {scene, index} <- Enum.with_index(document.labels) do
  File.write!("label-#{index}.png", Zpl.Scene.encode!(scene, :png))
  File.write!("label-#{index}.svg", Zpl.Scene.encode!(scene, :svg))
end
File.write!("labels.pdf", Zpl.Document.pdf!(document))
IO.inspect(document.warnings)
```

`Zpl.render/3` returns `{:ok, %Zpl.Document{}}` or `{:error, %Zpl.Error{}}`.
The `!` variants raise the same error. Input is a binary, preserving all bytes,
including NULs, printer encodings, and binary downloads. Elixir strings are UTF-8
binaries; select `^CI28` explicitly when that is the intended printer encoding.

The profiles are `:zd621` (default), `:specification`, and `:zq610_plus`.
Dimensions are dots; DPI controls physical output size. ZPL `^PW` and `^LL` can
change label dimensions according to the selected profile. Fidelity and capture
scope are exactly those of the
[Rust renderer](https://github.com/codyps/zpl/blob/main/docs/local-renderer.md).

Options, compatibility overrides, limits, and parser syntax accept maps or keyword
lists with atom keys. Omitted values use native defaults. `Zpl.options(profile)`
returns a complete options map, including that profile's compatibility flags.
`Zpl.compatibility()` returns all flags disabled. Every native compatibility
field is exposed independently; partial overrides merge into the selected profile:

```elixir
{:ok, document} = Zpl.render(source,
  profile: :zd621,
  compatibility: [qr_printer_mask_selection: false]
)
```

Optional integers accept `nil`; `:macro_pdf417_file_id` accepts `nil` or a tuple
of three unsigned 16-bit integers. Unknown fields and out-of-range integers fail.

Scenes provide `width`, `height`, and `dpi`, and these operations:

- `Zpl.Scene.png/2`, `svg/2`, `pdf/2`: `{:ok, binary}` or an error.
- `Zpl.Scene.encode/3` and `encode!/3`: select `:png`, `:svg`, or `:pdf`.
- `Zpl.Scene.rasterize/2` and `rasterize!/2`: a `%Zpl.Raster{width:, height:, pixels:}`.
- `Zpl.Document.pdf/2` and `pdf!/2`: every label, in source order.

Raw pixels are row-major grayscale bytes: 0 black, 255 white, one byte per dot.
Scene references remain valid after the document or its creating process is
released, and can be used concurrently by processes in the same VM. References
are opaque native resources: do not serialize them or send them to another node.
Changing Elixir metadata fields does not mutate the native scene/document.

## Lossless parsing

```elixir
source = "^CC!!XA!CD;!FO1;2!XZ"
result = Zpl.parse!(source)
^source = IO.iodata_to_binary(Enum.map(result.elements, & &1.data))
continued = Zpl.parse!("!XA!XZ", result.syntax)
```

`Zpl.parse/2` returns `{:ok, %Zpl.ParseResult{elements:, syntax:}}`.
Each `%Zpl.Element{kind:, offset:, data:}` preserves the original bytes and byte
offset. Kinds are `:before_first_command`, `:format_command`, `:control_command`,
and `:control_character`. `Zpl.syntax()` returns native initial syntax;
`:format_prefix`, `:control_prefix`, and `:delimiter` accept integers 0–255.
The result contains final syntax after in-stream changes.

Parsing consumes one complete buffer, returning an error rather than partial
results on malformed framing. It has no resource budget; callers control input
size. Element data uses sub-binaries, so retaining an element may retain the
source binary. Parsing does not validate operands, prove render support, or
authorize commands for printer submission.

## Limits and errors

```elixir
with {:ok, document} <- Zpl.render(source, [], input_bytes: 4096, labels: 1),
     {:ok, png} <- Zpl.Scene.png(hd(document.labels), pixels: 2_000_000) do
  File.write!("label.png", png)
else
  {:error, %Zpl.Error{} = error} -> IO.inspect(error)
end
```

`Zpl.render_limits()` and `Zpl.output_limits()` expose all native defaults.
Every field in Rust's `render::Limits` and `output::Limits` is configurable.
Output budgets are independent of render budgets. Floating-point ceilings must
be finite and nonnegative; Elixir integers are also accepted for these ceilings.

`Zpl.Error` has `stage` (`:argument`, `:parse`, `:render`, or `:output`), `message`,
and optional `offset` and parser `kind`. Native diagnostics are preserved.
Incorrect Elixir call shapes (non-binary input, non-atom keys, or forged native
references) raise standard Elixir argument/function errors.
`Zpl.library_version()` identifies the linked Rust library. Published Hex packages
use the same version as the renderer.

The bindings cover framing, rendering options/profiles/compatibility, limits,
documents, scenes, and output adapters. They do not expose custom font-provider
callbacks, font decoding utilities, arbitrary scene construction/editing, or
custom raster destinations.

## Links and license

- [Module documentation in source](https://github.com/codyps/zpl/tree/main/zpl-elixir/lib)
- [Renderer coverage](https://github.com/codyps/zpl/blob/main/docs/local-renderer.md)
- [Barcode support](https://github.com/codyps/zpl/blob/main/docs/barcodes.md)
- [Source and issues](https://github.com/codyps/zpl)
- [Contributor build instructions](https://github.com/codyps/zpl/blob/main/docs/releases.md#elixir)

The package and bundled workspace sources are licensed under the
[Open Software License 3.0](https://github.com/codyps/zpl/blob/main/LICENSE).
