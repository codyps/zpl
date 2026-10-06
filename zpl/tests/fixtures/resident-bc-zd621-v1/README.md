# ZD621 resident fonts B/C and bitmap-font FT placement

Sixty-nine raw ZD621 203-DPI previews, firmware V93.21.33Z, captured 2026-09-19.
Every frame requires pixel-exact equality, including all text. The manifest
pins input/capture hashes, zero underpaint/overpaint, and rendered pixel hashes.
No alignment, scaling, or cropping is applied to comparisons.

Font B was previously rejected. Twelve native extraction pages cover all 95
printable ASCII characters; a separate composed-text verification is the
thirteenth source frame. The checked-in `fontB-11-7.zbf` asset has its own
pinned hash. The printer renders lowercase input with uppercase glyphs, as
expected for this uppercase resident font. Each glyph advances nine dots:
seven matrix dots and two gap dots.

Font C was also rejected. It uses font D's documented 18 × 10 matrix; the
complete printable ASCII set is verified at native and doubled dimensions.
The renderer reuses the already captured D strike instead of duplicating it.

The other 56 independent frames cover ASCII, CF defaults, unequal width/height,
integer size quantization, centered wrapping, all four orientations, FO/FT,
and left/right justification. Scale controls include existing fonts A/D as
well as B/C, at native, double, triple and unequal-axis sizes.

Omitted/zero bitmap dimensions also have independent controls. A single
supplied axis determines the other from the native matrix. With neither axis,
A commands inherit both raw CF requests, including unequal-axis dimensions.
Explicit CF zero dimensions reset to native size. Selecting a bitmap font with
CF and omitting both dimensions also resets the ZD621 to native size, contrary
to the guide's last-CF inheritance rule. `bitmap_cf_font_only_resets_size`
selects that departure; SPECIFICATION retains the documented inheritance.

These controls found a shared bitmap-font FT departure. For vertical scale s,
the printer's placement relative to the geometric scaled-native baseline is:

| Rotation | X offset | Y offset |
| --- | ---: | ---: |
| N | 0 | 1-s |
| R | s | 0 |
| I | 1 | s |
| B | 1-s | 1 |

`bitmap_font_ft_dot_origin` selects these captured offsets for A/B/C/D. It is
enabled by ZD621_203_DPI and disabled by SPECIFICATION. It does not change FO,
font 0, or barcode captions. This coverage establishes ASCII support for B/C;
it does not claim arbitrary encodings or complete coverage of other fonts.

The raw extraction inputs, PNGs, configuration, glyph metadata and verification
report are preserved under `font-source`. To reproduce the packed asset offline,
copy that directory to a temporary directory and run `extract-font` with
`--font B --height 11 --width 7 --batch-size 8 --offline
--verify-text 'ABC xyz 0123 !?'`, the saved source host and copied output path.
Compare the resulting `font.zbf` to `zpl/assets/fontB-11-7.zbf`.

Reference: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
Table 29 (gap/baseline, p. 1582), Table 31 (203-DPI matrices and C/D alias,
p. 1583), ^A pp. 60–61, ^CF p. 154, and ^FT p. 205 Table 7. Exact raster placement comes from the
preserved printer controls.

Font extraction commands and extraction-only tests in this document now run
from the separate [private font research repository](https://github.com/codyps/zpl-font-extract).
The runtime assertions and captured bytes remain in ZPL.
