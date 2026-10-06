# ZPL

**[Open the live ZPL editor and preview →](https://codyps.github.io/zpl/)**

Rust tools for parsing Zebra Programming Language (ZPL), rendering labels locally
to PNG, SVG and PDF, and comparing previews from Zebra printers. The workspace also
includes a browser editor and a printer-backed HTTP proxy. Font research and
extraction tools live in a [separate private repository](https://github.com/codyps/zpl-font-extract).

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
direnv allow
cargo build --workspace
```

The shell supplies Rust, Cargo, rustfmt, Clippy, Diesel CLI, and SQLite, and sets
`ROOT_PATH` and `DATABASE_URL`. Alternatively, use your own Rust toolchain;
the proxy additionally requires Diesel CLI, SQLite, and a configured database URL.

Render the included sample locally:

```sh
cargo run -p zpl-cmd -- render docs/examples/local-label.zpl /tmp/label.svg
cargo run -p zpl-cmd -- render docs/examples/local-label.zpl /tmp/label.png
cargo run -p zpl-cmd -- render docs/examples/local-label.zpl /tmp/label.pdf
```

`zpl-cmd render` selects PNG, SVG or PDF by the output extension.
PDF includes one page per label; PNG and SVG require a single label. Local
rendering does not contact a printer or external service. Run
`cargo run -p zpl-cmd -- render --help` for profile options. To install the CLI
from this checkout, run `cargo install --locked --path zpl-cmd`, then use
`zpl-cmd render input.zpl output.pdf`.

Inspect command boundaries in a ZPL file:

```sh
cargo run -p zpl --example zpl-parse -- test-data/cc.zpl
```

## Use the renderer in Rust

```rust
use zpl::{
    output::{Adapter, Pdf, Png, Svg},
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
    std::fs::write("labels.pdf", Pdf.encode_pages(&document.labels)?)?;
    Ok(())
}
```

The renderer turns ZPL into a path-based scene shared by the PNG, SVG and PDF adapters.
PDF pages retain physical label dimensions using the scene DPI.
Text uses an embedded bitmap capture of resident font 0; other sizes and rotations
can differ from printer output. See [local rendering](docs/local-renderer.md) for
dimensions, resource limits, font fidelity, and custom output adapters.

## Browser preview

**[Try the browser preview on GitHub Pages](https://codyps.github.io/zpl/).**

The static editor in [`site/`](site/) uses `zpl-wasm` to render in the browser,
with selectable labels and PNG/SVG/PDF downloads. ZPL stays in the browser.

For a local build, use a Rust toolchain with rustup, Node.js 22 or later, and Python 3:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128 --locked
bash scripts/build-site.sh
python3 -m http.server 8080 --directory _site
```

Open <http://localhost:8080/>. See [browser preview](docs/web-preview.md) for
browser tests and GitHub Pages deployment.

## Renderer HTTP API

[`zpl-render-api`](zpl-render-api/) runs the local renderer on Cloudflare Workers
with public, rate-limited access. It implements Labelary-style POST URLs for PNG
rendering, including binary uploads and label selection, plus LabelZoom-compatible
ZPL-to-PNG and multipage PDF conversion routes. See the
[API and deployment guide](docs/worker-api.md) for compatibility, limits,
hosting cost comparisons, and local testing.

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
cargo run -- --printers printers.json --bind-addr 127.0.0.1:3000
```

Create `printers.json` from the [named-printer configuration example](docs/proxy-cache.md),
then open <http://127.0.0.1:3000/>. Run the proxy from `zpl-proxy-api/` because its
static asset paths are relative to that directory. Outside the development shell,
set `DATABASE_URL` to an absolute SQLite database path. Pending Diesel migrations
are embedded in the executable and applied automatically before requests are served.

The proxy exposes `POST /api/printers/{name}/preview` and stores submitted ZPL, PNG results,
errors, and request history in SQLite. One proxy can manage multiple named printers; each physical printer must have
only one owning proxy instance. See [cache behavior and refresh controls](docs/proxy-cache.md)
and [telemetry](docs/telemetry.md).

## Workspace

| Crate | Purpose |
| --- | --- |
| [`zpl`](zpl/) | Command-stream parser, local renderer, bitmap font types, PNG/SVG/PDF output |
| [`zpl-cmd`](zpl-cmd/) | Local command-line rendering with `zpl-cmd render` |
| [`zpl-wasm`](zpl-wasm/) | WebAssembly bindings for the browser preview |
| [`zpl-render-api`](zpl-render-api/) | Cloudflare Worker, Labelary/LabelZoom HTTP subsets, compressed PNG and multipage PDF output |
| [`zebra-http-api`](zebra-http-api/) | Printer HTTP client and rendering/comparison examples |
| [`zpl-proxy-api`](zpl-proxy-api/) | Axum proxy, browser interface, SQLite cache and request history |
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

CI uploads the Linux (`x86_64-linux`) and Intel macOS (`x86_64-darwin`)
development shells and their dependency closures to `codyps.cachix.org` on
pushes to every branch, pull requests, and manual CI runs when write credentials
are available. All workflows use GitHub-hosted runners: `ubuntu-24.04` for
Linux and `macos-15-intel` for the Intel macOS shell. Benchmarks run only on Linux.
The shell jobs use the checked-in `flake.lock`. Set the repository secret
`CACHIX_AUTH_TOKEN` to a token with write access to the `codyps` cache.
Jobs without the secret (including fork and Dependabot PRs) still build using
public caches, but skip uploads. Cachix uploads completed derivations as they
build; a successful build also pushes the full shell closure, including
substituted dependencies. This follows the
[nix.dev CI guide](https://nix.dev/guides/recipes/continuous-integration-github-actions).
ARM shells are not covered by these runners. Rust tests, Clippy, and the Pages
build use GitHub Actions Cargo caches, with writes restricted to main; they no
longer require private runner networking or Warbler cache credentials.
The flake configures `codyps.cachix.org` and `nix-community.cachix.org` as extra
cache sources with their public signing keys, preserving Nix's default cache.
Accept the flake's cache settings when prompted, or use
`nix develop --accept-flake-config`. CI accepts these settings explicitly.

`test-data/generate.sh` is a separate external-service
check: it sends fixtures to Labelary and writes PDFs under `test-data/_gen/`.
See [AGENTS.md](AGENTS.md) for contribution and testing conventions.

## License

Licensed under the [Open Software License version 3.0](LICENSE).
