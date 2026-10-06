# Native backslash character mapping

Fifty unmodified ZD621 203 DPI HTTP Preview Label responses, firmware
V93.21.33Z, captured 2026-09-20 with serialized requests and five-second
pacing. All requests completed. Glyph pages use widths divisible by 64;
layout atlases use PW832. No registration or image resizing is used.

Fifteen CI0 extraction pages and fifteen independent triple-glyph pages pin
native byte 0x5C ink and advances. Use the extract-font example with
`--encoding 0 --characters '\' --verify-text '\\\' --delay 5` to reproduce
the sampling method. The manifest records source, response and local pixel
hashes, and exact underpaint/overpaint counts. Asset hashes are also pinned.

Twenty held-out frames cover CI0/13/28, all fifteen embedded font settings,
ordinary text and escaped field-block backslashes, newline adjacency, all
rotations, FO/FT, field justification, and L/C/R block alignment. Two CI0/28
atlases contain 60 fields; six escape frames contain 48 fields; ten rotation
frames contain 128 fields; two alignment frames contain 36 fields.

43 frames are exact. Seven retain 6–14 overpaint dots each in font 0 and
zero underpaint (the alignment atlas has eight). The 82 fields in these
frames are individually scored, with minimum foreground IoU 99.6347%.
Their rectangles must cover every reference or candidate ink pixel exactly
once, preventing adjacent fields or blank canvas from hiding a regression.
FT controls were recaptured with ordinary and block fields on separate labels
so their ink does not overlap. All other frames must remain pixel-exact.

[Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf):
^CI pp. 156–159 documents CI0's character replacements and CI28 Unicode;
^FB p. 187 Item 1 requires CI13 for backslash escapes. CI0 always selects
its native replacement. `utf8_uses_legacy_backslash` separately enables the
printer's CI28 departure. `block_backslash_without_ci13` enables the printer's
acceptance of escaped backslashes under CI0/27/28. ASCII CI13 uses the normal
backslash and is supported with every compatibility option disabled.

The native bitmap glyphs differ from encoded U+00A2 in A/B/D/E/G/H. In
particular E/H paint their native replacement but have blank encoded-cent
glyphs. Keep those glyph sets distinct. Glyph selection applies to advances,
wrapping, alignment, per-glyph edge clamping, and inverted margins as well
as painting. The FO/R right-justified controls also establish that the printer
preserves line alignment inside the block; this is selected by
`block_fo_right_justification_printer_layout`.

Font extraction commands and extraction-only tests in this document now run
from the separate [private font research repository](https://github.com/codyps/zpl-font-extract).
The runtime assertions and captured bytes remain in ZPL.
