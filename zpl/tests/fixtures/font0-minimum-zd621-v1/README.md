# Minimum font-0 size and whole-dot FO baseline

50 unmodified ZD621 203-DPI V93.21.33Z HTTP preview frames. Five conformance
cases come from the complete 2026-09-19 capture, with source and PNG hashes
verified before copying. Fresh captures on 2026-09-20 were serialized with
five-second pacing; all completed. Canvas widths are multiples of 64. No
render/reference image is scaled, cropped, padded or registered for scoring.

The guide's ^A p. 60 scalable size range begins at 10 dots. Boundary controls
show that requests 1, 2, 7 and 9 produce exactly the same pixels as 10,
independently for height and width; 11 and above differ. The
`font0_minimum_dimensions` option clamps positive resolved font-0 dimensions
to that minimum after zero inference / default inheritance. SPECIFICATION
rejects dimensions below the documented minimum. Bitmap fonts retain their
separate native-matrix rules.

A separate `font0_fo_floor_baseline` option rounds the normal horizontal
font-0 FO baseline down from three quarters of the height to whole dots.
Sixty-four native FO/FT pairs cover heights 10–25 and all four rotations,
including every fractional-quarter phase. This fixes a one-dot R/I displacement
at height 10. FT and bounded-text placement retain their separate rules.
Both options are enabled in ZD621_203_DPI and independently overrideable.

Native ASCII strikes at 10x10 (requested 10x0), 10x32 and 32x10 supply the
hinted glyphs. Offline extraction regenerates all three assets byte-for-byte
from their twelve sampling pages and checks separate composition previews.
`manifest.tsv` pins 46 complete renderer comparisons: 39 sampling/verification
frames, five original cases and two origin/rotation/default/zero holdouts.
The first 44 are exact. Each holdout pins 102 underpaint and 93 overpaint
pixels; every one of their 64 text regions exceeds 90% foreground IoU with
an explicit 80% floor and a complete-ink-coverage check.

`measurements.tsv` pins four additional native measurement frames. Their
region comparisons establish the size threshold and FO/FT baseline offset;
they do not claim renderer accuracy at all unsampled 11–25-dot sizes. The
boundary and baseline tests compare native controls to each other, while the
46-frame rendering test compares unmodified complete canvases. Wider font
size and Unicode coverage remain separate work.

References: [Zebra ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^A pp. 60–61, ^CF p. 154, ^FO p. 201 and ^FT p. 205. All tests are offline.
