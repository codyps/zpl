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
of mixing in resident glyphs. ZPL downloads use the same engines (see below).

Register a virtual printer filename with `insert_named_truetype`,
`insert_named_bitmap`, or `insert_named_bitmap_font`. Both `^A@` and `^CW`
select these resources, as well as bundled ROM fonts and resources supplied by
a path resolver:

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
are `R:`, `E:`, `B:`, `A:`, and `Z:`. Require an explicit `.FNT`, `.TTF`, `.TTE`, `.OTF`, or `.DAT`
extension and a basename of 1–255 ASCII letters, digits, underscores or hyphens.
These are registry keys, not host filesystem paths; the extension does not
convert the supplied font data or enable additional outline formats.

`^CW` requires a single `0`–`9` or `A`–`Z` ID and a resolvable filename. It
replaces that ID's mapping for the remainder of the render call, including
subsequent labels, and works with both `^A` and `^CF`. It does not mutate the
caller's font collection. Each render starts with the caller's original mapping.
Aliases retain normalized filenames and resolve them against the font resources
when used; they do not capture a particular face. The remembered `^A@` filename
is stored separately from the ID aliases.
`^A@` with an omitted filename reuses the last named selection in that call;
before the first named selection it uses the current default font. Other font
selections and `^CW` assignments do not clear the remembered `^A@` name.

Explicit unknown filenames return `RenderError` at the referencing command,
rather than silently using a different face. This is deliberately stricter than
firmware's missing-name fallback. API-registered bitmap fonts represent already
installed downloads. Registered strikes, providers, resolver-returned bitmap faces,
and in-job `~DB` downloads share integer cell magnification, one-based baselines
and bitmap rotation pivots under the printer profiles through
`supplied_bitmap_font_metrics`; `bitmap_font_ft_dot_origin` also applies to their
FT anchors. See the [downloaded bitmap controls](../zpl/tests/fixtures/downloaded-bitmap-zd621-v1/README.md)
for the measured fonts, sizes and device scope.
See the Zebra Programming Guide [^A@](https://docs.zebra.com/us/en/printers/software/zpl-pg/zpl-commands/%5Ea-.html)
and ^CW (p. 168) for the command conventions.

### ROM fonts and application path resolvers

The default resolver exposes all 47 recovered `Z:` names in the bundled ZD621
203-dpi/V93.21.33Z collection through `^A@` and `^CW`, including OCR-A
`Z:H6.FNT`/`H8.FNT`/`H12.FNT`/`H24.FNT` and OCR-B
`Z:E6.FNT`/`E8.FNT`/`E12.FNT`/`E24.FNT`. `Z:` is the resident ROM namespace;
lookup does not read a printer or a host directory. The bundle is not a complete
ROM image: unknown/unrecovered filenames return an error.

```zpl
^CWX,Z:H12.FNT
^XA^CI28^FO20,20^AXN,30,19^FDABC123^FS
^FO20,80^A@N,41,20,Z:E12.FNT^FDABC123^FS^XZ
```

ROM faces retain their measured source/input/Unicode maps, bearings and advances.
Calibrated bitmap sizes round to integer native multiples. `Z:EPL6.FNT` and
`Z:EPL7.FNT` have measured glyphs but uncalibrated cell metrics: only explicit
`1,1` (native capture size) is supported. Untested or unresolved input mappings
remain errors except for the existing independently measured blank-advance
fallback. The bounded encoding survey is described in
[automatic recovery](automatic-bitmap-fonts.md); named selection does not expand it.
Local integration tests verify native pixels for every bundled name. They are
not new physical-printer parity evidence for named-font layout.

Library users can replace lookup with `Fonts::set_resolver`. Callbacks receive
validated uppercase printer paths, with `R:` added when the device is omitted.
They return `Result<Option<Face>, String>`: `None` means unknown, and errors
propagate at the selecting command. Use `Face::bitmap`, `Face::provider` or
`Face::truetype` for application resources and the public `resolve_rom_font`
function to delegate to the built-in collection:

```rust
use zpl::fonts::{Face, Fonts, resolve_rom_font};
use zpl::truetype::Hinting;

fn application_fonts(bytes: &[u8]) -> Fonts<'_> {
    let mut fonts = Fonts::new();
    fonts.set_resolver(move |path| {
        if path == "R:BRAND.TTF" {
            Face::truetype(bytes, Hinting::Native).map(Some)
        } else {
            resolve_rom_font(path)
        }
    });
    fonts
}
```

Precedence is in-job downloads, explicit named registrations, then the resolver.
A custom resolver replaces default lookup; it controls whether to delegate.
Successful results are cached per name per render call, shared by `^A@`/`^CW`,
and discarded after that call. Later downloads supersede cached faces. Borrowed
TrueType bytes must outlive the font collection; callbacks must be `Send + Sync`.
The default resolver performs no I/O; any application callback I/O is caller-owned.
Font downloads to `Z:` are rejected as read-only.

### Font downloads in ZPL

Downloads register virtual resources for the current render call. They work
before `^XA`, within labels, and in recalled stored formats. `^CW`/`^A@` resolve
these resources using the same namespace as caller-provided fonts. Downloading
an existing filename replaces it for subsequent fields, including existing ID
aliases and remembered `^A@` selections. Earlier fields retain their rendered
appearance. Caller registrations and later render calls are unaffected.

| Command | Supported payload | Default extension |
| --- | --- | --- |
| `~DB` | Structured bitmap glyphs; each glyph accepts hex, B64, or Z64 | `.FNT` |
| `~DT` | Quadratic TrueType SFNT in hex, B64, or Z64 | `.DAT` |
| `~DU` | Quadratic TrueType SFNT in hex, B64, or Z64 | `.FNT` |
| `~DY` | Font types `T` (TTF/OTF) and `E` (TTE); mode `A` for hex/B64/Z64, `B` for binary | `.TTF` / `.TTE` |

For example, this downloads a two-row bitmap character and selects it by name:

```zpl
~DBR:TEST.FNT,N,10,10,7,4,1,Example,#0041.2.8.1.7.9.FF81
^XA^FO20,20^A@N,20,20,R:TEST.FNT^FDA A^FS^XZ
```

`~DB` preserves glyph offsets, advances and the cell baseline. Its `space`
operand supplies blank advances for absent characters; caller-supplied bitmap
fonts still report missing glyphs. Cell dimensions are bounded to 32000 dots;
glyph dimensions, advances and offsets use the bitmap engine's 4096-dot bounds.
Glyph codes must be supported printable Unicode scalars. Downloads and registered
bitmap resources use the same profile-dependent sizing and placement rules above.

TrueType downloads use native hinting. All downloaded payloads must decode to
formats supported by the existing engine: the legacy **ZTools-specific
containers** mentioned for `~DT`/`~DU` are not decoded. An unrecognized container
produces an explicit error; accepting the command does not imply support for
every historical printer font format. CFF/CFF2, collections, `~DS`, `~DL`, and
`~DY` AR compression/non-font objects remain unsupported.

Declared lengths, CRCs, glyph counts and font metrics are validated. The
`Limits::font_bytes` budget defaults to 16 MiB across downloads, including
replacements; bitmap row/glyph allocation overhead also counts against it.
Code that constructs `Limits` with every field explicitly must add `font_bytes`
or use `..Limits::default()`. The existing input/expanded-format budgets still apply, and an individual
TrueType SFNT remains bounded to 16 MiB. Unknown names fail at selection, so
fonts must be downloaded before their `^CW`/`^A@` reference. Font data is never
written to a host file or sent to a printer. Proxy admission remains unchanged.

References: Zebra Programming Guide ~DB pp. 169–170, ~DT/~DU pp. 179–180,
~DY pp. 181–183 and the
[B64/Z64 appendix](https://docs.zebra.com/content/tcm/us/en/printers/software/zpl-pg/zb64-encoding-and-compression/b64-and-z64-encoding.html).

### Font metrics and engine limits

For bitmap fonts loaded from arbitrary formats, implement `fonts::BitmapFont`
and register an `Arc` with `insert_bitmap_font(id, provider)` or
`insert_named_bitmap_font(name, provider)` for `^CW`/`^A@`. The trait exposes
`BitmapMetrics` (native cell width, height, and baseline) and Unicode glyph lookup
returning `Result<Option<Cow<Glyph>>, String>`. Providers can borrow their storage
or decode glyphs on demand, without supplying a resident-font tag, DPI, or ZBF
bytes. Font sets share providers when cloned; borrowed storage must outlive the
set. Lookups should be deterministic. Missing glyphs and provider errors are
rendering errors, with no resident fallback. Cell dimensions are 1–4096 dots;
metrics are checked at registration and glyphs before use. `fonts::Glyph` stores
one MSB-first packed row per bitmap row, with one bits marking foreground ink.

For an already decoded strike, use
`insert_bitmap(id, settings, glyphs, baseline)`. Supply the same native-dot
baseline and baseline-relative glyph `top` values as for a `~DB` resource.
The caller's advances and ink metrics drive wrapping and placement. Printer
profiles interpret the baseline as one-based and round requested width/height
to independent integer cell multiples. SPECIFICATION uses the baseline directly
and scales continuously. An omitted axis preserves the aspect ratio before
quantization. DPI metadata is validated but does
not rescale dot-based requests. The registration ID may differ from the strike's
resident tag. A new registration replaces the prior face rather than adding a
size-specific strike.

ZBF1/ZBF2 loading has been removed: `bitmap_font::unpack`, `insert_zbf`, and
`insert_named_zbf` are no longer available. Migrate callers to a `BitmapFont`
provider or to decoded glyph registration (`insert_bitmap`/`insert_named_bitmap`).
Bundled fonts use compiled `zpl-bitmap-fonts` tables and need no binary decoder.
ZBF3 is a separate portable format from the `zebra-firmware` recovery tools;
this renderer does not include a ZBF3 reader. An external reader can be adapted
to `BitmapFont`, with explicit character mapping for printer source-position keys.

TrueType uses the existing original engine and scan converter, adding no runtime
dependencies. It supports TrueType/OpenType **quadratic `glyf` outlines**, not
CFF/CFF2 outlines or font collections. Dimensions are dots per em, independently
rounded to whole dots in `1..=4096`; the ZPL cell baseline is three quarters of
the rounded height, independent of the font's `hhea` ascender. The
`font0_fo_floor_baseline` compatibility option also applies to supplied TrueType
faces: it floors the `^FO` baseline to match measured ZD621 positioning. Bitmap
faces retain their supplied baseline metrics; `^FT` remains baseline-anchored.
`Hinting::None` skips hint execution; `Hinting::Native` reports unsupported
instructions instead of silently ignoring them. `SPECIFICATION` uses standard
font-engine and scan semantics. The ZD621 profile enables
`supplied_truetype_printer_metrics`: a 10-dot minimum on both dimensions,
measured outline and advance scaling, and scan conversion in the field's device
orientation before scene placement. This applies to registrations, named fonts,
resolver callbacks, downloads, and ordinary captions. Disable that option to
retain the standard engine under a printer layout profile. ZQ610 does not enable
this ZD621-only calibration. Existing Unicode processing still applies, but this
adds no OpenType shaping or kerning. The 35 unchanged controlled-Heros captures in
`tests/fixtures/external-fonts-zd621-v1` now match pixel for pixel at their native
origins, including rotations, aspect ratios and field blocks. The measured
scanner rounds quadratic midpoints and subdivisions in device coordinates and
applies printer-specific directed ties and dropout handling. Independent contour
witnesses and scaling probes are pinned in `tests/fixtures/truetype-raster-zd621-v1`:
all 36 native canvases match exactly. Scaling preserves the measured rational
path when point-size/DPI factors cancel by binary shifts, including its signed
half-tie rules; otherwise it rounds the ppem and coordinate multiplier to 16.16.
These captures do not establish parity for every TrueType program or device.

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

Text uses compact [captured resident font 0](../zpl/assets/README.md), captured from the
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


Font resources and policy live in `zpl/src/fonts`, separate from text layout and
scene construction. All bundled bitmap captures, including scalable 0/P–V
strikes, share `zpl-bitmap-fonts` records and pixel views. Encoding/source lookup,
strike selection, fallback and measured font metrics do not depend on output
adapters. See [normalized captures](automatic-bitmap-fonts.md#normalized-captured-strikes).
