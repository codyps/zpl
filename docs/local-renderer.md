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
832 × 1218 dots at 203 DPI; `^PW` and `^LL` override dimensions. The CLI takes
one label and selects the adapter by output extension:

```sh
direnv exec . cargo run -p zpl --example zpl-to-svg -- docs/examples/local-label.zpl /tmp/label.svg
direnv exec . cargo run -p zpl --example zpl-to-svg -- docs/examples/local-label.zpl /tmp/label.png
```

## Printer profiles

`zpl::render::profiles::SPECIFICATION` explicitly disables every printer
compatibility override and is used by specification-based tests.
`Options::default()` returns `ZD621_203_DPI`, including its compatibility
overrides. To select the captured ZD621 HTTP preview behavior explicitly, start
with the printer's initial options:

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

| `options.compatibility` field | Specification profile | ZD621 profile |
| --- | --- | --- |
| `qr_fo_uses_by_height` | QR ink begins at `^FO` | Offset by `^BY` height minus one dot |
| `qr_ft_includes_margin` | QR base at `^FT` | Include three modules minus one dot below symbol |
| `diagonal_dot_runs` | Diagonal band clipped to `^GD` box | Firmware horizontal runs, possibly outside box |
| `postal_fixed_pitch` | Use `^BY` ratio | Truncated 2.5-module pitch |
| `intelligent_mail_outward_rounding` | Fractional tracker thirds | Round tracker edges outward |
| `retail_guard_extension_dots` | `None`: five modules | `Some(13)`: 13 dots |
| `code93_normalize_input` | Reject bytes outside the ZPL alphabet/shift substitutes | Uppercase raw letters and discard unsupported bytes |
| `codablock_a_row_height_in_dots` | Multiply row height by module width | Row height operand is dots |
| `codablock_a_wrapping_checks` | Mathematical weighted sums modulo 43 | Wrap sums at 16 bits first |
| `code128_above_text_keeps_bar_origin` | Above-text included in field extent | Bars retain origin; text extends above |

The origin, ratio, and Code 93 rules follow the Zebra guide (`^FO` p. 201,
`^FT` p. 205 Table 7, `^BZ` p. 150, `^BA` pp. 87–89). The specification profile
retail guard extension follows [ISO/IEC 15420:2009](https://www.iso.org/standard/46143.html),
§4.3.3. The guide does not define exact diagonal scan conversion or Intelligent
Mail dot rounding; those options select empirical quantization choices.
See [printer accuracy](printer-accuracy.md) for capture evidence and remaining
gaps. Neither profile implies complete specification or printer
parity. Preview width adjustment remains unimplemented.

## Supported subset

| Area | Commands and restrictions |
| --- | --- |
| Framing | `XA`, `XZ`, `FS`, single-byte equivalents, `CC`, `CT`, `CD`, comments `FX`; ASCII parameter delimiter |
| Layout | `PW`, `LL`, `LH`, `LS`, `LT`, `FO`, explicit-coordinate `FT`, `FW`, `PO`, `FR`, `LR`; left/right/Latin-auto field justification |
| Text | `CF`, `A0`, `AA`, `AB`, `AC`, `AD`, `AE`, `AF`, `AG`, `AH`, `FD`, `FV`, `FH`; printable ASCII in fonts 0/A/B/C/D/E/F/G/H and captured font-0 é; `CI13` ASCII, `CI27` Latin-1 subset and `CI28` UTF-8 |
| Blocks | `FB`: left/center/right/justified alignment, hanging indent, wrapping, explicit `\&` breaks and last-row overflow; word hyphenation and negative line spacing return errors |
| Shapes | `GB` including rounded corners, `GC`, `GE`, `GD`; black or white outlines/fills |
| Graphics | `~DG`, `XG`, `GFA`, `GFB`: raw hex, Zebra ASCII run lengths/row shortcuts, B64, Z64; CRC16 and zlib checksums checked |
| Barcodes | Original per-code linear, matrix, stacked, and postal encoders; `BY`; see [coverage, limitations, specifications, and decoder tests](barcodes.md) |

Text uses [embedded resident font 0](../zpl/assets/README.md), captured from the
ZD621 preview at 32 dots and 203 DPI. All 95 printable ASCII glyphs, including
lowercase, retain their measured advances, bearings and baseline offsets. The
renderer defaults to font 0 at 20 dots. `^CF0,32` or `^A0N,32,0` selects the
captured size; omitted/zero width is proportional to height. Resident A/B/C/D/E/F/G/H use native bitmap matrices with integer magnification; C shares D's matrix, B renders lowercase input as uppercase, and H renders lowercase as advancing blanks. A single bitmap dimension determines the other proportionally; omitted A
dimensions inherit the CF request. The printer profile's
`bitmap_cf_font_only_resets_size` option resets a font-only bitmap CF command
to native size; SPECIFICATION retains the previous size. Other resident font
IDs and uncaptured glyphs return errors.

The 4,365-byte strike is compiled into the binary. Its pixels become horizontal
filled path runs, shared by PNG and SVG. Overlapping glyph strokes are merged.
Field blocks wrap and align using proportional advances, including spaces.
`^FT` uses the captured baseline; `^FO` uses the font matrix. Captured strikes also cover natural-width 16/20/64-dot text and 32-dot text at
widths 16/24/64. Unsampled sizes use scaled bitmap paths and emit a warning. Rotation can also differ slightly from
the printer's outline rasterizer, so it emits the same approximation warning.

Normal-orientation text at 32 × 32 dots matches all captured atlas pages and a
held-out printer label pixel-for-pixel. Native-size layout tests cover baselines,
blocks, and rotation origins. A broader rotated-curve sample retains three edge
pixel differences, recorded as a regression bound. This is a bitmap strike, not
an extracted scalable outline font.

Leave adequate clear space around barcode fields; quiet zones are not inserted
automatically. Barcode interpretation defaults to module-scaled resident A, with explicit `^A` font overrides. `^CV` validation panels
use the separately sampled fixed validation lettering. `^CVY` persists across
labels in the same call; unsupported encoder semantics remain errors.

Legacy Data Matrix ECC 000–140 and `^FM` structured append for PDF417 and
MicroPDF417 are supported. See [barcode scope](barcodes.md) for limits and the
printer options controlling observed departures.

Examples of explicit errors include Code 128 extended-byte FNC4, downloaded fonts, stored formats, serialization,
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

ZD621 preview refinements also expose independent options to ignore `^LT` and
`^PO` in previews, include the firmware's trailing-space field-block alignment,
interpret CODABLOCK F/E row heights in dots, and fit its rows to actual data.
The specification profile disables each override. These choices are tested
against the raw captures in `accuracy-refinements-zd621-v1`.

Font/caption controls additionally verify the independently selectable
`barcode_interpretation_printer_layout` gaps and
`right_justified_inverted_text_uses_ink_margin` origin adjustment. See
[resident-font controls](../zpl/tests/fixtures/resident-fonts-zd621-v1/README.md).

[Resident F and bitmap-caption controls](../zpl/tests/fixtures/resident-f-zd621-v1/README.md)
add 41 exact frames. Explicit A/B/C/D/F Code 128 captions use the selectable
`barcode_reverse_interpretation_shift` in I/B orientation. The independent
`code128_fo_uses_bar_width` option anchors the FO rotation pivot at the bars,
including captions wider than the symbol. Both are enabled in the printer
profile and disabled in SPECIFICATION.

[Resident E controls](../zpl/tests/fixtures/resident-e-zd621-v1/README.md) verify
33 exact frames for full ASCII OCR-B text, sizing, placement and captions.
[Field-block controls](../zpl/tests/fixtures/field-block-overflow-zd621-v1/README.md)
verify specified last-row overprinting and explicit-break alignment: 24 bitmap
frames are exact and 12 font-0 frames score at least 97.98% text IoU, with every
remaining pixel difference pinned.

[Resident G controls](../zpl/tests/fixtures/resident-g-zd621-v1/README.md)
add 72 exact frames for the 95-character native strike, scaled text, origins,
justification, CF/A defaults, wrapping and wide Code 128 captions. G uses the
existing bitmap compatibility options in the printer profile.

[Resident H controls](../zpl/tests/fixtures/resident-h-zd621-v1/README.md)
add 41 exact frames, including native OCR-A extraction, scaling, placement,
captions and lowercase blanks with measured advances. The complete resident
bitmap set A through H is now available for the captured ASCII inputs.
