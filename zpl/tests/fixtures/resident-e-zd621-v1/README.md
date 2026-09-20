# ZD621 resident E (OCR-B)

Thirty-three raw ZD621 203-DPI previews, firmware V93.21.33Z, captured 2026-09-19.
Every frame requires exact full-canvas equality. The manifest pins input and
capture hashes, zero underpaint/overpaint, and rendered pixel hashes. Comparisons
do not align, scale or crop either image.

The full native 28 × 15 strike supplies all 95 printable ASCII glyphs. Previously
only the ten retail-caption digits were embedded, and general E commands were
rejected. The new strike's digit metrics and bitmaps equal that original subset;
the existing retail-caption regression suite also checks the replacement.
Every glyph advances 20 dots, with a zero-based native baseline of 22.

Thirteen source frames preserve twelve extraction pages and an independent
composed-text verification. Twenty independent frames cover native/doubled
ASCII, CF/A omitted and zero dimensions, unequal scaling, integer size rounding,
wrapping, all rotations, FO/FT, and left/right justification. Code 128 controls
include explicit E captions above/below the bars, with both FO and FT origins.

The existing bitmap placement and caption compatibility options apply to E.
The right-justified inverted E margin is six native dots times horizontal scale,
plus two dots, although its ordinary advance uses a five-dot gap. Four extra
controls distinguish native/double/triple and unequal-axis sizes, with wide,
narrow and trailing-space final characters. The existing
`right_justified_inverted_text_uses_ink_margin` option selects this departure;
SPECIFICATION disables it.

The centered layout also exercises documented FB overflow, which must overprint
the final row. Broader overflow and explicit-break controls are preserved in
[field-block-overflow-zd621-v1](../field-block-overflow-zd621-v1/README.md).

To reproduce the packed asset offline, copy `font-source` to a temporary
directory and run `extract-font` with `--font E --height 28 --width 15
--batch-size 8 --offline --verify-text 'Abc XYZ 0123 !?'`, the saved source host
and copied output path. Compare `font.zbf` to `zpl/assets/fontE-28-15.zbf`.

Reference: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
Tables 29/31 (pp. 1582–1583), ^A pp. 60–61, ^BC p. 94, ^CF p. 154,
^FB pp. 185–187, ^FO p. 201, and ^FT p. 205. Raster departures are measured
from the preserved printer previews; coverage is printable ASCII at 203 DPI.
