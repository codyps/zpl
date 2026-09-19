# ZD621 unequal-axis ellipse controls

Eight unmodified HTTP Preview Label captures from ZD621 203 DPI, firmware
V93.21.33Z, collected 2026-09-19 with zebra-render. Each source explicitly sets
PW832, label length and field origins. These are previews, not scanned labels.
No image is cropped, registered, rescaled or repaired for the comparisons.

- Three GE atlases cover 30 small even/odd widths, with height approximately
  half the width, at borders 1, 3 and 100.
- Three ratio atlases cover 34 size pairs, including squares, wide ellipses,
  tall ellipses and odd dimensions, at borders 1, 3 and 1000.
- Two independent holdout atlases cover 40 further width/height pairs at borders
  3 and 1000, including relatively prime sizes and heights as small as two dots.

The filled cases separate outer scan conversion from border geometry. Square
ellipses already use the verified circle curve; unequal-axis ellipses retain
known gaps. A rule that fits the first set but fails the holdouts is not evidence
of general printer parity. Sources preserve the exact dimensions and placements.

manifest.tsv pins source/response hashes, separate underpaint and overpaint,
and the entire local pixel hash. ellipse_preview.rs uses ZD621_203_DPI and
requires those exact values, so both improvements and regressions require an
explicitly reviewed baseline update. A passing test does not mean zero error.

Zebra Programming Guide ^GE, p. 214, specifies dimensions, border and color,
not the exact integer scan conversion observed here. See docs/zpl-zbi2-pg-en.pdf
and the [Zebra command reference](https://www.zebra.com/us/en/support-downloads/knowledge-articles/ait/zpl-command-information-and-details.html).
