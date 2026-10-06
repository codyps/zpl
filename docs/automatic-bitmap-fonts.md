# Automatic bitmap font recovery

`zpl-font-extract` can discover bitmap font metrics, request printer previews,
cache the original PNGs, extract and independently verify `fonts.json`, and
compile it into compact Rust tables. It requires no Python, font files, firmware
image, resident metric table, or renderer-generated reference images.

```sh
cargo run --locked -p zpl-font-extract -- recover \
  --host http://printer.local/ --cache _font-cache --out _recovered-fonts
```

This samples printable ASCII from A–H and the separate `^GS` face (`@`). Use
`--font A` to start with one face. Credentials, when needed, come from
`ZPL_USERNAME` and `ZPL_PASSWORD`. Requests use the printer's HTTP **Preview Label**
form and its shared `R:TEST1` object; no job is submitted to raw TCP printing.
Avoid concurrent preview clients. The CLI locks its local cache and printer
origin, rejects redirects and cross-origin image URLs, bounds responses/timeouts,
and spaces requests five seconds apart by default. It has no automatic retries.

Inspect initial requests before capture:

```sh
cargo run --locked -p zpl-font-extract -- plan --font A --out _initial-plan
```

Refinement requests depend on measurements, so this initial plan is only the
first stage. Every subsequent page's plan, ZPL, PNG and hash receipt are retained
in the cache. `recover` automatically reuses complete, matching receipts and
resumes after interruption. Corrupt entries fail verification. Use a new cache
for a fresh printer measurement, especially after firmware or printer-state
changes. An existing output directory is never overwritten.

Replay a complete run with the printer disconnected:

```sh
cargo run --locked -p zpl-font-extract -- recover \
  --host http://printer.local/ --cache _font-cache \
  --out _replayed-fonts --offline
```

Use the same host and probe options as the original run. Offline mode fails if
any adaptive stage or independent repeat is absent; it never contacts the printer.
A successful run writes `fonts.json` and `rust/{fonts.rs,bitmaps.bin,catalog.json}`.

## Measurement and verification

The method follows `zebra-firmware/tools/bitmap_preview.py`:

1. Probe two visible glyphs with both `^FO` and `^FT` to measure a one-based
   baseline. Measure repeated glyphs and a space-separated pair for advances.
2. Request different heights and widths independently. Require exact integer
   replication of native pixels. Fit the documented nearest-integer 1–10 bitmap
   magnification model, then choose further requests that distinguish remaining
   native dimension candidates. Ink bounds are not used as nominal cell dimensions.
3. Capture each isolated glyph and a sentinel–glyph–sentinel composition. Solve
   its advance against all ink coordinates, including overlap and bearings.
4. Recheck reference glyphs, space, baseline and doubled dimensions. Serialize
   and reparse the font JSON. Capture fresh, differently ordered text that includes
   every surviving glyph and require exact full-canvas recomposition from that JSON.

All stages check canvas dimensions, clipping, page-specific registration marks,
stray ink, and a separately requested repeat of the first page. These checks reject
stale, blank, clipped, changed and non-bitmap output. A verification failure never
produces a successful font JSON. Cache evidence remains available for diagnosis.

`--bound` bounds native dimension discovery (default 128). `--width` and `--height`
set the printer preview canvas (832×4096 by default); oversize probes fail with a
request to adjust these limits. Unsupported scalable fonts fail the bitmap model.
Font selection is checked against two different default faces for named fonts.

## Encodings and source positions

Use `--codes 32,65,66,200` to select decimal keys. Space and the two control keys
are always included. `--encoding` supports CI0, CI13, CI27 and CI28. CI28 encodes
input characters as UTF-8; ordinary input mode permits printable ASCII and the
Latin-1 range 160–255 for CI27/CI28.

Named fonts and CI0 source positions can be sampled explicitly:

```sh
cargo run --locked -p zpl-font-extract -- recover \
  --host http://printer.local/ --cache _source-cache --out _source-fonts \
  --font Z:A.FNT --source --encoding 0
```

Source mode defaults to keys 0–255, omits TAB (9), and uses explicit CI0 remapping
for every field. Non-composable or ignored source positions are excluded with
reasons in JSON provenance. A separate fresh verification capture covers all
remaining glyphs. A named face whose default A/B probes are blank needs two known
visible keys, such as `--probes 48,49`; this CLI does not automatically survey
unknown font files or stored graphics. Multiple fonts share those probe choices;
run separate jobs if faces need different controls.

JSON schema `zebra-bitmap-fonts`, version 1, is compatible with the Python
intermediary. The content hash covers canonical sorted-key JSON for schema,
version, mapping and fonts. It detects edits after verification; it is not a
signature authenticating the printer. Keys describe the declared input/source
mapping, not an inferred Unicode cmap. Invisible padding, original file layout,
unused slots and non-observable metadata cannot be recovered.

## Compact fonts in a separate crate

`zpl-bitmap-fonts` contains the allocation-free `no_std` reader. It has no runtime
dependencies or unsafe code. The optional `zd621` feature exposes the bundled,
previously verified ZD621 CI0 source fonts as `zpl_bitmap_fonts::zd621::FONTS`.
See that crate's README for the dataset provenance.

Compile verified JSON independently, including JSON from the Python workflow:

```sh
cargo run --locked -p zpl-font-extract -- compile _recovered-fonts/fonts.json \
  --out _compact-fonts
# Select a smaller collection with repeated --font selectors.
```

Generated modules depend only on `zpl-bitmap-fonts`; keep `fonts.rs` and
`bitmaps.bin` together and import them with a `#[path = "..."] mod fonts;`.
Use `fonts::font_by_name("A")`, then `font.glyph(65)` and `glyph.pixel(x, y)`.
An absent key returns `None`; an observed advancing blank returns a real glyph.
Bearings are signed native dots relative to the FT origin.

The compiler deduplicates glyph records across faces and deduplicates bitmap
payloads separately. Lookups use u16 record IDs, metrics occupy five bytes per
record, offsets are u32, and bits run continuously across rows, MSB first. Signed
bearings must fit i8; advance/ink dimensions must fit u8. Oversize values fail
explicitly instead of truncating. Font-level metrics retain u16 precision. The
catalog reports array payload sizes, excluding names, references and alignment.

This adds a font dataset crate without changing the renderer's existing font
selection or compatibility profiles.

## Tests

```sh
cargo test --locked -p zpl-font-extract -p zpl-bitmap-fonts --all-features
cargo clippy --locked -p zpl-font-extract -p zpl-bitmap-fonts --all-targets --all-features
cargo fmt --all -- --check
```

Tests use an independent ZPL-interpreting simulated printer, including real
loopback HTTP preview transactions, cache replay, unfamiliar dimensions, variable
and overlapping advances, remapping, corrupt evidence and full-page holdouts.
These validate the implementation; they do not constitute a new live printer run.
