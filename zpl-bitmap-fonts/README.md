# zpl-bitmap-fonts

Allocation-free, `no_std` bitmap glyphs and encoding maps, with no dependencies,
runtime JSON parsing or unsafe code. Enable `zd621` for the single bundled dataset:

```rust
use zpl_bitmap_fonts::zd621;
let font = zd621::font_by_name("Z:A.FNT").unwrap();
let glyph = font.glyph(65).unwrap(); // measured CI0 source position
let advance = glyph.advance;
let ink = glyph.pixel(0, 0);
```

The renderer uses this same dataset for resident A–H and GS (`@`). `resident(id)`
resolves the appropriate face; C/D share a matrix, E/H select their 203-dpi forms.
There is no second copy of the bitmap fonts or separate encoding-data feature.

The [2026-10-06 snapshot](data/zd621/README.md) contains 47 fonts and 10,111 glyph
records, including calibrated native metrics and advancing blanks from the earlier
verified captures. It combines measured encoding observations and explicitly
unverified candidates. The latter never supply renderer glyphs. These observations
apply to the ZD621 at 203 dpi/V93.21.33Z, not other devices or configurations.

Glyph IDs and input keys are separate. Use `font.encoding(Encoding::Input { ci })`
to inspect a mapping's status and candidate record IDs, then `font.record(id)` for
raw glyph metrics and padded pixels. An absent map input is untested; an unresolved
input is not a verified blank. Font-level `metrics` is optional; `cell_metrics()`
returns measured dimensions, baseline and space advance when present.

`font.glyph(source_key)` is the renderer's convenience view over measured CI0
source mappings. Matched candidate IDs are observed pixel/advance equivalents;
choosing their rendering does not identify one physical record. The view crops
ink bounds while borrowing the same bitmap bytes. Bearings are signed native dots
relative to `^FT`. Its `bitmap()` is row-padded storage: use `row_offset(y)` for
bit scanning or `pixel(x, y)`. This replaces the old continuously packed-bit API.

Generate custom modules with the same compiler:

```sh
cargo run --locked -p zpl-font-extract -- compile \
  zpl-bitmap-fonts/data/zd621/fonts.json --out _zd621-generated
```

Keep `fonts.rs` and `bitmaps.bin` adjacent. `catalog.json` records source/output
hashes and compact array sizes. The JSON retains evidence digests; raw captures
are not included. Exhaustive tests check the JSON against the reader, exact
regeneration, and the unchanged golden digest for the previous source-key behavior.
See [automatic recovery](../docs/automatic-bitmap-fonts.md) for probing and schema
semantics. Font content retains its original licensing; code is OSL-3.0.
