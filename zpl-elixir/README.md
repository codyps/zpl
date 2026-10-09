# Zpl for Elixir

Elixir bindings for the [Rust `zpl` crate](https://github.com/codyps/zpl), using
[Rustler 0.38](https://hexdocs.pm/rustler/). Parsing, rendering, and output encoding
run locally on BEAM dirty CPU schedulers. No printer or external service is used.

## Install from a checkout

Requires Elixir 1.15+, Erlang/OTP 25+, a current stable Rust toolchain, and a native
linker. Add this dependency to your application's `mix.exs`, pointing at a complete
checkout (the binding links directly to sibling workspace crates):

```elixir
{:zpl, path: "../zpl/zpl-elixir"}
```

Then run `mix deps.get`. Mix compiles the native library automatically; production
builds use Rust's release profile. The Mix/OTP application is `:zpl`, the public
module is `Zpl`, and the workspace Cargo package is `zpl_elixir`.
Stable renderer releases automatically publish the same version to Hex after the
one-time [publishing setup](https://github.com/codyps/zpl/blob/main/docs/releases.md#hex-publishing-setup).
Once the first release is published, use `{:zpl, "~> <released-version>"}`.
The package does not ship precompiled NIFs.

## Render and encode

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
`Zpl.library_version()` identifies the linked Rust library; the Mix package has
its own version.

The bindings cover framing, rendering options/profiles/compatibility, limits,
documents, scenes, and output adapters. They do not expose custom font-provider
callbacks, font decoding utilities, arbitrary scene construction/editing, or
custom raster destinations.

## Develop and package

From this directory:

```sh
mix deps.get
mix format --check-formatted
mix test
cargo clippy --locked -p zpl_elixir --all-targets -- -D warnings
```

ExUnit checks binary framing, diagnostics, limits, exact raster pixels, resource
lifetime/concurrency, and byte-for-byte PNG/SVG/PDF/raster parity against direct
Rust calls for every profile. The parity test builds the `reference` Rust example.
These test the binding contract; printer accuracy remains in the Rust corpus.

To build a portable Hex source archive, run from the repository root:

```sh
cargo fetch --locked
python3 scripts/elixir-package.py zpl-elixir/_package
cd zpl-elixir/_package
mix deps.get
mix hex.build
```

Use a new staging directory on each run. The staging script includes the runtime
sources of `zpl`, `zpl-bitmap-fonts`, and `raster-diff`, copies the root lockfile,
and lets Cargo prune unrelated entries offline. This creates a self-contained
workspace without fetching older published renderer sources. No crate-local
lockfile is maintained in the checkout. Build archives from the staged directory;
running `mix hex.build` directly in the checkout is rejected because its sibling
Rust crates would be omitted.
The archive includes tests and the native parity example. CI unpacks it outside
the repository and runs the same tests. The release workflow stamps the renderer version, tests the archive on two
Elixir/OTP versions, and uploads the exact artifact in a separate publishing job.
See the release guide for initial account setup and checksum-checked retries.
OSL-3.0 covers the package and its bundled workspace sources.
