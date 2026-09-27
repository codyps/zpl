# ZPL

**[Open the live ZPL editor and preview →](https://codyps.github.io/zpl/)**

Rust tools for parsing Zebra Programming Language (ZPL), rendering labels locally
to PNG and SVG, and comparing previews from Zebra printers. The workspace also
includes a browser editor, a printer-backed HTTP proxy, and font extraction tools.

Use the [firmware update CLI](docs/firmware-updates.md) to inspect network Zebra
printers and apply local firmware files with model and serial checks.

The parser preserves command bytes, including binary payloads and unknown commands.
The local renderer supports a practical subset of ZPL; unsupported commands return
errors, and approximate font rendering produces warnings. See
[parser coverage](docs/parser-coverage.md), [renderer coverage](docs/local-renderer.md),
and [barcode support](docs/barcodes.md) for the exact boundaries.
[Printer accuracy tests](docs/printer-accuracy.md) pin overpaint, underpaint,
and pixel hashes against checked-in ZD621 previews using the overridable
[ZD621 renderer profile](docs/local-renderer.md#printer-profiles).

## Quick start

Run these commands from the repository root. With Nix and direnv installed, enter
the development environment:

```sh
direnv exec . "$SHELL"
cargo build --workspace
```

The shell supplies Rust, Cargo, rustfmt, Clippy, Diesel CLI, and SQLite, and sets
`ROOT_PATH` and `DATABASE_URL`. Alternatively, use your own Rust toolchain;
the proxy additionally requires Diesel CLI, SQLite, and a configured database URL.

Render the included sample locally:

```sh
cargo run -p zpl --example zpl-to-svg -- docs/examples/local-label.zpl /tmp/label.svg
cargo run -p zpl --example zpl-to-svg -- docs/examples/local-label.zpl /tmp/label.png
```

Despite its name, `zpl-to-svg` writes either format based on the output extension.
It accepts one label per input; the library supports multiple labels. Local
rendering does not contact a printer or external service.

Inspect command boundaries in a ZPL file:

```sh
cargo run -p zpl --example zpl-parse -- test-data/cc.zpl
```

## Use the renderer in Rust

```rust
use zpl::{
    output::{Adapter, Png, Svg},
    render, Options,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = render(
        b"^XA^PW400^LL200^FO20,20^A0N,32,0^FDHello, ZPL!^FS^XZ",
        Options::default(),
    )?;
    for warning in &document.warnings {
        eprintln!("warning: {warning}");
    }
    for (index, scene) in document.labels.iter().enumerate() {
        std::fs::write(format!("label-{index}.png"), Png.encode(scene)?)?;
        std::fs::write(format!("label-{index}.svg"), Svg.encode(scene)?)?;
    }
    Ok(())
}
```

The renderer turns ZPL into a path-based scene shared by the PNG and SVG adapters.
Text uses an embedded bitmap capture of resident font 0; other sizes and rotations
can differ from printer output. See [local rendering](docs/local-renderer.md) for
dimensions, resource limits, font fidelity, and custom output adapters.

## Browser preview

**[Try the browser preview on GitHub Pages](https://codyps.github.io/zpl/).**

The static editor in [`site/`](site/) uses `zpl-wasm` to render in the browser,
with selectable labels and PNG/SVG downloads. ZPL stays in the browser.

For a local build, use a Rust toolchain with rustup, Node.js 22 or later, and Python 3:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128 --locked
bash scripts/build-site.sh
python3 -m http.server 8080 --directory _site
```

Open <http://localhost:8080/>. See [browser preview](docs/web-preview.md) for
browser tests and GitHub Pages deployment.

## Printer-backed previews

To inventory and recapture the saved `zpl` and `zpl-comparison` previews at the
ZQ610 Plus's 384-dot width, see [printer recapture](docs/printer-recapture.md).
The tool saves source/submission bytes, printer identity, timestamps and SHA-256
provenance separately from the existing test baselines.

To request a PNG from a Zebra printer's HTTP preview API, replace `printer.local`
with your printer's address:

```sh
cargo run -p zebra-http-api --example zebra-render -- \
  --host http://printer.local/ docs/examples/local-label.zpl /tmp/printer-label.png
```

For a socket-activated Linux service with TCP or Unix sockets, use the
[NixOS module](docs/nixos.md).

To run the proxy and its browser interface, start in the repository root inside
the development shell:

```sh
mkdir -p _db
cd zpl-proxy-api
diesel migration run
cargo run -- --zd621-url http://printer.local/ --bind-addr 127.0.0.1:3000
```

Open <http://127.0.0.1:3000/>. Run the proxy from `zpl-proxy-api/` because its
static asset paths are relative to that directory. Outside the development shell,
set `DATABASE_URL` to an absolute SQLite database path before running migrations.

The proxy exposes `POST /api/zpl-zd621` and stores submitted ZPL, PNG results,
errors, and request history in SQLite. Run one proxy instance per printer because
the printer preview object is shared. See [cache behavior and refresh controls](docs/proxy-cache.md)
and [telemetry](docs/telemetry.md).

## Workspace

| Crate | Purpose |
| --- | --- |
| [`zpl`](zpl/) | Command-stream parser, local renderer, bitmap font types, PNG/SVG output |
| [`zpl-wasm`](zpl-wasm/) | WebAssembly bindings for the browser preview |
| [`zebra-http-api`](zebra-http-api/) | Printer HTTP client and rendering/comparison examples |
| [`zpl-proxy-api`](zpl-proxy-api/) | Axum proxy, browser interface, SQLite cache and request history |
| [`zpl-font-extract`](zpl-font-extract/) | Resident-font sampling, bitmap extraction, export, and verification |
| [`raster-diff`](raster-diff/) | Raster decoding, pixel comparisons, and the `png-diff` CLI |

Shared fixtures live in [`test-data/`](test-data/). The
[documentation index](docs/README.md) links the bundled ZPL reference, font
extraction and reconstruction studies, rendering comparisons, and release setup.

## Development

Run from the repository root in the development shell:

```sh
cargo test --workspace
cargo test -p zpl --lib
cargo fmt --all -- --check
taplo fmt --check
cargo clippy --workspace --all-targets
```

Use `taplo fmt` to automatically format TOML files and `nix fmt` for Nix files.
CI checks TOML formatting with the same Taplo configuration.
`test-data/generate.sh` is a separate external-service
check: it sends fixtures to Labelary and writes PDFs under `test-data/_gen/`.
See [AGENTS.md](AGENTS.md) for contribution and testing conventions.

## License

Licensed under the [Open Software License version 3.0](LICENSE).
