# ZD621 short barcode-caption glyphs at the edge

Eighteen raw ZD621 203-DPI previews, firmware V93.21.33Z, captured 2026-09-19.
All require pixel-exact equality across the entire 832 × 1218 canvas, including
text. The manifest pins source/capture hashes, zero underpaint/overpaint, and
the rendered pixel hash. No alignment, scaling, or cropping is applied.

Code 11 triangles/dashes and Code 128 dashes/dots cover module widths 1/2/3,
R/I orientations, and three origins per frame. Code 39, Code 93, and a native
Code 93 checksum triangle provide separate family holdouts at module width 2.
Each frame has three symbols, for 54 independently placed fields.

R/I caption glyphs preserve the blank rows below their ink within the
seven-row resident-A area when that area crosses the label edge. At module width 2, for example,
a dash occupies only two of those fourteen dots and retains six blank dots
below its ink. The renderer formerly moved that ink directly to zero. The
intermediate origin controls deliberately keep visible dash ink on the label
while its padded area still crosses the edge, distinguishing this rule from
clamping only when painted pixels are negative. Dots have no bottom padding.
N/B continue using their measured visible-ink clamp, covered by
`barcode-edges-zd621-v1`.

One-dot barcode heights isolate the captions at origin zero: the measured
R/I boundary rule removes those bars. Positive-origin controls retain the bars.
The correction also makes all four overlapping-caption/reverse-print holdouts
in `barcode-boundary-zd621-v1` exact, removing 12–112 underpaint and the same
number of overpaint dots per frame.

This is part of the independently selectable
`linear_barcode_clamps_negative_ink` behavior, enabled by ZD621_203_DPI and
disabled by SPECIFICATION. It applies to resident-A captions; these controls
do not establish corresponding metrics for other fonts.

Reference: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FO p. 201, ^FT p. 205, ^BA pp. 87–89. The raw printer previews establish the
undocumented padding and rotation behavior.
