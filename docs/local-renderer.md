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
                                                ├─ output::Pdf
                                                └─ your output::Adapter
```

A scene contains dimensions, DPI, and ordered filled paths in printer dots.
Paths contain moves, lines, cubic Béziers, and closes, with even-odd filling.
Each drawing paints black, white, or inverts the pixels beneath it. Text glyphs,
barcode bars, and downloaded bitmaps become paths; adapters never interpret ZPL.
SVG uses paths and isolated difference blending for inversion. PNG uses a
pixel-center scan converter writing directly to packed one-bit grayscale PNG
with stored DEFLATE blocks. Its decoded pixels match the public eight-bit raster
output; PNG byte streams differ from earlier eight-bit encodings.
PNG files prioritize simplicity over compression. Curves are flattened for PNG;
SVG and PDF viewers may rasterize fractional coordinates and antialias edges
differently; vector geometry does not guarantee pixel parity with PNG. PDF 1.7 retains vector
paths, even-odd fills, and ordered black/white/invert compositing using Difference
blending against a white page backdrop. Text is exported as glyph paths, so no
fonts are required and text is not searchable. The writer adds no runtime dependencies.
PDF pages are sized in points (72 per inch) using scene DPI; there are no added
margins. The viewer is asked to disable print scaling, but printing settings
can still override this preference. Streams are uncompressed and output is deterministic.

```rust
use zpl::{
    output::{Adapter, Pdf, Png, Svg},
    render, Options,
};
let document = render(b"^XA^FO20,20^FDHELLO^FS^XZ", Options::default())?;
for scene in &document.labels {
    let png = Png.encode(scene)?;
    let svg = Svg.encode(scene)?;
    let pdf = Pdf.encode(scene)?;
    // Save or serve these byte buffers using your application's transport.
}
let multipage_pdf = Pdf.encode_pages(&document.labels)?;
```

### Caller-supplied fonts

Use `render::fonts::Fonts` with `render::render_with_fonts`, or
`render_with_fonts_and_limits` when setting renderer budgets. Existing `render`
and `render_with_limits` calls keep their embedded resident fonts.

```rust
use zpl::render::{fonts::Fonts, profiles::SPECIFICATION, render_with_fonts};
use zpl::truetype::Hinting;

let font_bytes = std::fs::read("my-font.ttf")?;
let mut fonts = Fonts::new();
fonts.insert_truetype('Z', &font_bytes, Hinting::Native)?;
let document = render_with_fonts(
    b"^XA^FO20,20^AZN,32,24^FDHello^FS^XZ",
    SPECIFICATION,
    &fonts,
)?;
```

Assignments accept `0`–`9` and `A`–`Z`, selected with `^A` or `^CF`; `@`
selects the separate `^GS` symbol face. Registering `0` replaces the default face.
Unregistered faces continue to use resident behavior. Font sets belong to the
caller, with no global installation or filesystem/network access by rendering.
The same set can be reused across render calls. Invalid assignments leave the
previous face intact. A registered face's missing glyphs return an error instead
of mixing in resident glyphs. This API does not implement ZPL font downloads.

Register a virtual printer filename with `insert_named_truetype`,
`insert_named_bitmap`, or `insert_named_zbf`. Both `^A@` and `^CW` resolve only
these caller-registered faces:

```rust
fonts.insert_named_truetype("R:BRAND.TTF", &font_bytes, Hinting::Native)?;
let direct = render_with_fonts(
    b"^XA^FO20,20^A@N,32,24,R:BRAND.TTF^FDHello^FS^XZ",
    SPECIFICATION,
    &fonts,
)?;
let aliased = render_with_fonts(
    b"^CWZ,R:BRAND.TTF^XA^FO20,20^AZN,32,24^FDHello^FS^XZ",
    SPECIFICATION,
    &fonts,
)?;
```

Names are case-insensitive; omitting the device selects `R:`. Supported devices
are `R:`, `E:`, `B:`, and `A:`. Require an explicit `.FNT`, `.TTF`, or `.TTE`
extension and a basename of 1–255 ASCII letters, digits, underscores or hyphens.
These are registry keys, not host filesystem paths; the extension does not
convert the supplied font data or enable additional outline formats.

`^CW` requires a single `0`–`9` or `A`–`Z` ID and a registered filename. It
replaces that ID's mapping for the remainder of the render call, including
subsequent labels, and works with both `^A` and `^CF`. It does not mutate the
caller's font collection. Each render starts with the caller's original mapping.
`^A@` with an omitted filename reuses the last named selection in that call;
before the first named selection it uses the current default font. Other font
selections and `^CW` assignments do not clear the remembered `^A@` name.

Explicit unknown filenames return `RenderError` at the referencing command,
rather than silently using a different face. This is deliberately stricter than
firmware's missing-name fallback. Named selections retain the custom sizing
rules below; this does not claim native downloaded-bitmap magnification parity.
See the Zebra Programming Guide [^A@](https://docs.zebra.com/us/en/printers/software/zpl-pg/zpl-commands/%5Ea-.html)
and ^CW (p. 168) for the command conventions.

For bitmap fonts, use `insert_bitmap(id, settings, glyphs, baseline)` or
`insert_zbf(id, bytes, baseline)`. The baseline is measured from the strike's
cell top in native dots; glyph `top` values are relative to it. The caller's
advances and ink metrics drive wrapping and placement. Requested width/height
scale the native strike independently, without resident-matrix quantization;
an omitted axis preserves its aspect ratio. DPI metadata is validated but does
not rescale dot-based requests. The registration ID may differ from the strike's
resident tag. A new registration replaces the prior face rather than adding a
size-specific strike.

TrueType uses the existing original engine and scan converter, adding no runtime
dependencies. It supports TrueType/OpenType **quadratic `glyf` outlines**, not
CFF/CFF2 outlines or font collections. Dimensions are dots per em, independently
rounded to whole dots in `1..=4096`; the baseline uses the `hhea` ascender.
`Hinting::None` skips hint execution; `Hinting::Native` reports unsupported
instructions instead of silently ignoring them. Standard font-engine and scan
semantics apply, even with a printer compatibility profile. Existing Unicode
processing still applies, but this adds no OpenType shaping or kerning.

Custom text, field blocks, bounded text, and ordinary barcode captions resolve
into the same scene paths consumed by PNG/SVG/PDF. Barcode-specific built-in
interpretation symbols and retail digit artwork remain barcode geometry.
Use `SPECIFICATION` for custom-font layout without printer compatibility
adjustments; font customization does not assert physical-printer parity.

The library returns all labels and preview warnings. Options default to
832 × 1218 dots at 203 DPI; `^PW` and `^LL` override dimensions. The `zpl-cmd render` command takes
one label for PNG/SVG, or one or more labels for PDF, and selects the adapter
by output extension:

```sh
direnv exec . cargo run -p zpl-cmd -- render docs/examples/local-label.zpl /tmp/label.svg
direnv exec . cargo run -p zpl-cmd -- render docs/examples/local-label.zpl /tmp/label.png
direnv exec . cargo run -p zpl-cmd -- render docs/examples/local-label.zpl /tmp/label.pdf
```

Install from this checkout with `cargo install --locked --path zpl-cmd`, or use
the Cargo commands above. `zpl-cmd render --help` lists the supported profiles
and QR mask option. This command replaces the former rendering example.

## Printer profiles

`ZQ610_PLUS_203_DPI` targets the captured ZQ610 Plus V100.21.21Z HTTP preview:
384 × 2030 initial dots, a 384-dot width cap, 64-dot width rounding with a
centered origin, width fixed at the first draw, and ignored `^LL`. These four
preview compatibility settings are independently selectable and disabled in
the existing ZD621 and specification profiles. The CLI accepts
`--profile zq610-plus`; `zd621-preview` enables only the two measured width
rounding/latching behaviors on the ZD621 base. See
[capture evidence and limits](printer-recapture.md).

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
| `qr_printer_segmentation` | Minimum-bit automatic QR encoding | Forward run merging measured on ZD621; can change symbol size and modules |
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
| Text | `CF`, `A0`, `AA`, `AB`, `AC`, `AD`, `AE`, `AF`, `AG`, `AH`, `FD`, `FV`, `FH`, initial-label `SN`; printable ASCII in fonts 0/A/B/C/D/E/F/G/H and captured font-0 é; Hebrew letters and final forms at 32×32 and 40×24 dots; `PA` with all advanced features disabled; `CI13` ASCII, `CI27` Latin-1 subset and `CI28` UTF-8 |
| Blocks | `FB`: left/center/right/justified alignment, hanging indent, wrapping, explicit `\&` breaks and last-row overflow; automatic and explicit-marker hyphenation, negative line spacing, and zero-width/overflowing-indent printer compatibility |
| Shapes | `GB` including rounded corners, `GC`, `GE`, `GD`; black or white outlines/fills |
| Graphics | `~DG`, `XG`, `GFA`, `GFB`: raw hex, Zebra ASCII run lengths/row shortcuts, B64, Z64; CRC16 and zlib checksums checked |
| Barcodes | Original per-code linear, matrix, stacked, and postal encoders; `BY`; see [coverage, limitations, specifications, and decoder tests](barcodes.md) |

Text uses [embedded resident font 0](../zpl/assets/README.md), captured from the
ZD621 preview at 32 dots and 203 DPI. All 95 printable ASCII glyphs, including
lowercase, retain their measured advances, bearings and baseline offsets. The
renderer defaults to font 0 at 20 dots. `^CF0,32` or `^A0N,32,0` selects the
captured size; omitted/zero width is proportional to height. Resident A/B/C/D/E/F/G/H and GS read their native bitmap matrices directly from
the shared `zpl-bitmap-fonts` crate with integer magnification; C shares D's matrix, B renders lowercase input as uppercase, and H renders lowercase as advancing blanks. A single bitmap dimension determines the other proportionally; omitted A
dimensions inherit the CF request. The printer profile's
`bitmap_cf_font_only_resets_size` option resets a font-only bitmap CF command
to native size; SPECIFICATION retains the previous size. Other resident font
IDs and uncaptured glyphs return errors.

The 4,365-byte strike is compiled into the binary. Its pixels become filled
rectangles, shared by PNG, SVG and PDF. Identical adjacent bitmap runs with exactly
shared edges are joined vertically; overlapping glyph strokes are merged.
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

Examples of explicit errors include Code 128 extended-byte FNC4, downloaded fonts, persistent storage across render calls, print-quantity serial iteration,
compressed binary `GFC`, and printer configuration commands.
The parser still frames these commands; rendering coverage is separate from
command-stream parsing coverage. Configuration persists only within one `render`
call. Fields require `FS`; the ZD621 profile also ends inline `GF` fields before
`FO`, `FT`, or `BY` through `inline_graphic_implicit_separator`.

The [public-document regressions](../zpl/tests/fixtures/public-zpl-zd621-v1/README.md)
cover all 22 labels from the October 2026 comparison. Five independent tolerances
are enabled in `ZD621_203_DPI` (and therefore `Options::default()`), while
`SPECIFICATION` and the mobile-printer profile retain strict behavior:

| Compatibility field | ZD621 behavior |
| --- | --- |
| `inline_graphic_implicit_separator` | Finish inline `GF` before `FO`/`FT`/`BY` without `FS` |
| `qr_malformed_header_uses_defaults` | An invalid correction selector consumes three switch bytes, then uses automatic input and level M |
| `code39_normalize_input` | Uppercase ASCII letters and discard bytes outside the Code 39 alphabet before checksums and captions |
| `barcode_module_width_through_12` | Accept `BY` module widths through 12 dots |
| `box_zero_thickness_as_one` | Treat `GB` thickness zero as one dot |

These settings reproduce acceptance, including data loss: the malformed QR
`Package…` encodes `kage…`, and Code 39 `{0}` encodes `0`. Valid QR headers,
manual segments and structured append remain strictly parsed. Successful rendering
does not establish intended barcode payload correctness or full-image printer parity.
All 22 native canvases have pinned regression counts and hashes; the affected
graphics/barcode regions in the five formerly rejected labels are pixel-exact.

## Limits and validation

`zpl-cmd render` is unlimited by default for resource-policy budgets, including
input, decoded fields/graphics, labels, geometry, raster work, and PDF pages.
Use its optional `--max-*` flags to set budgets; `render --help` lists them.
The numeric operand ceiling is configurable too.

Library convenience entry points retain bounded defaults. Use
`render::render_with_limits(input, options, render::Limits { ... })` to raise or
lower any interpretation budget; values are no longer clamped to the defaults.
`render::Limits::unlimited()` disables all of these policy ceilings:

| Interpretation budget | Library default |
| --- | --- |
| Input and expanded stored-format bytes, each | 1 MiB |
| Labels | 64 |
| Scene segments across all labels | 1,000,000 |
| Retained downloaded-graphic segments | 1,000,000 |
| Decoded bytes per field / concatenated field | 4,096 |
| Decoded bytes per graphic | 25,000 |
| Pixels per label | 32 Mi pixels |
| Width / height | `u32::MAX` |
| Absolute numeric operand | 1,000,000 |
| Stored formats / recall depth / total recalls | 256 / 8 / 4,096 |
| Absolute scene coordinate | 1,000,000,000 dots |

Output budgets are separate from ZPL interpretation. Pass `output::Limits` to
`Scene::new_with_limits`, `Scene::validate_with_limits`,
`Svg.encode_with_limits`, `Png.encode_with_limits`,
`Pdf.encode_pages_with_limits`, `raster::rasterize_with_limits`, or
`raster::rasterize_into_with_limits`. Defaults are 32 Mi pixels per image,
1,000,000 scene segments (across all pages for PDF), 1,000,000 flattened edges
per draw, 100,000,000 edge/scanline visits per raster, 64 PDF pages, and the same
coordinate ceiling. `output::Limits::unlimited()` disables these ceilings.
A raised render budget needs a corresponding output budget when applicable:

```rust
use zpl::{render::{render_with_limits, Limits, profiles::SPECIFICATION}, output};
let document = render_with_limits(input, SPECIFICATION, Limits::unlimited())?;
let pdf = output::Pdf.encode_pages_with_limits(
    &document.labels, output::Limits::unlimited(),
)?;
```

Unlimited mode still validates ZPL semantics, finite coordinates, representable
dimensions, and output-format constraints. Stored-format cycles are errors;
acyclic expansion uses a heap stack. PNG dimensions and chunk lengths follow
the format's 31-bit bounds. PDF requires at least one page and uses `UserUnit`
for large physical pages, up to the format's 75,000 limit; classic cross-reference
offsets must fit ten decimal digits. Available memory still bounds rendering.
Limit errors are not a claim that arbitrary hostile inputs take constant time.
The browser preview and Cloudflare service retain their bounded policies.

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
[Field-block limit controls](../zpl/tests/fixtures/field-block-limits-zd621-v1/README.md)
cover zero widths, negative line spacing, overflowing indentation, centered
edge placement and long off-label continuations using independent printer options.

[Resident G controls](../zpl/tests/fixtures/resident-g-zd621-v1/README.md)
add 72 exact frames for the 95-character native strike, scaled text, origins,
justification, CF/A defaults, wrapping and wide Code 128 captions. G uses the
existing bitmap compatibility options in the printer profile.

[Resident H controls](../zpl/tests/fixtures/resident-h-zd621-v1/README.md)
add 41 exact frames, including native OCR-A extraction, scaling, placement,
captions and lowercase blanks with measured advances. The complete resident
bitmap set A through H is now available for the captured ASCII inputs.

[QR Kanji controls](../zpl/tests/fixtures/qr-kanji-zd621-v1/README.md) cover
manual Shift JIS input for both QR models and all error-correction levels.
Encoding matches the printer with its captured mask; automatic mask selection
remains a separate, pinned accuracy gap. Unicode-to-Shift-JIS conversion is not
part of this manual byte-input support.

[QR structured-append controls](../zpl/tests/fixtures/qr-append-zd621-v1/README.md)
verify the `D` sequence/parity envelope and mixed manual segments, including
counted byte payloads containing commas. Printer mask selection remains an explicit known gap.
[Extended Model 1 controls](../zpl/tests/fixtures/qr-model1-extended-zd621-v1/README.md)
verify the ZD621 versions 15–40 behind `qr_model1_extended_versions`; the
specification profile retains the standard version-14 limit.

[Page-mirror controls](../zpl/tests/fixtures/page-mirror-zd621-v1/README.md) verify
13 exact previews. `preview_ignores_print_mirror` independently selects the
printer preview's omission of `^PM`; SPECIFICATION mirrors the whole label,
retains the setting across labels, and combines it with `^PO` inversion.

[`^FP` direction controls](../zpl/tests/fixtures/field-direction-zd621-v1/README.md)
cover horizontal gaps, vertical columns and reverse character order independently
of glyph rotation. The printer profile selects vertical-gap and anchor departures
through separate options. FP with FB remains an explicit unsupported combination.

## Request-local stored formats

`^DF` stores a template without producing a label, and `^XF` recalls it within
the same render request. Existing `^FN` planning binds its variable fields.
R/E/B/A names refer only to in-memory namespaces; no printer or filesystem
storage is accessed. Omitted recall devices search R, E, B, A. Definitions can
be replaced. Recall depth, total recalls, stored-object count, and expanded bytes
are configurable through `render::Limits`; defaults are 8 levels, 4096 recalls,
256 objects, and 1 MiB of expanded input. Cycles are always rejected. Error offsets point back to
the original source. Syntax changes inside stored templates and recalls under
a different syntax are explicit errors. Proxy admission remains unchanged.

Legacy CI0/CI13 text now maps extended bytes through CP850, after character
remapping. Fonts still reject glyphs outside the captured repertoire. Native
retail glyph supplements cover the original legacy byte interpretations and
the explicit UTF-8 middle dot; see the retail-font-zd621-v1 evidence.
