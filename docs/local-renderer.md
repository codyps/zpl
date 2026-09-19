# Local renderer

The renderer is entirely local and adds no runtime dependencies. It is a practical
preview implementation, not a complete Zebra printer emulator. Unsupported
commands and unsupported parameter modes return `RenderError` with a byte offset;
no partial document is returned on failure. Nothing is sent to a printer.

## Architecture and use

```text
ZPL bytes → parse::ParseContext → render::render → output::Scene
                                                ├─ output::Svg
                                                ├─ output::Png
                                                └─ your output::Adapter
```

A scene contains dimensions, DPI, and ordered filled paths in printer dots.
Paths contain moves, lines, cubic Béziers, and closes, with even-odd filling.
Each drawing paints black, white, or inverts the pixels beneath it. Text glyphs,
barcode bars, and downloaded bitmaps become paths; adapters never interpret ZPL.
SVG uses paths and isolated difference blending for inversion. PNG uses a
pixel-center scan converter, then grayscale PNG with stored DEFLATE blocks.
PNG files prioritize simplicity over compression. Curves are flattened for PNG;
SVG viewers may antialias edges differently.

```rust
use zpl::{
    output::{Adapter, Png, Svg},
    render, Options,
};
let document = render(b"^XA^FO20,20^FDHELLO^FS^XZ", Options::default())?;
for scene in &document.labels {
    let png = Png.encode(scene)?;
    let svg = Svg.encode(scene)?;
    // Save or serve these byte buffers using your application's transport.
}
```

The library returns all labels and preview warnings. Options default to
812 × 1218 dots at 203 DPI; `^PW` and `^LL` override dimensions. The CLI takes
one label and selects the adapter by output extension:

```sh
direnv exec . cargo run -p zpl --example zpl-to-svg -- docs/examples/local-label.zpl /tmp/label.svg
direnv exec . cargo run -p zpl --example zpl-to-svg -- docs/examples/local-label.zpl /tmp/label.png
```

## Printer profiles

`Options::default()` disables empirical printer overrides. To match the captured
ZD621 HTTP preview behavior, start with the printer's initial options:

```rust
use zpl::{render, render::profiles::ZD621_203_DPI};
let mut options = ZD621_203_DPI;
options.height = 300;
options.compatibility.qr_fo_uses_by_height = false;
let document = render(b"^XA^FO20,20^BQN,2,3^FDQA,HELLO^FS^XZ", options)?;
```

The profile is an ordinary `Options` value, so every setting remains independently
overridable. It targets a 203-DPI ZD621 running V93.21.33Z, with initial dimensions
832 × 1218 dots. Changing DPI does not recalibrate firmware-specific dot counts.
`^PW` and `^LL` still override dimensions.

| `options.compatibility` field | Default behavior | ZD621 profile |
| --- | --- | --- |
| `qr_fo_uses_by_height` | QR ink begins at `^FO` | Offset by `^BY` height minus one dot |
| `qr_ft_includes_margin` | QR base at `^FT` | Include three modules minus one dot below symbol |
| `diagonal_dot_runs` | Diagonal band clipped to `^GD` box | Firmware horizontal runs, possibly outside box |
| `postal_fixed_pitch` | Use `^BY` ratio | Truncated 2.5-module pitch |
| `intelligent_mail_outward_rounding` | Fractional tracker thirds | Round tracker edges outward |
| `retail_guard_extension_dots` | `None`: five modules | `Some(13)`: 13 dots |
| `code93_normalize_input` | Reject bytes outside the ZPL alphabet/shift substitutes | Uppercase raw letters and discard unsupported bytes |

The origin, ratio, and Code 93 rules follow the Zebra guide (`^FO` p. 201,
`^FT` p. 205 Table 7, `^BZ` p. 150, `^BA` pp. 87–89). The default standalone
retail guard extension follows [ISO/IEC 15420:2009](https://www.iso.org/standard/46143.html),
§4.3.3. The guide does not define exact diagonal scan conversion or Intelligent
Mail dot rounding; those options select empirical quantization choices.
See [printer accuracy](printer-accuracy.md) for capture evidence and remaining
gaps. Neither defaults nor the profile imply complete specification or printer
parity. Preview width adjustment remains unimplemented.

## Supported subset

| Area | Commands and restrictions |
| --- | --- |
| Framing | `XA`, `XZ`, `FS`, single-byte equivalents, `CC`, `CT`, `CD`, comments `FX`; ASCII parameter delimiter |
| Layout | `PW`, `LL`, `LH`, `LS`, `LT`, `FO`, explicit-coordinate `FT`, `FW`, `PO`, `FR`, `LR`; left field justification |
| Text | `CF0`, `A0`, `FD`, `FV`, `FH`; ASCII glyphs only; `CI0/27/28` without remapping |
| Blocks | `FB`: left/center/right alignment, word wrapping, explicit `\&` breaks; overflow, hyphenation, hanging indent and justified text return errors |
| Shapes | `GB` including rounded corners, `GC`, `GE`, `GD`; black or white outlines/fills |
| Graphics | `~DG`, `XG`, `GFA`, `GFB`: raw hex, Zebra ASCII run lengths/row shortcuts, B64, Z64; CRC16 and zlib checksums checked |
| Barcodes | Original per-code linear, matrix, stacked, and postal encoders; `BY`; see [coverage, limitations, specifications, and decoder tests](barcodes.md) |

Text uses [embedded resident font 0](../zpl/assets/README.md), captured from the
ZD621 preview at 32 dots and 203 DPI. All 95 printable ASCII glyphs, including
lowercase, retain their measured advances, bearings and baseline offsets. The
renderer defaults to font 0 at 20 dots. `^CF0,32` or `^A0N,32,0` selects the
captured size; omitted/zero width is proportional to height. Other resident font
IDs and unsupported glyphs return errors.

The 4,365-byte strike is compiled into the binary. Its pixels become horizontal
filled path runs, shared by PNG and SVG. Overlapping glyph strokes are merged.
Field blocks wrap and align using proportional advances, including spaces.
`^FT` uses the captured baseline; `^FO` uses the font matrix. Other sizes use
scaled bitmap paths and emit a warning. Rotation can also differ slightly from
the printer's outline rasterizer, so it emits the same approximation warning.

Normal-orientation text at 32 × 32 dots matches all captured atlas pages and a
held-out printer label pixel-for-pixel. Native-size layout tests cover baselines,
blocks, and rotation origins. A broader rotated-curve sample retains three edge
pixel differences, recorded as a regression bound. This is a bitmap strike, not
an extracted scalable outline font.

Leave adequate clear space around barcode fields; quiet zones are not inserted
automatically. Barcode interpretation text also uses embedded font 0.

Examples of explicit errors include unsupported barcode variants, Code 128 invocation
sequences and UCC/automatic modes, downloaded fonts, stored formats, serialization,
compressed binary `GFC`, and printer configuration commands.
The parser still frames these commands; rendering coverage is separate from
command-stream parsing coverage. Configuration persists only within one `render`
call. Each field must end with `FS` before another drawing command.

## Limits and validation

Limits: 1 MiB input, 64 labels, one million total scene segments, one million
stored-graphic segments, 4,096 bytes per text field, 25,000 decoded bytes per
graphic, and 32 Mi pixels per image. PNG also limits scan work and curve
flattening. Limit violations return errors. These are resource bounds, not a
claim that rendering arbitrary hostile inputs is constant-time.

Run `direnv exec . cargo test -p zpl`. Tests cover geometry, clipping, inversion,
rotation, framing changes, multiple labels, barcode modules, malformed commands,
and independent Python-zlib fixtures for stored/fixed/dynamic DEFLATE blocks.
`zpl/tests/fixtures/*.zlib` encode `bytes(range(256)) * 200`, generated using
Python's `zlib.compressobj` at level 0 or 9, with default or fixed strategy.

Semantics are based on the [bundled Zebra guide](README.md), especially field,
shape, and graphic commands. Output formats follow the
[PNG specification](https://www.w3.org/TR/png-3/),
[RFC 1950](https://www.rfc-editor.org/rfc/rfc1950),
[RFC 1951](https://www.rfc-editor.org/rfc/rfc1951), and
[SVG compositing](https://www.w3.org/TR/compositing-1/).
