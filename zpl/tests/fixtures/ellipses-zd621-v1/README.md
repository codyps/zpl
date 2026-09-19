# ZD621 unequal-axis ellipse controls

Sixteen unmodified HTTP Preview Label captures from ZD621 203 DPI, firmware
V93.21.33Z, collected 2026-09-19 with zebra-render. Each source explicitly sets
PW832, label length and field origins. These are previews, not scanned labels.
No image is cropped, registered, rescaled or repaired for the comparisons.

- Three GE atlases cover 30 small even/odd widths, with height approximately
  half the width, at borders 1, 3 and 100.
- Three ratio atlases cover 34 size pairs, including squares, wide ellipses,
  tall ellipses and odd dimensions, at borders 1, 3 and 1000.
- Two independent holdout atlases cover 40 further width/height pairs at borders
  3 and 1000, including relatively prime sizes and heights as small as two dots.
- Four further filled atlases cover 200 unique width/height pairs, selected with
  Python `random.Random(621203)`: width 8–199 and height 2 through width minus one.
  These include nearly circular ellipses that were sparse in the earlier sets.
  Their exact dimensions are preserved in the ZPL sources.
- Four final holdout atlases cover 48 larger ellipses, with major axes 200–395
  dots, both orientations and borders 1, 3, 7, 20 and 1000. Dimensions were
  selected with Python `random.Random(621204)` after deriving the curve rule.

The filled cases separate outer scan conversion from border geometry. Square
ellipses use the verified circle curve. All 520 instances match exactly. A rule
that fits the first set but fails the holdouts is not evidence of general
printer parity. Sources preserve the exact dimensions and placements.

manifest.tsv pins source/response hashes, separate underpaint and overpaint,
and the entire local pixel hash. ellipse_preview.rs uses ZD621_203_DPI and
requires those exact values, so both improvements and regressions require an
explicitly reviewed baseline update. This suite now also requires zero error.

Zebra Programming Guide ^GE, p. 214, specifies dimensions, border and color,
not the exact integer scan conversion observed here. See docs/zpl-zbi2-pg-en.pdf
and the [Zebra command reference](https://www.zebra.com/us/en/support-downloads/knowledge-articles/ait/zpl-command-information-and-details.html).

The `ellipse_printer_curve` option uses measured steep-region steps, minor-axis
quantization, transposition for tall ellipses, and the captured cap/side endpoint
convention. The shallow region starts at the last steep-region point, truncates
aspect-ratio products before updating its integer decision value, and includes
the final center step. SPECIFICATION disables these printer-specific rules.

The earlier 272 atlas instances had 144 differing dots and the next 200 filled
ellipses had 944. Both sets now match exactly, as do the 48 larger holdouts.
The test additionally requires zero differing pixels across all 16 full frames;
the input, capture and complete output hashes remain pinned. Printer images
have not been regenerated or replaced by renderer output.
