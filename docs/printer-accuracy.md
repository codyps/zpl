# Printer accuracy regressions

Run `cargo test -p zpl --test printer_accuracy -- --nocapture` for the offline
accuracy gate, or `cargo test --workspace` to include it with all existing tests.
No printer or external renderer is contacted by these tests.

Set `ZPL_ACCURACY_ARTIFACTS=/tmp/zpl-accuracy` when running the test to write
render/diff PNGs for differing cases. Magenta means underpaint; cyan means
overpaint. These diagnostics do not update references or expectations.

The corpus has 133 cases from `codyps-zpl` in the sibling `zpl-comparison`
benchmark and 31 independent controls for the fixes below. The original corpus
was taken from comparison commit `4346d33e1238de3f31cc0e81d493cff187494075`.
All comparison printer responses were **replaced** by new ZD621 previews using
`^PW832`; the old 812-dot barcode captures and test-side padding were removed.
This intentionally leaves printer preview width adjustment outside this work.

The printer is a ZTC ZD621-203dpi ZPL, firmware V93.21.33Z. These are HTTP
**Preview Label** responses, not scanned physical labels. Capture metadata,
input/image hashes and the separate state-reset request are in
[`provenance.json`](../zpl/tests/fixtures/printer-accuracy/provenance.json).
The argument capture includes a repeated control with identical pixels.
Barcode fixtures remain in `zebra-http-api/tests/fixtures/barcodes-zd621-v1`;
the zpl test reuses those files rather than maintaining a second copy.

## Regression contract

[`baseline.tsv`](../zpl/tests/fixtures/printer-accuracy/baseline.tsv) records each
case's exact underpaint (printer-only ink), overpaint (renderer-only ink), canvas
dimensions, output pixel SHA-256, input/capture SHA-256, and any known render
error. Pixels are compared at the original origin with threshold 128. There is
no padding, registration, rescaling, cropping, or whole-white-canvas tolerance.

Both improvements and regressions fail until their individual baseline rows are
reviewed and deliberately edited. The output hash also detects spatial changes
that preserve the two error counts. Known unsupported cases must retain their
specific diagnostic; replacing one failure with another is not accepted.
A successful render of a previously unsupported case also requires review.
There is no automatic baseline-update mode.

Before updating a row, inspect the reference and candidate and compare both
error counts. Do not raise a count to make CI green without explaining the
behavior change. Never replace a printer reference with a locally rendered PNG.
Source changes require a new printer preview and matching capture provenance.
A blank printer image is an observation, not evidence of barcode correctness.

## Rendering options used by the tests

Every captured-font, hardware barcode and corpus accuracy test starts with
`zpl::render::profiles::ZD621_203_DPI`, overriding canvas dimensions as needed.
The profile enables the independently selectable compatibility options described
in [printer profiles](local-renderer.md#printer-profiles). Existing pixel hashes
and overpaint/underpaint counts are unchanged by this separation. Tests in
`zpl/tests/profiles.rs` also cover the explicit specification profile, the ZD621 default, and individual
option overrides.

## Improvements verified against the printer

- White `^GB/^GC/^GE` painting uses white compositing rather than rejection.
  The captured overlapping white box is exact.
- `^B3` Mod-43 checksums are encoded before the stop character. The checksum
  probe is exact; the interpretation line still uses the original data.
- `^GD` diagonal lines use horizontal dot runs. Both directions and eight
  additional square, wide, tall, thin and thick controls are exact. Fractional
  slopes use a truncated fixed-point accumulator; untested dimensions remain
  subject to printer verification.
- `^BQ` with `^FO` uses the default `^BY` height minus one dot as its vertical
  offset. Independent heights 40, 60 and 100 verify this behavior. With `^FT`,
  the anchor includes a lower margin of three modules minus one dot; controls
  cover magnifications 1–5 and show independence from the `^BY` height.
- POSTNET/PLANET use a fixed 2.5-module pitch truncated per bar, independent of
  the variable-width barcode ratio. Module widths 1–3 are exact. Intelligent
  Mail uses this pitch and outward-rounded tracker boundaries; its corpus
  capture is exact.
- Standalone EAN-8/EAN-13/UPC-A/UPC-E extend their guard bars below the nominal
  height even with interpretation text disabled. The four corpus cases and
  extra EAN-13/UPC-A module-width controls are exact. Guard extension is 13 dots
  at 203 DPI and independent of font height in the tested 10/20/40-dot controls.
  The profile stores this as an explicit 13-dot override; changing DPI does not
  scale it. Other resolutions are not printer-verified.

Command references: Zebra ZPL II Programming Guide, `^B3` p. 70, `^BQ`
pp. 128–131, and `^GB/^GC/^GD/^GE` pp. 210–214. The command/page index is
[`zpl-command-index.tsv`](zpl-command-index.tsv); the full guide is available
from [Zebra](https://www.zebra.com/us/en/support-downloads/knowledge-articles/ait/zpl-command-information-and-details.html).
Dot-level placement rules above are empirical firmware observations, not claims
that the guide specifies those scan-conversion details.

## Remaining accuracy work

This is not yet 100% non-text parity. Of the 133 comparison cases, 109 are exact,
24 render with differences, and none report unsupported input. All 31 additional
controls are exact. Text and non-text failures remain separate in the provenance
and per-case baseline; aggregate percentages must not conceal either category.

Remaining non-text issues include circle/ellipse/rounded-box scan conversion,
QR automatic mask selection, Aztec/Data Matrix encoding choices,
MaxiCode, DataBar component layout, and TLC39 linked data.
The tested firmware produces identical QR images for requested masks 0/3/7;
the renderer still honors the documented mask operand. Do not substitute a
payload-specific mask table for the missing general selection behavior.
LOGMARS interpretation now includes its checksum and matches exactly.
The blank DataBar UPC-E printer response needs separate validation.

Independent barcode decoder tests remain necessary: different valid encodings
can decode to the same content without matching printer pixels. Captured cases
are development fixtures, not a holdout or proof for every parameter/payload.

## Formerly unsupported barcode modes

Code 128 above-text and automatic mode, QR Model 1, and CODABLOCK A now render.
The existing automatic Code 128 and CODABLOCK A captures match exactly. The
above-text case retains font differences; Model 1 retains the mask-selection
mismatch. Their baseline rows were changed from exact expected errors to reviewed
pixel counts and hashes. No printer reference was replaced.

An additional 159 [barcode mode controls](../zpl/tests/fixtures/barcode-modes-zd621-v1/README.md)
cover Code 128 starts/switches/FNC1/UCC/GS1, CODABLOCK A sizing/checks/padding,
all QR Model 1 versions, legacy Data Matrix ECC 000–140 at every size, `^CV`,
and PDF417/MicroPDF417 `^FM`. Run `cargo test -p zpl --test barcode_modes_preview`.
All 138 non-QR controls are pixel-exact. The 21 Model 1 controls pin
ordinary render differences and separately match every printer module when
using the mask encoded in the reference. That second check validates encoding
and placement without claiming that automatic-mask parity is implemented.

The expanded implementation also renders all 130 barcode-family/argument inputs
in the sibling comparison repository's conformance suite without unsupported
command errors. This is coverage evidence, not an assertion that all 130 match
hardware. New legacy/multiple-origin/validation references were captured from the
printer, while the existing accuracy references remain unchanged.

## Layout and CODABLOCK refinements

Fifteen additional [printer controls](../zpl/tests/fixtures/accuracy-refinements-zd621-v1/README.md)
require exact full-canvas equality. CODABLOCK F/E now uses correct subset-dependent
checks, includes mode E's implicit FNC1 in block checks, and draws internal/outer
separators. The profile selects firmware row heights in dots and fits rows to
actual data. Both original F/E comparisons are exact.

Field blocks support hanging indents and full justification. The profile enables
the captured trailing-space alignment rule, fixing centered text. Separate preview
options ignore `^LT` and `^PO`, while the specification profile honors both.
The original five affected layout/block cases and all fifteen controls are exact.

## Resident-font and caption refinements

All original text/layout cases now render pixel-exactly, including fonts A/D,
font-0 size/width cases, right/automatic origins, accented UTF-8 and Code 128
captions. The renderer selects captured strikes at supported sizes, scales native
bitmap-font matrices by integers, and preserves explicit font selection for
barcode captions. LOGMARS captions include the mandatory check character.

The [resident-font suite](../zpl/tests/fixtures/resident-fonts-zd621-v1/README.md)
adds 118 exact page/caption controls and 34 origin controls. Rotated font-0
controls pin up to four differing dots and exceed 99.3% ink IoU individually;
font A/D controls are exact. This does not promise 80% at arbitrary unsampled
sizes or for unimplemented scripts. The remaining 24 corpus differences are
non-text shapes/barcodes, still pinned and not claimed as complete.
