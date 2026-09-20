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

- `^GE` uses a measured scan conversion behind `ellipse_printer_curve`. The
  original case, all ten earlier controls, and 520 atlas instances are exact.
  Independent holdouts include nearly circular and very flat ellipses, both
  orientations, borders 1–1000, and major axes through 395 dots.

- `^GC` uses the captured integer circle curve and cap/side span endpoints.
  The original circle and twelve earlier controls are exact, along with
  [44 new atlas and holdout captures](../zpl/tests/fixtures/circles-zd621-v1/README.md)
  covering diameters 1–511, thin/thick/filled circles, equal-axis `^GE`,
  and white painting over black boxes.

- `^BY` starts with the documented 10-dot height and retains omitted operands.
  Twenty [Code 39/128 controls](../zpl/tests/fixtures/barcode-defaults-zd621-v1/README.md)
  match exactly after a separate default-state reset before each capture.

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

This is not yet 100% non-text parity. Of the 133 comparison cases, 127 are exact,
6 render with differences, and none report unsupported input. All 31 additional
controls are exact. Text and non-text failures remain separate in the provenance
and per-case baseline; aggregate percentages must not conceal either category.

The remaining non-text issue in the original comparison corpus is QR automatic mask selection.
The tested firmware produces identical QR images for requested masks 0/3/7;
the renderer still honors the documented mask operand. Do not substitute a
payload-specific mask table for the missing general selection behavior.
LOGMARS interpretation now includes its checksum and matches exactly.
The invalid six-digit DataBar UPC-E input and blank printer response are preserved
as a rejection control. Its positive case now uses eleven uncompressed UPC-A
digits and a fresh nonblank printer response; all four retail aliases match.

A fresh audit of the completed 512-frame comparison conformance reference set
also found unsupported print-quantity serialization, missing glyphs/encodings and unsampled font sizes below the
80% target. These remain work beyond the original 133 cases. Capture context
must be preserved: its reset uses `^BY2,3,100`, and some inputs still use PW812,
whose printer preview width adjustment remains outside this work. Those cases
need correctly contextualized comparisons or new PW832 captures, not padding.
`^PM` is now covered by [13 exact page-mirror controls](../zpl/tests/fixtures/page-mirror-zd621-v1/README.md).

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
bitmap-font matrices by integers, and honors explicit font selection for
Code 128 captions. Other families follow their separately captured profile
rules below. LOGMARS captions include the mandatory check character.

The [resident-font suite](../zpl/tests/fixtures/resident-fonts-zd621-v1/README.md)
adds 118 exact page/caption controls and 34 origin controls. Rotated font-0
controls pin up to four differing dots and exceed 99.3% ink IoU individually;
font A/D controls are exact. This does not promise 80% at arbitrary unsampled
sizes or for unimplemented scripts. The remaining six original-corpus
differences are QR mask cases, listed below and not claimed as complete.


`aztec_preview.rs` adds 38 exact printer comparisons covering all five text
sets, punctuation pairs, binary length boundaries, default/explicit error levels
and fixed compact/full layers. The default parity truncation and preservation of
contiguous binary runs are independently selectable printer options. Both original
Aztec aliases now match exactly. Independent decoder tests exercise both profiles.


`data_matrix_preview.rs` adds 67 exact ECC200 controls for all six compaction
modes, short tails, rectangular/forced sizes, binary length boundaries and
explicit/default escape characters. The implementation was checked against the
supplied ISO/IEC 16022:2006 second edition, Cor.1:2008 and Cor.2:2011. The latter
correct grading/reference decoding rather than encodation. ZD621 tilde defaults
and EDIFACT transition choices are separately selectable compatibility behavior.
The original Data Matrix corpus case is now exact; legacy modes remain covered.

DataBar Expanded's compressed GTIN, weight, price and date encodings follow
ISO/IEC 24724:2011 §7.2.5.4, pp. 26–29. Both original Expanded cases are now
pixel-exact. A further 48 [printer controls](../zpl/tests/fixtures/databar-expanded-zd621-v1/README.md)
require exact pixels. Separate compatibility options reproduce the firmware's
malformed long-weight/no-date encodings and adjacent-four-module-bar separator
templates; SPECIFICATION retains the standard encodings and separators.
Run `cargo test -p zpl --test databar_expanded_preview`.

MaxiCode now uses Annex F's numeric shifts, A/B latches and two/three-character
shifts, plus C/D/E lock-in. All five original MaxiCode cases match the printer.
The [61 focused captures](../zpl/tests/fixtures/maxicode-zd621-v1/README.md)
cover switching boundaries, all extended sets, control bytes, short inputs and
alternate origins. Printer options independently control dot geometry, terminal
latching, NUL termination, short-input suppression and mode-5 payload omission.
The last option deliberately matches an undecodable HTTP preview; specification
rendering retains mode-5 payload data. Run `cargo test -p zpl --test maxicode_preview`.

## Remaining measured gaps

The main 133-case comparison corpus has six mismatches. The 31 additional
controls in the same baseline are exact (158/164 baseline rows are exact).
These are current required counts, not a tolerance that may grow silently:

| Case | Underpaint | Overpaint |
| --- | ---: | ---: |
| `argument-qr-model-1` | 522 | 522 |
| `argument-qr-ec-M` | 522 | 540 |
| `argument-qr-ec-H` | 306 | 234 |
| `argument-qr-mask-3` | 603 | 441 |
| `argument-qr-mask-7` | 261 | 279 |
| `barcode-qr` | 464 | 496 |

Rounded boxes now match the original corpus and all twelve earlier controls.
A further [25 exact captures](../zpl/tests/fixtures/rounded-boxes-zd621-v1/README.md)
cover dense small-radius atlases and independent radii through 257. The integer
curve and geometry rules have separate printer options.

The original ellipse case and independent unequal-axis controls now match
exactly. QR encoding
matches captured symbols when given the captured mask, but the firmware's
automatic mask scoring is not yet reproduced; it also ignores the requested
mask. TLC39 text/numeric compaction, padding, component placement and extended
flag geometry now match the original corpus case and all 119 additional controls.
Short all-numeric payloads now use Numeric compaction, including one digit.
The profile reserves Byte-compaction capacity for additional fields while
retaining the actual Text/Numeric encoding; a separate option disables this
conservative sizing. Mixed-data numeric runs switch at fourteen digits. See the
[TLC39 controls](../zpl/tests/fixtures/tlc39-zd621-v1/README.md).

Focused suites cover more than the original corpus. All 48 additional DataBar
Expanded controls now match exactly, including the firmware-specific separator
templates and malformed no-date encodings. The resident-font origin controls
retain up to four differing dots; their baselines pin counts and pixel locations.

The [34 curved-shape controls](../zpl/tests/fixtures/shapes-zd621-v1/README.md)
were captured before the outage and preserve border, radius and even/odd size
boundaries. `cargo test -p zpl --test shape_preview` pins their current counts
and exact differing-pixel positions. The twelve rounded-box and twelve circle controls are exact;
all ten ellipse controls are also exact.

Additional [ellipse atlases](../zpl/tests/fixtures/ellipses-zd621-v1/README.md)
and [44 QR captures](../zpl/tests/fixtures/qr-zd621-v1/README.md) pin the remaining
work across aspect ratios, borders, both QR models, all correction levels and
several payload classes. Run `cargo test -p zpl --test ellipse_preview --test qr_preview`.
The ellipse test requires zero differing pixels. QR controls retain explicit
known-gap counts and pixel hashes; passing them is not a claim of zero QR error.

Four further filled ellipse atlases add 200 independently selected sizes,
including nearly circular ellipses. Their former 944 differing pixels are now
eliminated. Four final holdout atlases add 48 larger sizes with varied borders
and both orientations; all are exact. Raw printer captures and full renderer
pixel hashes remain pinned in the same regression test.

## Fractional Code 39 widths and broader conformance checks

Sixteen [Code 39 ratio captures](../zpl/tests/fixtures/code39-ratios-zd621-v1/README.md)
cover 56 symbols, including all ratio tenths from 2.0 to 3.0 at module widths
1, 3, 7 and 9. The ZD621 truncates each wide element to whole dots. Retaining
fractional widths previously accumulated position errors: the original
module-1/ratio-2.5 and module-3/ratio-2.5 controls had 2640 and 7600 differing
dots. All sixteen frames now match exactly. Run
`cargo test -p zpl --test code39_ratios_preview --test profiles`.

`code39_floor_wide_elements` independently selects the observed ^B3 behavior.
The specification profile instead follows the nearest-dot worked example in
^BY, p. 148. The printer profile enables truncation; both profiles now quantize
individual elements before accumulating their positions.

The broader conformance hardware audit also found differences in above-bar
interpretation layout, rotated interpretation text, and PDF417 compaction.
The captured cases are resolved by the fixes detailed below. Successful
rendering still does not establish pixel accuracy for the broader corpus;
overall non-text parity remains incomplete, including the six original QR cases.

## Standalone PDF417 numeric transitions

Thirteen [PDF417 numeric-transition frames](../zpl/tests/fixtures/pdf417-numeric-zd621-v1/README.md)
now require exact full-canvas parity for 118 symbols. The renderer previously
used an eight-digit Numeric threshold inside mixed text and encoded short
prefixes as bytes. The printer instead uses fourteen digits for mixed input,
eight for entirely numeric input, and preserves short Text prefixes before
Numeric runs. Six conformance security/truncation cases are now exact, including
the level-8 pair that each had 22764 differing dots. The seven boundary atlases
cover digit lengths 1–16 with uppercase/lowercase prefixes and suffixes.

These choices preserve valid PDF417 encoding under both profiles. Macro
PDF417 retains its separately verified eight-digit behavior; MicroPDF417 and
TLC39 remain independent. Run `cargo test -p zpl --test pdf417_numeric_preview`.
The captured above-bar and rotated interpretation cases are resolved by the
subsequent controls below. QR mask selection and broader unverified coverage
remain open.

## Barcode interpretation origins

Forty [interpretation controls](../zpl/tests/fixtures/barcode-interpretation-zd621-v1/README.md)
now match exactly, including their text. They cover all orientations for
above-bar Code 128, Interleaved 2 of 5, EAN-13 and UPC-A; below-bar Code 128
and Interleaved 2 of 5; and module-width 1/3 reversal holdouts. Run
`cargo test -p zpl --test barcode_interpretation_preview --test profiles`.

Three independent profile options reproduce the captured choices: rotated
^FO anchoring by bar height, above-bar interpretation that preserves the bar
origin, and a one-dot correction for reversed auto-centered interpretation.
Code 128 keeps its existing separate above-text option. ^FM component origins
are unaffected. Whole-canvas equality checks both bar geometry and text;
previous reversed-text IoU of roughly 52–54% is now 100% in these controls.

Code 39 interpretation and below-bar UPC/EAN layout remain unresolved, as does
QR automatic mask selection. The goal is not complete.

## Code 39 interpretation

Twenty-three [Code 39 caption controls](../zpl/tests/fixtures/code39-caption-zd621-v1/README.md)
now match the printer exactly, including every text pixel. They cover all
orientations, above/below captions, checksum, empty input, explicit fonts,
inherited orientation, and ^FT boundaries. Code 39 now shares the barcode
interpretation pipeline instead of its former separate caption placement.
Independent options control start/stop/checksum caption characters, ignored
explicit caption fonts, and the normal/bottom-up ^FT last-row anchor.
Run `cargo test -p zpl --test code39_caption_preview --test profiles`.

Below-bar UPC/EAN layout and QR automatic mask selection remain unresolved.
Broader conformance coverage still needs hardware verification; this is not
a claim that the overall accuracy goal has been achieved.

## UPC/EAN interpretation and baseline boundaries

Seventy-four [retail barcode controls](../zpl/tests/fixtures/retail-caption-zd621-v1/README.md)
now require full-canvas equality, including every digit pixel. The renderer
previously centered one font-A string across the bars; the printer divides
EAN-8, UPC-E, EAN-13 and UPC-A captions into groups around the guards and
uses OCR-B for wider modules. The new decimal OCR-B strike was independently
sampled and verified against a separate composed-text preview.

`retail_interpretation_printer_layout` selects the captured 203-DPI below-bar
layout, including integer OCR-B scaling, fixed four-dot gap, and rotation
about the bar width. `linear_barcode_ft_uses_last_bar_row` independently selects the
normal/bottom-up inclusive baseline, verified with captions above, below and
hidden. The controls also cover alternate payloads, check-digit suppression,
explicit fonts, all orientations, and module widths 1–7/9. Run
`cargo test -p zpl --test retail_caption_preview --test profiles`.

QR automatic mask selection remains unresolved. Broader conformance and
resident-font coverage still need hardware verification; the overall accuracy
goal remains open.

## Shared linear barcode FT boundary

Twelve [linear FT atlases](../zpl/tests/fixtures/linear-ft-zd621-v1/README.md)
add 72 exact fields across eighteen barcode variants and all four rotations.
Normal/bottom-up symbols formerly sat one dot early; rotated/inverted controls
were already exact. `linear_barcode_ft_uses_last_bar_row` now controls the shared
boundary rule, replacing the earlier Code-39-only and retail-only options.
This does not infer corresponding behavior for matrix/stacked symbols or
other postal variants. Run `cargo test -p zpl --test linear_ft_preview`.

## Linear interpretation symbols and Code 11 widths

[Thirty caption frames](../zpl/tests/fixtures/linear-caption-zd621-v1/README.md)
now pin Code 11 checksum/triangle glyphs, Code 93 box delimiters, Codabar
start/stop letters, POSTNET/PLANET pitch-based centering, and explicit-font
handling for ^B1/^B2/^B5/^BA/^BK. All thirty frames are pixel-exact. Their
previous caption-only IoUs ranged from about 12% to 72%; corrected unclipped
controls reach 100%. Each behavior has a separate compatibility option.

[Five Code 11 width atlases](../zpl/tests/fixtures/code11-widths-zd621-v1/README.md)
require zero differences for 148 symbols, including the complete width 1–10,
ratio-tenths 2.0–3.0 grid. `code11_printer_element_widths` selects independent
whole-dot quantization of wide and extra-wide elements. The extra-wide
formula is five-thirds of the unrounded wide width, rather than 2W-X.

Run `cargo test -p zpl --test linear_caption_preview --test code11_widths_preview
--test profiles`. The former left-edge rotated caption gap (1146 underpaint/207 overpaint)
is now resolved by the edge compatibility option below. Code 93 optional
checksum interpretation is covered by its dedicated suite below. QR mask
selection and broader unverified coverage keep the overall goal open.

## Code 93 checksum interpretation

[Twenty-three checksum frames](../zpl/tests/fixtures/code93-checks-zd621-v1/README.md)
now match exactly for 336 symbols. ^BA e=Y previously rejected rendering,
including fields with hidden interpretation. C/K encoding is now shared with
the caption formatter, and hidden captions leave the bars unchanged.

The controls cover every C and K value, every extended C/K pair, and separate
payload, size, rotation, font and full-ASCII holdouts. The printer's malformed
extended-check formatting is isolated behind `code93_extended_checksum_preview`;
the specification profile displays documented ZPL substitutes. Run
`cargo test -p zpl --test code93_checks_preview --test profiles`.

Code 93 checksum interpretation is resolved for these controls. QR mask
selection and broader unverified coverage remain open; the overall accuracy
goal has not been achieved.


## Barcode ink at negative edges

[Twenty-five edge frames](../zpl/tests/fixtures/barcode-edges-zd621-v1/README.md)
require zero differences for Code 11/39/93/128 and Interleaved 2 of 5. The
printer clamps bars and individual caption glyphs by their actual ink bounds,
then unions black ink. ^FR/^LR instead invert overlapping components twice.
`linear_barcode_clamps_negative_ink` controls this independently of nominal
^FO/^FT geometry. The earlier caption suite's final residual now becomes exact.
Run `cargo test -p zpl --test barcode_edges_preview --test linear_caption_preview
--test profiles`. The separately captured rotated origin-zero bar boundary
is resolved below; no full-coverage accuracy claim is implied.


## Rotated bar boundary at zero

[Twenty-four boundary frames](../zpl/tests/fixtures/barcode-boundary-zd621-v1/README.md)
pin Code 11/39/93/128 and Interleaved 2 of 5 at zero, positive and negative
origins. All twenty-four are pixel-exact after the boundary and padding fixes:
R at nonpositive X and I at nonpositive Y lose one height dot, independently
of negative-ink translation. The controls include heights 1–3 and 30/31/80,
FO/FT, all rotations, and home/shift offsets.

Four overlapping-caption holdouts formerly had Code 11 text residuals
(12–112 underpaint and the same overpaint count). The padding fix below now
resolves them. Run
`cargo test -p zpl --test barcode_boundary_preview --test barcode_edges_preview
--test profiles`. QR mask selection and broader unverified coverage also remain.


## Short-glyph padding in rotated barcode captions

[Eighteen padding frames](../zpl/tests/fixtures/barcode-padding-zd621-v1/README.md)
require exact full-canvas equality for 54 Code 11/128/39/93 symbols. R/I
captions retain blank bottom rows of resident-A glyphs when their seven-row
area crosses the label edge, even when their visible ink is already positive.
N/B retain their measured visible-ink clamp. Widths 1/2/3, dots, dashes,
triangles and a native checksum glyph distinguish these behaviors.

The existing `linear_barcode_clamps_negative_ink` option selects this placement;
SPECIFICATION disables it. Run `cargo test -p zpl --test barcode_padding_preview
--test barcode_boundary_preview --test barcode_edges_preview --test profiles`.
The boundary suite's last four caption residuals now become exact. Remaining
QR-mask differences and broader font/command coverage still leave the overall
accuracy goal open.


## Resident B/C and magnified bitmap-font FT origins

[Sixty-nine resident-font frames](../zpl/tests/fixtures/resident-bc-zd621-v1/README.md)
now require exact full-canvas equality. B and C text commands were previously
rejected. B uses a freshly captured 95-character native strike; C reuses the
documented C/D matrix, verified across the full printable ASCII set. Source
sampling, the independent composed-text verification and the B asset hash are
preserved alongside the layout controls.

The audit also found shared FT offsets for A/B/C/D at native and magnified
sizes. `bitmap_font_ft_dot_origin` selects the measured dot placement after
rotation; it leaves FO and proportional font 0 unchanged. The specification
profile retains geometric scaled-native-baseline placement. Controls cover
scales 1/2/3, unequal axes, all rotations, field justification, CF defaults,
size quantization and wrapping. Ten further frames verify zero/omitted dimensions:
a supplied axis determines the other, and A commands inherit both CF requests.
`bitmap_cf_font_only_resets_size` selects the printer's native-size reset when
CF selects a bitmap font without dimensions; SPECIFICATION retains the prior
CF request as documented. Run `cargo test -p zpl --test resident_bc_preview
--test resident_fonts_preview --test profiles`.

This establishes ASCII coverage for B/C, not arbitrary encodings or all
remaining resident fonts. QR-mask differences and broader unverified text and
command coverage still leave the overall accuracy goal open.

## Resident F and explicit bitmap Code 128 captions

[Forty-one additional frames](../zpl/tests/fixtures/resident-f-zd621-v1/README.md)
require pixel-exact equality. They include all 95 printable ASCII F glyphs,
an independent extraction verification, and 28 layout/caption holdouts.
F now supports native and integer-magnified sizes, FO/FT, justification,
wrapping, CF sizing and all orientations with the measured bitmap options.

The caption controls also correct explicit A/B/C/D/F reverse interpretation
placement and a Code 128 FO pivot error when captions extend outside the bars.
`barcode_reverse_interpretation_shift` and `code128_fo_uses_bar_width` select
these departures independently; the specification profile disables both.
Run `cargo test -p zpl --test resident_f_preview --test profiles`.

This does not close the overall accuracy goal. QR mask selection, remaining
resident-font support and proportional-font rotated glyphs still need work.

## Resident E and field-block overflow

[Thirty-three resident E frames](../zpl/tests/fixtures/resident-e-zd621-v1/README.md)
are exact. The full 95-character OCR-B strike replaces the retail-only subset;
its original digits are unchanged. CF/A sizing, all rotations, justification,
wrapping and explicit Code 128 captions are covered. Inverted right-justified
E uses a six-dot scaled margin plus two dots under the existing margin option.

[Thirty-six field-block frames](../zpl/tests/fixtures/field-block-overflow-zd621-v1/README.md)
cover documented last-row overflow, previously rejected, and explicit breaks.
Bitmap A/F frames are exact; font-0 frames score 97.98–100% foreground IoU.
Exact residual counts and pixel hashes guard every frame, in addition to the
80% text floor. Explicit breaks end justification and do not add the printer's
centering space. Overflow ink is unioned on the final row.

Run `cargo test -p zpl --test resident_e_preview --test field_block_overflow_preview
--test retail_caption_preview --test profiles --test render`. QR masks,
remaining resident-font support and wider command coverage still leave the
overall accuracy goal open.

## Resident G

[Seventy-two resident G frames](../zpl/tests/fixtures/resident-g-zd621-v1/README.md)
are pixel-exact. The full 95-character 60 × 40 strike has a 48-dot advance and
zero-based baseline 47. Twenty-five source frames preserve extraction and
independent verification; 47 holdouts cover full ASCII, CF/A sizing, all
rotations/origins/justifications, scale quantization, wrapping and explicit
Code 128 captions wider than the bars. Existing bitmap compatibility options
match G without an additional departure.

Run `cargo test -p zpl --test resident_g_preview --test profiles`. QR masks,
resident H and wider unverified text/command coverage still leave the overall
accuracy goal open.

## Resident H and valid blank glyphs

[Forty-one resident H frames](../zpl/tests/fixtures/resident-h-zd621-v1/README.md)
are pixel-exact. Native OCR-A uses a 21 × 13 matrix, 19-dot advance and zero-based
baseline 20. Lowercase a–z are advancing blanks, confirmed by individual
sentinel probes and independent native/doubled printer comparisons to spaces.
The extractor now accepts blank glyphs with validated positive advances while
retaining its nonempty-probe and preview checks. The renderer supports all 95
ASCII inputs, including the 27 blanks (space and lowercase).

Controls cover sizing, scaling, all origins/orientations/justifications,
wrapping, margins and wide Code 128 captions. Run `cargo test -p zpl
--test resident_h_preview --test profiles` and `cargo test -p zpl-font-extract`.
The existing printer options cover H. QR mask selection, unsampled font-0
sizes and broader unverified command/encoding coverage still leave the overall
accuracy goal open.

## Graphic symbols and 24-dot text

[Sixty graphic-symbol controls](../zpl/tests/fixtures/graphic-symbols-zd621-v1/README.md)
add `^GS` rendering, its 95-entry ASCII strike, and font 0 at 24 by 24 dots.
All sixty frames are exact, including the field-block frame that switches
from symbols to ordinary text.
The capture includes independent composed verification for both new strikes.

The ZD621 profile enables two new options: `graphic_symbol_last_row_baseline`
and `graphic_symbol_ignores_justification`. SPECIFICATION disables both,
using Table 29's three-quarter-height baseline and honoring FO/FT justification.
Tests cover the options independently, GS/barcode command ordering, dimensions,
all rotations/origins, reverse fields and label reversal. Run `cargo test -p zpl
--test graphic_symbols_preview --test profiles` and `cargo test -p zpl-font-extract`.
QR mask selection, justified-text spacing and broader unverified sizes,
commands and encodings still leave the overall accuracy goal open.

## Justified word-position rounding

[Six spacing atlases](../zpl/tests/fixtures/field-block-rounding-zd621-v1/README.md)
contain 72 controls using fonts 0 and A, one to three word gaps, and widths
that distribute fractional slack. Every full frame is pixel-exact. The new
`block_justification_rounds_up` option selects the ZD621's integer spacing distribution;
SPECIFICATION keeps nearest-dot rounding. The remaining GS field-block residual
improves from 55 underpaint/overpaint dots to zero. Other saved justified blocks
retain their previous results. Run `cargo test -p zpl --test field_block_rounding_preview
--test field_block_overflow_preview --test graphic_symbols_preview --test resident_fonts_preview
--test profiles`. Soft-hyphen escapes are covered by the marker suite below; broader command coverage remains open.

## Automatic field-block hyphenation

[Sixty-one hyphenation frames](../zpl/tests/fixtures/field-block-hyphenation-zd621-v1/README.md)
replace the previous overlong-word error with measured wrapping. Fifty-nine
frames are exact; two font-0 B layouts retain three underpaint dots each and
zero overpaint. Controls cover width boundaries, existing text
prefixes, hanging indents, final-row overflow, all alignments and rotations,
FO/FT and CI0/27/28. Sixteen independently verified font captures supply native
soft-hyphen/eth glyphs without modifying the previous ASCII assets.

`block_hyphenation_printer_layout` controls soft-hyphen sizing, strict fit and
retained hyphen space on the final chunk. `block_hyphenation_ci27_uses_eth`
separately reproduces the printer's incorrect CI27 automatic-break glyph.
SPECIFICATION disables both. Run `cargo test -p zpl --test field_block_hyphenation_preview
--test profiles`; the full renderer suite checks shared wrapping and font behavior.
The later marker and narrow-block suites cover those cases. Unsampled sizes,
QR mask selection and broader command/encoding coverage remain open.

## Graphic dimensions and placement

[Three box atlases](../zpl/tests/fixtures/box-minimum-zd621-v1/README.md) cover
27 zero, omitted and undersized dimension controls. Both profiles now honor
the documented minimum/default thickness before computing rounded corners.

[63 placement frames](../zpl/tests/fixtures/graphic-placement-zd621-v1/README.md)
are pixel-exact across shifted origins, FO/FT, all FW values, top-edge boundaries,
right justification, odd/even circle sizes and recalled bitmaps with blank rows.
Graphics no longer inherit FW rotation, and right justification uses nominal
width. The independently selectable `graphic_ft_last_row_baseline` and
`graphic_clamps_negative_origin` options reproduce ZD621 dot placement;
SPECIFICATION disables both. Run `cargo test -p zpl --test box_minimum_preview
--test graphic_placement_preview --test profiles`.

## Narrow field blocks

[Eight narrow-block atlases](../zpl/tests/fixtures/field-block-narrow-zd621-v1/README.md)
cover 216 controls across font 0 and resident A/B/D/E/F/G/H settings. All frames
are pixel-exact. The new `block_narrow_printer_layout` option emits a character
when a block cannot fit a character plus hyphen, retains the measured exact-fit
hyphen and separator behavior, and clamps negative alignment slack.
SPECIFICATION instead suppresses text below the documented font-width minimum.
Run `cargo test -p zpl --test field_block_narrow_preview --test profiles`.
Explicit escapes and negative rotated text origins remain open.


## Centering before rotation

[Four centering atlases](../zpl/tests/fixtures/field-block-centering-zd621-v1/README.md)
contain 128 pixel-exact controls across fonts 0/A, FO/FT, every rotation and
odd slack widths. `block_center_rounds_down` quantizes each centered line
before rotating it; SPECIFICATION retains fractional positions. Fourteen
older overflow/hyphenation frames improve, with six becoming exact. The 62
saved centered-text frames reviewed show no increase in either error count.
Run `cargo test -p zpl --test field_block_centering_preview --test field_block_hyphenation_preview
--test field_block_overflow_preview --test profiles`.

## Text origins at canvas edges

[106 edge and origin frames](../zpl/tests/fixtures/text-edge-zd621-v1/README.md)
cover 912 independently scored fields. Every field exceeds 99.84% foreground
IoU; 85 frames are pixel-exact and 21 retain only pinned font-0 overpaint dots.
The previously 79.21% narrow FT-B field is now exact. The tests partition all
ink into field regions and enforce the 80% target separately for each field.

`text_clamps_negative_origins` clamps the shifted field anchor, then each
rotated glyph's ink origin, unioning overlapping ink. The independent
`block_fo_right_justification_printer_layout` option reproduces the B/I right
anchors for FO blocks. SPECIFICATION disables both. Run `cargo test -p zpl
--test text_edge_preview --test profiles`. Field-block escapes, QR mask
selection and broader unverified coverage still leave the overall goal open.

### Field-block backslash escapes

[Four backslash atlases](../zpl/tests/fixtures/field-block-backslash-zd621-v1/README.md)
cover 96 CI27 fields across resident fonts 0/A, narrow wrapping, repeated
backslashes, and adjacent explicit newline escapes. Every full canvas is
pixel-exact. Escape decoding consumes a backslash pair before recognizing a
newline, so an escaped backslash followed by `&` stays on its line.

`block_backslash_without_ci13` enables the printer's CI0/27/28 departure from
the ^FB p. 187 CI13 prerequisite. The specification profile disables it.
ASCII CI13 is supported. CI0/28's native cent-glyph substitution is covered by
the separate legacy-backslash suite below. QR mask selection remains open; these results do not establish the overall goal.

Run `cargo test -p zpl --test field_block_backslash_preview --test profiles`.

### Encoded cent characters

[Thirty-two cent-glyph frames](../zpl/tests/fixtures/cent-glyph-zd621-v1/README.md)
verify U+00A2 across fifteen embedded font settings. Separate extraction and
composition frames pin glyph metrics, assets, and zero underpaint/overpaint;
60 held-out CI27/CI28 fields verify ordinary text and centered field blocks.
All frames are pixel-exact. E/H retain the printer's blank, advancing glyphs.
The legacy CI0 backslash substitution uses a different cent design in some
bitmap fonts; the separate legacy-backslash suite covers those glyphs.

Run `cargo test -p zpl --test cent_glyph_preview`.

### Legacy backslash mappings and rotated block alignment

[50 native-backslash frames](../zpl/tests/fixtures/legacy-backslash-zd621-v1/README.md)
cover fifteen font settings, encodings CI0/13/28, escapes, rotations, origins,
and alignment. 43 frames are exact; seven have 6–14 font-0 overpaint dots and
no missing ink. All 82 individually scored fields in those seven frames
exceed 99.63% foreground IoU, with complete, disjoint ink coverage required.

CI0 uses its documented native glyph; `utf8_uses_legacy_backslash` independently
selects the printer's CI28 departure. ASCII CI13 is supported without
compatibility options. The FO/R controls also fix canceled line alignment in
right-justified blocks, under `block_fo_right_justification_printer_layout`.

Run `cargo test -p zpl --test legacy_backslash_preview --test profiles`.

### Explicit field-block soft markers

[Twenty-two marker frames](../zpl/tests/fixtures/field-block-markers-zd621-v1/README.md)
cover 520 fields across widths, indentation, multiple markers, punctuation,
rotations, alignment, and CI0/27. Eighteen frames are exact; four rotated font-0
frames each retain 6 underpaint and 8 overpaint dots. Their 32 individually
scored fields all exceed 99.89% foreground IoU with complete ink coverage.

The specification profile recognizes documented alphanumeric markers.
`block_soft_hyphen_printer_layout` independently enables the printer's extra
marker syntax, blank-row and indentation behavior, and overflowing remainder
lines. Automatic hyphen metrics and CI27's eth glyph retain their separate
options. Existing ordinary wrapping and backslash regressions remain in place.

Run `cargo test -p zpl --test field_block_markers_preview --test profiles`.

Field direction now has [102 passing printer comparisons](../zpl/tests/fixtures/field-direction-zd621-v1/README.md),
with 697 text regions at 98.9474% foreground IoU or better. Both FP+FB
command-order cases are exact. [41 additional directed-block frames](../zpl/tests/fixtures/field-block-direction-zd621-v1/README.md)
cover wrapping, rotation, alignment and quotient/remainder word justification;
all 230 regions exceed 99.80% IoU.

### Bounded text blocks

[50 TB frames](../zpl/tests/fixtures/bounded-text-zd621-v1/README.md) cover
rectangle layout, height clipping, narrow wrapping, spaces, angle escapes,
command order, all rotations and both origins/justifications. Forty-one frames
are exact; every one of 362 regions exceeds 88.23% foreground IoU. The twelve
independent conformance TB cases now meet the text target. An exact 28x14 font-0
ASCII strike replaces scaled glyphs that previously scored around 34%.

Separate options select measured printer line leading, rectangle anchors, and
cancellation by later font selection; SPECIFICATION disables them. Source/PNG/
render hashes and exact under/overpaint pins prevent silent regressions.
Nondefault FP combined with TB remains explicitly unsupported; this suite does
not establish accuracy for every text layout combination or font size.

Run `cargo test -p zpl --test bounded_text --test bounded_text_preview`.

### Retail field-data lengths and validation

[Twelve exact retail-data frames](../zpl/tests/fixtures/retail-data-zd621-v1/README.md)
cover EAN-8, EAN-13 and UPC-A padding, truncation, supplied checks, nondigit
coercion and empty input. A 1–20-character sweep and independent CVN/CVY
controls match every pixel, including captions and validation labels.

Both profiles now implement the guide's short-data zero padding and left
truncation. Separate printer options select recomputed supplied check digits,
nondigit-to-zero conversion, and the measured overlong EAN data windows.
CVY still validates the original data before normalization. UPC-A overlong
validation now reports INVALID-L; the EAN-specific INVALID-S option no longer
applies to UPC-A. UPC-E and DataBar input rules remain separate.

Run `cargo test -p zpl --test retail_data --test retail_data_preview`.

## Initial serial fields

`serial_preview` compares eight complete raw ZD621 frames with zero underpaint
and overpaint. Text, Code 128 and EAN-13 share `^SN` initial-value handling,
including default values, Y/N zero suppression, embedded numbers and hex escapes.
An independent holdout verifies that field-width-preserving suppression occurs
after `^FH` decoding. The documented twelve-digit indexing limit is enforced in
`SPECIFICATION`; `serial_overlong_keeps_value` selects the measured printer
behavior of retaining longer numeric runs unchanged. `^PQ` iteration remains an
explicit unsupported-command error. See the fixture README and Zebra guide
`^SN`, pp. 341–342, for capture provenance and command semantics.

## Unicode resident-font strikes and advanced-text defaults

`unicode_fonts_preview` pins 40 complete native frames: 38 are pixel-exact;
the two mixed-text rotation holdouts retain 11 underpaint and 3 overpaint
pixels in total. Every independently positioned N/R/I/B text field is at
least 99.7956% foreground IoU, above the 80% requirement. Source, PNG, rendered
pixel and font-asset hashes prevent an unnoticed regression.

Font 0 now has an exact 40×24 ASCII strike, its existing accented/special
characters, and all 27 Hebrew letters/final forms at 40×24 and natural-width
32 dots. The six sampled missing characters from `advanced-text-0000` have
the native blank seven-dot advance at 40×24 with advanced features disabled.
The unchanged original case was freshly captured and now matches every pixel.
`^PA` defaults are accepted. Enabled advanced properties are covered by the
additional controls below; unmeasured glyphs remain explicit errors.

The sampler emits real UTF-8 byte escapes for `^CI28`. ZBF2 stores Unicode
scalar values; the decoder still accepts ZBF1 and the exporter preserves
byte-identical ZBF1 output for existing one-byte strikes. See the fixture
README and the assets format documentation for provenance and limits.

## Advanced text properties and bidirectional layout

`advanced_text_preview` adds 49 complete native frames, 45 pixel-exact,
including all six original `advanced-text-*` cases. All 58 separately checked
rotated/wrapped text regions exceed 80% foreground IoU (minimum 93.2298%).
The four non-exact frames retain 870 underpaint and 862 overpaint pixels;
raw input, PNG, rendered-pixel and font hashes pin every accepted residual.

PA selects measured default glyphs and Unicode bidirectional layout. FB
ignores bidi as documented; TB wraps logical paragraphs before reordering
lines, and supports automatic script-dependent or explicit justification.
Native controls cover all 16 flag combinations, independent ligature/kerning
strings, Hebrew final forms, bidi brackets, formatting controls, persistence,
rotations and wrapped paragraphs. Shaping/OpenType toggles leave this embedded
repertoire unchanged; broader shaping and unmeasured glyphs are not claimed.

Three independently selectable compatibility options reproduce native PA
operand persistence, omission of paired-bracket rule N0 and older treatment
of isolates as class-L missing characters. All are disabled in SPECIFICATION
and enabled in ZD621_203_DPI. See the advanced-text fixture README for provenance
and the sequential-label context used by the persistence test.

## Inline numbered fields

`numbered_fields_preview` pins 19 complete native frames, all pixel-exact,
including the unchanged original inline-numbered-fields case, forward and
multiple references, independent numbers, empty/rebound data, mixed barcode/
text styles, FH handling, prompts, boundary numbers and FV. The renderer plans
data substitutions before drawing, preserving the original command syntax and
error offsets. Bindings do not leak across labels.

The ZD621's preview only populates preceding unresolved references; it consumes
the binding's own drawing when references exist, and leaves later references
blank. `numbered_fields_forward_only` independently controls that departure.
SPECIFICATION shares the last supplied value with all data-less fields of the
same number, while retaining explicit data on the binding fields themselves.
Stored DF/XF formats and FN serialization remain explicit unsupported cases.
The fixture README records capture provenance and the excluded unverified
custom-prefix request.

## Initial serialization masks

The original `serial-mask` conformance input now renders pixel-exactly. `^SF`
validates decimal, hexadecimal, octal, alphabetic, alphanumeric and skip masks
and the combined 3 KiB mask/increment limit after FD/FV. It retains the initial
field value, as documented; `^PQ` iteration remains an explicit unsupported
command. This does not claim multi-label serialization support.

`serial_mask_preview` pins 14 complete native frames, including text, barcode,
FH and independent FD/SF pairs across CI0/13/27/28. Ten frames are exact; four
retain zero underpaint and 168 overpaint pixels. The latter reveal an existing
CI13 font-A zero difference (native unslashed versus embedded slashed), present
with or without SF. All 18 separate text regions meet 80% foreground IoU;
minimum 88.6792%. Barcode pixels have a separate exactness requirement.

## Field concatenation and substring extraction

All five original `field-concat-*` cases now render exactly. The
`field_concatenation_preview` test pins 26 complete native frames, all with zero
underpaint and overpaint, including mixed text/barcodes, nested and missing
bindings, hex decoding, Unicode character indexing, delimiter and scope controls.
The existing numbered-field planning pass resolves concatenation before drawing;
inserted command bytes remain data and errors retain original input offsets.

Three independent compatibility options select native backward extraction,
retention across intervening commands, and permissive token/delimiter parsing.
SPECIFICATION disables them and follows the guide's backward example and
adjacency rule; ZD621_203_DPI enables them. Both retain the field-data size limit.
The fixture README records exact controls and remaining scope limits, including
unsupported non-ASCII delimiter operands and stored formats.

## Unavailable resident font selection

The 20 original `font-id-*` cases for IDs 1–9, I–O and W–Z now match the
printer exactly. `unavailable_fonts_use_default` selects the measured ZD621
resolution: unavailable ^A fonts inherit the current ^CF family, independently
of preceding fields; unavailable ^CF fonts select A. SPECIFICATION continues
to report unavailable font assets as unsupported. P–V are genuine preset
fonts and are excluded from this fallback.

`font_fallback_preview` pins 33 raw printer frames with source/image hashes
and exact underpaint/overpaint counts. Thirty-two frames are pixel exact;
the four-rotation font-0 frame retains two underpaint and two overpaint dots,
with each text field independently required to exceed 80% foreground IoU.
Thirteen fresh controls cover default families 0/A/B, sizing, field history,
rotations and repeatability. See the fixture README for capture provenance
and the relevant ^A/^CF programming-guide sections.

## Preset font P

Font P now renders using its native proportional glyphs, its documented 20x18
matrix and separately captured hinted sizes. The original `font-id-P` case
is exact. A profile option controls its measured rotated FO pivot; FT keeps
its baseline origin. The mixed-font control also fixes font 0 at 20x18.

`resident_p_preview` pins 59 native frames: 57 exact, two with 10 total
underpaint and 16 overpaint dots. All 24 separate rotated text regions exceed
80% foreground IoU (minimum 99.6610%). Offline extraction tests reproduce all
four 95-character assets from their raw native pages and verify independent
composition. See the fixture README for provenance and specification links.
Uncaptured sizes remain approximations; S–V and extended encodings remain open.

## Preset fonts Q and R

Both original font-id-Q/R conformance cases now match exactly, using native
28x24 and 35x31 proportional strikes and measured face baselines. The existing
preset FO compatibility option covers their different vertical pivots; FT
keeps its baseline anchor. SPECIFICATION disables that placement option.

`resident_qr_preview` pins 33 native frames: 31 exact, with eight underpaint
and ten overpaint dots in rotated controls. All 16 isolated text regions exceed
99.75% foreground IoU. Offline extraction tests reproduce both 95-character
assets from raw sampling pages and independently verify composition. See the
fixture README for provenance, specification references and scope limits.

## Font-0 anchor accuracy at 40x22

All 24 original FO/FT anchor cases now match every pixel, including their
non-text reference crosses. The fix embeds the actual 40x22 hinted strike
instead of scaling another size; existing origin/rotation rules are retained.
`font0_40_22_preview` pins these frames and thirteen extraction/verification
frames, all exact. An offline extraction test reproduces the 95-character
asset byte-for-byte and verifies independent composition. Other unsampled
sizes remain open; see the fixture README for provenance and guide references.

## Common font-0 sizes and field-block paragraphs

Native 24x12, 20x10, 32x20 and 26x16 strikes fix the text-content, repeated-field,
equivalent-input and compact-baseline cases. Their accurate metrics also exposed
two field-block layout errors: hard breaks restart hanging indentation, and
horizontal justification assigns leftover dots to the earliest word gaps.
`block_hard_break_resets_indent` controls the first behavior (off in SPECIFICATION,
on in ZD621_203_DPI); the existing justification option now uses the measured
quotient/remainder rule. Earlier one-to-three-gap controls could not distinguish
it from cumulative ceiling. New four-to-six-gap controls do, and remain exact.

`font0_common_preview` pins 77 raw frames: 76 exact and one dense geometry label
with seven underpaint and seven overpaint dots, all in rotated text. All 36
isolated text regions exceed 93.33% foreground IoU. A fresh shapes-only printer
preview verifies every non-text pixel separately. All four compact wrapping
alignments and the new paragraph controls now match every pixel. Offline tests
reproduce all four 95-character assets from native sampling pages and verify
independent compositions. Other sizes, presets S–V and encoding gaps remain open.

## Font-0 28x14 hyphenation coverage

Thirteen field-block cases that failed on a missing soft-hyphen glyph now match
exactly. A two-glyph supplement supplies the measured soft hyphen and eth at
28x14, preserving the existing profile-controlled CI27 painting behavior.
`font0_28_14_hyphen_preview` pins all thirteen cases plus the raw sampling and
independent verification frames, all with zero underpaint and overpaint. An
offline extraction test reproduces the asset; no layout rules change here.

## Repeated spaces in field blocks

`field_block_spaces_preview` pins [49 raw printer frames](../zpl/tests/fixtures/field-block-spaces-zd621-v1/README.md),
48 pixel-exact. The rotated font-0 frame retains 2 underpaint and 24 overpaint
pixels; every one of its 40 ink regions exceeds 98.6% foreground IoU and has an
80% test floor. ZD621_203_DPI retains repeated ASCII separators inside and after
words and reproduces the measured terminal-line justification threshold.
Oversized leading runs consume one blank row; oversized internal runs do so
when the preceding line is full, using the next line's available width after
hanging indentation. Controls vary word lengths, run lengths, block widths,
indentation, paragraph breaks, all alignments and all rotations.
SPECIFICATION keeps normalized whitespace; `block_preserves_extra_spaces`
independently selects the native behavior. The original conformance case and
all formerly diagnostic narrow-block frames now match without allowances.

## Default font and per-field size override

The `field-defaults` case now matches the printer exactly, improving from
76.5% foreground IoU. Native font-0 strikes at 24x16 and 48x32 replace scaled
approximations. [31 raw frames](../zpl/tests/fixtures/font0-defaults-zd621-v1/README.md)
pin this improvement: 27 are exact, and the 48 fields in four rotated FO/FT
holdouts each exceed 98.5% IoU with an 80% regression floor. Exact residuals
and hashes are pinned; offline extraction reproduces both font assets.

## Minimum scalable size and fractional FO baselines

The five original font-0 cases requesting 1x1, 2x2, 7x0, 32x1 and 1x32 now
match exactly. `font0_minimum_dimensions` reproduces the printer's independent
10-dot minimum after default/zero resolution; SPECIFICATION rejects values
below the documented scalable range. `font0_fo_floor_baseline` independently
selects the measured whole-dot baseline for normal horizontal font-0 FO text.
The latter fixes one-dot R/I offsets at fractional baseline heights.

[50 raw native frames](../zpl/tests/fixtures/font0-minimum-zd621-v1/README.md)
include four measurement atlases and 46 renderer comparisons. Forty-four
renderer frames are exact; all 64 text fields in the two remaining holdouts
exceed 90% IoU with an 80% floor and pinned underpaint/overpaint counts.
Offline extraction reproduces the 10x10, 10x32 and 32x10 ASCII strikes.

## Remaining sampled font-0 dimensions

Nine dimension cases formerly below 80% now match exactly: square 15, 17, 31,
33, 63 and 65 dots, plus 64x16, 16x64 and 96x96. Captured ASCII strikes replace
scaled approximations. [127 native frames](../zpl/tests/fixtures/font0-dimensions-zd621-v1/README.md)
include 126 exact sampling/verification/conformance comparisons and a 72-field
origin/rotation atlas whose fields all exceed 97.1% IoU. Tests pin residuals
and hashes and independently regenerate all nine assets. This does not close
remaining QR, unsupported encoding/font, or preview-width coverage gaps.

The aligned barcode conformance suite (`barcode_aligned_preview`) replaces
812-dot requests with fresh 832-dot native captures. Of 59 rendered frames,
58 are pixel exact; the remaining QR mask difference is pinned at 464
underpaint / 496 overpaint pixels and remains an open accuracy gap. A sixtieth
capture, six-digit BR UPC-E, is a blank invalid-input control and is not counted
as successful barcode rendering. See the fixture README for provenance.

`long_field_preview` requires exact pixels for the original 3072-byte text
field and thirteen native font-0 16x10 sampling/verification frames. Active-edge
scan conversion removes the false work-limit rejection; the captured strike
removes the scaled-font mismatch. The scan-work cap remains enforced.

`resident_tuv_preview` covers native T/U/V resident faces with 43 raw printer
frames. The three original unsupported-font cases and all 39 sampling and
verification frames are exact. Each of the 24 separate rotated origin fields
exceeds 97.65% foreground IoU, with pinned residuals and an 80% floor.

`resident_s_preview` separates resident AS from GS and pins 32 native frames.
The original S case, all 26 sampling/verification pages, and mixed AS/GS/CFS
label are exact. All 72 origin/block/multiline fields exceed 97.10% foreground
IoU with an enforced 80% floor. `font_s_block_metrics` selects the measured
printer block ascents and pitches; specification behavior remains selectable.

## Unicode conformance and zero-advance formatting

`unicode_conformance_preview` pins 18 additional unmodified native frames at
zero underpaint and zero overpaint. Seventeen contain ink; the blank CJK frame
records missing-character behavior and does not count as CJK text coverage.
The new Font 0 40x24 supplements cover the original Latin, Greek, Cyrillic,
combining-accent, supplementary-character, and control-character cases.
`zpl-font-extract` reproduces both assets from the native pages and independently
composes the verification frames, including the decomposed-accent source.

CI28 text uses canonical NFC composition; barcode bytes remain unchanged.
Zero advances are accepted only after the existing sentinel probe validation.
Plain text and bounded blocks suppress soft hyphen and zero-width space.
The ZD621's field-block departure (visible soft hyphen, zero-width space treated
as ordinary space) is controlled by `block_utf8_formatting_visible`, enabled
only in the printer profile. See the
[fixture evidence](../zpl/tests/fixtures/unicode-conformance-zd621-v1/README.md)
for exact source/PNG hashes, standards, and the historical failed literal
composition retained in capture metadata.

The recovered resident TTF also provides a useful independent font reference.
[Font 0 TTF comparison](font0-ttf.md) records the offline method and measured
remaining differences; exact captured strikes remain the renderer's source.

## Single-byte code pages

`code_pages_preview` pins seven native frames at zero underpaint/overpaint:
six original ASCII cases and a new twelve-field non-ASCII comparison. CI31 and
CI33–CI36 now decode Windows-1250/1251/1253/1254/1255; CI27 also decodes its
Windows-1252 C1 mappings, including euro. Both profiles use these documented
mappings. Barcode bytes remain unchanged and code-page changes between fields
are tested. [Native evidence and limits](../zpl/tests/fixtures/code-pages-zd621-v1/README.md)
include paired UTF-8 controls for Latin, Cyrillic, Greek, and Hebrew text.

## Legacy character-image remapping

`character_remap_preview` pins four pixel-exact native frames. CI0/CI13 mappings
are retained separately across fields and encoding changes, apply once, and
use the last pair for repeated destinations. CI27/CI28 ignore remapping pairs,
as documented. Font lookup applies mappings to text and barcode captions;
encoded bars retain their original data. Native Code 128 and UPC captions are
covered, including both resident A and OCR-B retail sizes.

The documented euro image and printer-only space remapping have independent
controls. `remap_space` enables the ZD621 departure from the guide's space
restriction; SPECIFICATION disables it. See the
[capture evidence and limits](../zpl/tests/fixtures/character-remap-zd621-v1/README.md).
The controls explicitly initialize and restore mappings because native preview
requests retain these tables. A separate off-label UPC-caption placement gap
was observed while testing and remains open.
