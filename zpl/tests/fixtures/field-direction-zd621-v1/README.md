# Field direction: ZD621 203 DPI

These are unmodified HTTP previews from firmware V93.21.33Z. Ninety-three frames
were captured on 2026-09-20 in fourteen serialized batches, with five seconds between
requests. Every batch completed; its final FPH/PMN/PON reset control matches the
initial normal control pixel-for-pixel. Nine more frames are the hash-verified
`field-direction-*` cases from zpl-comparison's completed conformance capture.
All sources use PW832. No reference image is registered, padded, or resampled.

[Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FP p. 202 and Field Interactions pp. 1606–1611, defines horizontal, vertical,
and reverse field direction independently of glyph rotation. FP is field-local.
The specification profile honors its additional gap, including vertical text.

Two independent options select measured printer departures:

- `field_vertical_ignores_gap`: vertical glyph positions advance by font height,
  ignoring the requested extra gap.
- `field_direction_printer_anchors`: rotated right-justified positioning uses
  measured ink extents, leading-descender height, and terminal bearing offsets.
  Inverted vertical columns use the widest glyph; a single-character column has
  a different anchor. Proportional and bitmap fonts also differ by one-dot
  origin conventions. These rules were checked with both `Wj` and `jW`, single
  glyphs, unequal character widths, gaps, and multiple font sizes.

Both options are enabled in ZD621_203_DPI and disabled in SPECIFICATION.

The existing `right_justified_inverted_text_uses_ink_margin` option also controls
font-0 FO/I/right positioning. The printer measures the maximum right edge with
a backwards character pen, then subtracts it from the first advance. This
measurement can be negative (for example `jW`); glyphs still paint in normal
field order. It replaces the previous last-character bearing estimate. The
63 mixed pairs, repeated letters, spaces, three sizes, FO/FT and nonzero-gap
controls verify the calculation, including the pre-existing `gj ABC` regression.
Explicit FPH,0 versus omitted FP and CI13 versus CI27 have identical positioning.

The passing manifest contains 102 frames: 76 pixel-exact and twenty-six with small
font raster differences, totaling 90 underpaint and 34 overpaint pixels. The
697 text regions have a minimum foreground IoU of 98.9474%. Tests pin source,
raw PNG, rendered pixel hashes, and exact under/overpaint counts. They also
require at least 80% IoU in each region. Regions partition the canvas midway
between field anchors; the full-frame comparison covers all remaining pixels.

Coverage includes FO/FT, all four rotations, both justifications, H/V/R,
zero/nonzero gaps, field reset, and fonts A, F and sampled font-0 dimensions.
This is not a claim about unsupported fonts, shaping, encodings, or every size.

The two FP+FB cases now pass pixel-for-pixel, including either command order.
The separate `field-block-direction-zd621-v1` suite extends this coverage to
rotation, anchors, alignment, gaps and justified word placement.
