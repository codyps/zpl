# zpl-bitmap-fonts

Bitmap glyphs and encoding maps measured from Zebra printer previews. The crate
is allocation-free and `no_std`, with no dependencies, runtime JSON parsing,
build script, or unsafe code.

For rendering complete ZPL labels, use the [`zpl` crate](https://crates.io/crates/zpl).
Use this crate directly when you need the bundled glyph pixels, metrics, or
encoding observations.

## Installation

Add the crate to your `Cargo.toml`, enabling the bundled ZD621 dataset:

```toml
[dependencies]
zpl-bitmap-fonts = { version = "0.1", features = ["zd621"] }
```

Import it as `zpl_bitmap_fonts` in Rust. No features are enabled by default;
without `zd621`, only the shared bitmap and collection types are available.

## Quick start

Look up a resident font and a measured CI0 source position:

```rust
use zpl_bitmap_fonts::{resident, zd621};

let font = zd621::font_by_name("Z:A.FNT").unwrap();
let glyph = font.glyph(65).unwrap();
let advance = glyph.advance;
let ink = glyph.pixel(0, 0);
assert_eq!(resident('A').unwrap().name, font.name);
```

`resident(id)` resolves resident A–H and the GS face (`@`). C/D share a matrix;
E/H select their 203-DPI forms. The `zpl` renderer uses this same dataset.

The [bundled dataset](https://github.com/codyps/zpl/blob/main/zpl-bitmap-fonts/data/zd621/README.md)
contains 47 fonts, calibrated native metrics, and advancing blanks from verified
captures. These observations apply to the **ZD621 at 203 DPI, firmware V93.21.33Z**.
They do not establish compatibility with other printers, firmware, or configurations.
Only records with measured input equivalents and preview-derived maps are bundled.

## Glyphs and encoding maps

Glyph IDs and input keys are separate. Use
`font.encoding(zpl_bitmap_fonts::collection::Encoding::Input { ci })` to inspect
an input mapping's status and candidate record IDs, then `font.record(id)` for
raw glyph metrics and padded pixels. An absent map input is untested; an unresolved
input is not a verified blank. Font-level `metrics` is optional; `cell_metrics()`
returns measured dimensions, baseline, and space advance when present.

`font.glyph(source_key)` provides a convenience view over measured CI0 source
mappings. Its key is a byte source position, rather than a Unicode codepoint.
Matched candidate IDs are observed pixel/advance equivalents; selecting one does
not identify a unique physical record.

The glyph view crops ink bounds while borrowing the same bitmap bytes. Bearings
are signed native dots relative to ZPL's `^FT` origin. `bitmap()` uses row-padded
storage; use `row_offset(y)` for bit scanning or `pixel(x, y)` for individual pixels.

## Scalable-font captures

The `zd621` feature also includes fixed scalable-font captures for 0/P–V under
`zpl_bitmap_fonts::captures::zd621`. `Strike::glyph` resolves Unicode/layout keys
into the shared `Glyph` view. `VARIANTS` contains eight precompiled combinations
of legacy backslash, PA1, and native-control behavior (bits 0, 1, and 2), with the
enriched 32-dot fallback last. `CAPTURES` retains original capture identities;
these are not new measured input maps.

## Documentation and license

- [API documentation](https://docs.rs/zpl-bitmap-fonts)
- [Dataset provenance](https://github.com/codyps/zpl/blob/main/zpl-bitmap-fonts/data/zd621/README.md)
- [Custom datasets and regeneration](https://github.com/codyps/zpl/blob/main/docs/automatic-bitmap-fonts.md#json-and-compilation)
- [Source and issues](https://github.com/codyps/zpl)

Code is licensed under the [Open Software License 3.0](https://github.com/codyps/zpl/blob/main/LICENSE).
Font content retains its original licensing.
