# ZD621 resident G

Seventy-two raw ZD621 203-DPI previews, firmware V93.21.33Z, captured 2026-09-19.
Every frame requires exact full-canvas equality. The manifest pins input and
capture hashes, zero underpaint/overpaint, and rendered pixel hashes. Comparisons
do not align, scale or crop either image.

The native 60 × 40 strike supplies all 95 printable ASCII glyphs, previously
rejected by the renderer. Every glyph advances 48 dots (40 matrix plus eight gap
dots), with a zero-based native baseline of 47. Twenty-four extraction pages
and a separate composed-text verification preserve the raw source. Extraction
uses four characters per page so its tall cells fit the printer preview.

The 47 independent frames include full ASCII at native/doubled dimensions,
all rotations, FO/FT, left/right justification, omitted/zero CF/A sizes,
unequal-axis scaling, integer size rounding and field-block overflow. Origin
controls use a separate full frame for each orientation/origin/justification
combination, keeping the large glyphs inside the canvas. Four further controls
vary the inverted right margin across native/double/triple and unequal-axis
sizes, with wide/narrow glyphs and a trailing space.

Sixteen Code 128 frames cover explicit G captions at native/doubled sizes,
FO/FT, every rotation and above/below placement. These captions extend beyond
the bar width. The existing bitmap-font FT/CF sizing, inverted-margin,
reverse-caption and Code 128 bar-width options select the captured printer
behavior. SPECIFICATION disables those overrides; no G-specific option is
needed because the existing measured rules match every control.

To reproduce the packed asset offline, copy `font-source` to a temporary
directory and run `extract-font` with `--font G --height 60 --width 40
--batch-size 4 --offline --verify-text 'Abc XYZ 0123 !?'`, the saved source host
and copied output path. Compare `font.zbf` to `zpl/assets/fontG-60-40.zbf`.

Reference: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
Tables 29/31 (pp. 1582–1583), ^A pp. 60–61, ^BC p. 94, ^CF p. 154,
^FB pp. 185–187, ^FO p. 201, and ^FT p. 205. Coverage here is printable ASCII
at 203 DPI, not arbitrary encodings or resolutions.
