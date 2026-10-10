# Supplied TrueType baseline regressions

These are unchanged native canvases from the controlled-font capture in
[codyps/zpl-comparison](https://github.com/codyps/zpl-comparison/tree/main/references/font-controlled).
`manifest.json` pins the comparison commit, font and input hashes, original
submission hashes, printer identity and repeated/restored controls. The captures
use a ZD621, 203 DPI, V93.21.33Z; they do not establish other-device parity.

`ComparisonHerosCondensedBold.ttf` is the exact renamed quadratic conversion of
TeX Gyre Heros Condensed Bold supplied to that printer and the comparison adapter.
Its GUST license accompanies it. Conversion provenance and source hashes are in
`benchmarks/fonts/sources.json` and `benchmarks/fonts/build.py` at the pinned
comparison revision. It has 1000 units per em, an hhea ascender of 1105, and
OS/2 cap height of 718. Neither metadata height is the ZPL cell baseline.

The original `.zpl` files are rendered with the font registered as ID 0, as in
the comparison adapter. `.submitted.zpl` preserves the exact printer submissions,
including aliases to the previously uploaded fonts. No device requests are made
by these tests. The saved session verified repeat and restoration pixel controls.

The 16/32/64-dot probes establish the removed 6/11/23-dot baseline displacement.
17/33/65-dot holdouts exercise whole-dot FO baseline flooring; rotations and
descenders cover positioning beyond the reported example. Comparisons use the
full original canvas and origin, with exact underpaint/overpaint/shared-ink
counts and raster hashes. All 35 canvases now match exactly with the ZD621
profile, including every foreground pixel. This is parity for this font and
capture session, not a claim about every font, firmware or printer.

## Supplied-font printer compatibility

The fixed corpus now contains 35 cases from the same comparison revision and
capture session, including the original 14 and 21 size/field-block holdouts.
Every source, submission and native PNG retains its original hash. The
`standard_rendering` record preserves the previous renderer's exact output when
`supplied_truetype_printer_metrics` is disabled; the outer counts/hashes pin the
new ZD621 profile. Neither test aligns, rescales, pads or crops a canvas.

The comparison's supplied-font path previously used `Environment::Standard` and
`ScanMode::Center`, bypassing the measured printer engine. Connecting the ZD621
outline scale, device-oriented scan conversion and 10-dot minimum improves
coverage. The minimum must also apply to the cell baseline: clamping only the
outline misplaced 1/2/7-dot requests. Rotation must precede scan conversion so
edge ties, dropout and GETINFO see the correct device axes; the resulting pixels
are returned to upright layout coordinates before the scene rotates them once.

Advance spacing required a separate correction. The previous low-level model
rounded requested sizes up to sixteenths of a point. It predicted H=10 at 16
dots, but incorrectly predicted j/space=6 at width 24 and W/m=50/47 at 65 dots.
The native strings require 5 and 51/48 respectively. Continuous layout scaling
by `max(width, 10) * 204 / 203`, followed by per-glyph nearest-dot rounding,
explains these cases and the independent 96-dot string. This is a shared empirical
rule, not a claim about firmware internals. It also preserves every exact
count in the existing 187-page/7,818-field constructed-font regression corpus;
outlines still use the separately measured downward point-size quantization.

The remaining contour differences came from several independent rounding stages:

- Aspect scaling rounds in integer font units before scaling to 26.6 device
  coordinates. Applying a floating-point anisotropic transform directly moved
  the right edge of narrow, stretched letters.
- Outline/CVT scaling rounds both the 16.16 ppem and the 16.16 coordinate
  multiplier. Rounding only the point-to-pixel matrix misplaced a stretched W.
- Implied quadratic midpoints truncate; subdivision midpoints round. Both use
  device Y-up coordinates. Subdivision uses one uniform depth per original
  curve and the measured `max(dx,dy) + min(dx,dy)/2` flatness norm. The initial
  straight-line test precedes rounded-midpoint depth selection.
- Directed center ties, monotonic-chain stub suppression, perpendicular dropout
  and minimum glyph height differ from the standard center-sampling scanner.

The 16-, 32-, 64-, 65- and 96-dot strings, descenders, all rotations, narrow and
wide aspect ratios, minimum dimensions and all field blocks now have zero
underpaint and zero overpaint. Native files and the opt-out results are unchanged.
The test explicitly requires zero differences in addition to checking hashes.

See [the independent contour witnesses](../truetype-raster-zd621-v1/README.md)
for printer captures that distinguish these rules, broader regression results,
and remaining artificial scaling residuals. The specification profile remains
standard; the ZQ610 profile does not inherit this calibration.
