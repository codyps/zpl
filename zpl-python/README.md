# zplkit

Python bindings for the [codyps/zpl](https://github.com/codyps/zpl) Rust `zpl`
crate, linked directly from this workspace's `zpl/` directory. Parsing and rendering run
locally with no printer, subprocess, network service, or Python runtime
dependencies. The distribution is named `zplkit`; the import is `zplkit`.
Use a separate environment if another package already provides that import.

## Install from this checkout

Python 3.10+ and a Rust toolchain are required to build from source. From the
repository root:

```sh
python3 -m venv .venv
. .venv/bin/activate
python -m pip install ./zpl-python
```

A built wheel needs no Rust toolchain. These bindings use PyO3's Python 3.10
stable ABI. Stable renderer releases automatically publish `zplkit` to PyPI
with the same version, after the one-time
[trusted-publisher setup](../docs/releases.md#pypi-publishing-setup). Once the
first release is published, install it with `python -m pip install zplkit`.

## Render

```python
from pathlib import Path
from zplkit import Options, render

options = Options(profile="specification", width=400, height=200, dpi=203)
document = render(b"^XA^FO20,20^A0N,32,0^FDHello, ZPL!^FS^XZ", options)
for index, label in enumerate(document.labels):
    Path(f"label-{index}.png").write_bytes(label.png())
    Path(f"label-{index}.svg").write_bytes(label.svg())
Path("labels.pdf").write_bytes(document.pdf())
print(document.warnings)
```

`render` accepts `bytes` unchanged or encodes `str` as UTF-8. A string does not
implicitly select `^CI28`; select the appropriate ZPL character encoding yourself.
Use bytes for other printer encodings and binary downloads. Convert mutable
buffers with `bytes(buffer)` before calling.

`Options()` uses the native ZD621 203-DPI defaults. Profiles are `zd621`,
`specification`, and `zq610-plus`; their native initial dimensions and DPI apply
unless overridden. ZPL `^PW` and `^LL` can also change dimensions. The printer
profiles have the same firmware/capture scope and fidelity limitations as the
[Rust renderer](../docs/local-renderer.md).

All compatibility fields are available independently. Configurations are
immutable; `replace` returns a changed copy. `Compatibility()` starts with all
overrides disabled, matching the native type; use an options object to start
from a printer profile:

```python
base = Options(profile="zd621")
options = Options(compatibility=base.compatibility.replace(
    qr_printer_mask_selection=False,
))
```

`document.labels` returns scenes in source order. Each scene has `width`,
`height`, and `dpi`, and encodes to PNG, SVG or single-page PDF bytes. The document
PDF contains all labels in order. Scenes remain usable after the document is
released. Warnings are returned, never silently printed or discarded.

For raw pixels, `scene.rasterize()` returns a raster with `width`, `height`, and
`pixels`: row-major bytes, one byte per dot, 0 black and 255 white. For example,
with optional Pillow installed:

```python
from PIL import Image
raster = document.labels[0].rasterize()
image = Image.frombytes("L", (raster.width, raster.height), raster.pixels)
```

## Parse without rendering

```python
from zplkit.parse import parse, Syntax

source = b"^XA^FO10,20^FDhello^FS^XZ"
result = parse(source)
assert b"".join(bytes(element) for element in result.elements) == source
for element in result.elements:
    print(element.kind, element.offset, element.data)
next_result = parse(b"^XA^XZ", syntax=result.syntax)
```

Kinds are `before_first_command`, `format_command`, `control_command`, and
`control_character`. Elements preserve every byte, including whitespace, unknown
commands, and binary payloads. Offsets are byte offsets, including for UTF-8
strings. `Syntax(format_prefix=94, control_prefix=126, delimiter=44)` controls
initial syntax; the result reports final syntax after changes in the stream.
Framing does not validate operands, establish render support, or authorize
printer operations. Parsing consumes a complete buffer and raises on any framing
error; it does not return partial results.

## Limits and errors

```python
from zplkit.rendering import Limits, RenderError
from zplkit.output import Limits as OutputLimits

try:
    document = render(b"^XA^XZ", limits=Limits(input_bytes=4096, labels=1))
    png = document.labels[0].png(limits=OutputLimits(pixels=2_000_000))
except RenderError as error:
    print(error.offset, error.message)
```

The default resource budgets exactly match the bounded Rust library defaults.
Every field in `render::Limits` and `output::Limits` can be overridden through
keyword arguments or `replace`. Output budgets are independent of rendering
budgets. Floating-point ceilings must be finite and nonnegative. Unknown fields
raise `TypeError`; integer overflow/negative unsigned values raise `OverflowError`.
The parser has no resource budget; callers control the input buffer size.

`zplkit.parse.ParseError` has `offset` and `kind`; `zplkit.rendering.RenderError` has
`offset` and `message`; `zplkit.output.OutputError` preserves the native diagnostic.
All three extend `ValueError`. `zplkit.__version__` is the binding package version;
`zplkit.library_version` identifies the linked renderer. Type hints and `py.typed`
are included. Native parsing, rendering and output encoding release the GIL.

The binding covers command framing, render options and compatibility, resource
limits, rendered documents/scenes, and output adapters. It does not yet expose
custom font-provider callbacks, TrueType/bitmap decoding utilities, arbitrary
scene construction/editing, or custom raster destination callbacks.

## Development and packaging

The package uses [PyO3](https://pyo3.rs/v0.29.3/) and
[Maturin's mixed project layout](https://www.maturin.rs/project_layout.html).
From the repository root, with the virtual environment active:

```sh
python -m pip install 'maturin>=1.15,<2'
maturin develop --locked --manifest-path zpl-python/Cargo.toml
python -m unittest discover -s zpl-python/tests -v
cargo test --locked -p zpl-python
maturin build --locked --release --manifest-path zpl-python/Cargo.toml --out dist
maturin sdist --manifest-path zpl-python/Cargo.toml --out dist
```

The source distribution includes the local Rust dependencies and workspace
lockfile. Maturin trims unrelated workspace members in the archive; installation
allows Cargo to prune their lockfile entries. Checkout wheel builds use `--locked`.
CI builds and installs a wheel, tests it, and rebuilds a wheel from the source
distribution. Release builds publish five native wheels (Linux x86_64/aarch64,
macOS Intel/Apple Silicon, Windows x86_64) and a source archive. Linux wheels
require glibc 2.28 or newer; other platforms can build from source. See the
[release policy](../docs/releases.md#pypi-publishing-setup) for versioning and retries.
OSL-3.0 applies to both the bindings and linked workspace crates.
