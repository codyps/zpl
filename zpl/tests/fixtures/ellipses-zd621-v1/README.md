# ZD621 unequal-axis ellipse controls

Twelve unmodified HTTP Preview Label captures from ZD621 203 DPI, firmware
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

The `ellipse_printer_curve` option now uses measured steep-region steps, minor-
axis quantization, transposition for tall ellipses, and the captured cap/side
endpoint convention. All 272 instances improve or preserve foreground IoU.
The two initial border-one/filled atlases are exact. Other atlases retain up to
40 differing dots per frame because the shallow-region transition still needs
refinement. SPECIFICATION disables this approximation.

The additional 200 filled ellipses retain 944 differing dots. Their steep-region
steps agree with the measured rule, but the shallow-region updates need further
work as well as the transition. The earlier 272-instance improvement result
does not establish pixel parity for these independent samples. These four
frames pin the current errors and full output hashes without relaxing any of
the earlier regression expectations.

Some formerly overfilled frames now have a few missing pixels: for example, the
filled ratio atlas changes from 0 underpaint / 3538 overpaint to 12 / 0. This is
an inspected geometry improvement, not a relaxed tolerance; both counts and
the complete output hash remain exact regression requirements.
