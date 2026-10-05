# Extending reconstruction to printable ASCII

The requested target covers every measured size/rotation case for **all 95
printable ASCII characters**. Space additionally requires the correct advance;
a blank glyph's perfect IoU does not establish correct spacing. The 90% target
has **not** been achieved across the character set. These are experimental fonts,
not a replacement for production captured strikes.

All new observations use ZD621 D7J211001302, 203 DPI, firmware V93.21.33Z. They
establish evidence for this environment only. Comparisons preserve native
canvases, field origins, and every foreground pixel.

## Predeclared data

[`ascii_font_probe.py`](../zpl-font-extract/scripts/ascii_font_probe.py) prepares
bounded campaigns before collecting pixels. The frozen
[plan](../zpl-font-extract/tests/fixtures/ascii-font-20261004/plan.json) retains the
request hashes of earlier experiments and excludes their configurations when
choosing new validation cases. `--previous-plan` reproduces that plan from the
pinned request metadata even after more captures are added to the repository.

The six research glyphs `H O S g j @` retain their existing data and partitions.
The other 88 visible characters receive:

- 176 large outline observations, at 192 and 384 dots.
- 4,312 small/transformed observations: dense squares from 10 through 32 dots,
  additional squares through 96, eight stretched configurations, and three
  rotations at four sizes.
- A separate spacing campaign covers all 95 characters at six sizes, including
  space: 570 measured advances, with terminal-sentinel shape verification.

That is 5,058 development observations across 170 previews, including repeated
controls. The plan separately reserves 1,598 visible-glyph validation cases and
285 spacing observations. Validation capture follows candidate freeze. Repeated
controls, printer identity, request/PNG hashes and native dimensions are retained
with each capture. Failed captures are not automatically retried.

## Outlines and spacing

[`reconstruct_ascii_font.py`](../zpl-font-extract/scripts/reconstruct_ascii_font.py)
fits alternative polygons and quadratic contours, using both legacy and
calibrated scale initializers. Geometry selection first minimizes shortfalls
below 90%, then combines mean error with a small point-count cost. Without this
cost, a detailed contour can beat a compact curve by memorizing pixel steps in
the large source image. Signed-distance gradients propose subsequent coordinate
changes; actual raster counts decide acceptance, preserving each selected large
case. Independent glyph jobs can run concurrently, with deterministic proposals,
content-addressed rendering and checkpoints.

The initial 88-glyph outline experiment used 22,904 points and a 117,408-byte TTF.
The compact experiment uses 8,103 points and a 43,436-byte TTF. All 176 large
training cases remain above 90%; the compact font's lowest large training IoU is
91.30%. These measurements do not establish small-size or holdout accuracy.

[`ascii_font_metrics.py`](../zpl-font-extract/scripts/ascii_font_metrics.py)
measures the displacement of the final `|` in `|character|`, relative to `||`.
It checks the sentinel shape, initial sentinel and tile boundaries. One integer
`hmtx` advance per character explains **all 570 development observations exactly**
using the renderer's existing measured layout scale. Incompatible observations
would remain explicit residuals; the fitter emits no per-size advance table.
The independent spacing results below show why an exact development fit does
not establish the correct advance at every size.

## Hint fitting and font assembly

The bootstrap chooses whole-axis policies using only the declared training
partition. The new glyphs have 3,080 training cases and 1,408 internal-check cases;
internal checks are reported after font/state freeze. Small-size accuracy remains
substantially below the target at this initial stage.

[`fit_ascii_hints.py`](../zpl-font-extract/scripts/fit_ascii_hints.py) applies the
[per-case target objective](font-target-optimization.md) to each glyph with fixed
outlines and CVTs. Glyph workers retain exact-case, case-regret, large-case and
size/transformation guards. A deterministic sample covers the whole proposal
neighborhood rather than favoring its first feature groups. Whole-font execution
must reproduce every individual worker's training counts after merging.

[`merge_ascii_font.py`](../zpl-font-extract/scripts/merge_ascii_font.py) combines
frozen fits by remapping shared widths and height-zone references, including
nested and optical programs. The old six glyphs keep their outlines and hints;
the measured advances replace placeholders. Independent FreeType tests verify
that merging preserves outline execution.

## Printer execution evidence

The old probe font occupied `!` and `"` with diagnostic witnesses. Full-ASCII
fonts retain those characters and place the witnesses at U+E000/U+E001 instead.
The original generator now supports sparse BMP segments using OpenType
[`cmap` format 4](https://learn.microsoft.com/en-us/typography/opentype/spec/cmap#format-4-segment-mapping-to-delta-values).
Existing contiguous fonts still compile to identical bytes.

The constructed
[ASCII control font](../zpl-font-extract/tests/fixtures/ascii-cmap-20261004/manifest.json)
was uploaded and previewed on the printer. All three native canvases match the
local renderer exactly: state witnesses, 95 individual ASCII fields, and six
spacing/escaping strings. The temporary font's removal was verified. This is
same-font mapping/layout evidence, not a Font 0 reconstruction score.

A separate
[training replay](../zpl-font-extract/tests/fixtures/target-training-replay-20261004/manifest.json)
checks fractional hint placement and the new feature controls on 156 known
six-glyph cases. Its page residuals `(under, over, union)` are:

| Page | Residuals |
| --- | --- |
| State | `(0, 0, 52)` |
| 0 | `(19, 28, 3457)` |
| 1 | `(21, 32, 6035)` |
| 2 | `(26, 9, 4640)` |

In particular, narrow `g` at width 10 and heights 10, 12, 14, 16 and 18 differs
between local and printer rendering by 0, 0, 1, 1 and 0 pixels respectively.
The much larger error against resident Font 0 therefore belongs mainly to the
reconstructed hint model. This replay uses training cases and is explicitly
excluded from independent validation claims.

## Frozen fitting results

The [assembled candidates](../zpl-font-extract/tests/fixtures/ascii-reconstruction-20261004/report.json)
cover all 94 visible characters plus measured space. Assembly preserves the
individual fonts' raster output in all **5,808** development/internal-check
cases. Different stem CVT cut-ins are retained in the emitted instructions;
merging does not silently apply one component's threshold to the other.
The baseline TTF is 68,284 bytes; the fitted candidate is 118,896 bytes.

| Partition | Baseline cases at ≥90% | Fitted cases at ≥90% |
| --- | ---: | ---: |
| Original six glyphs: training | 670 / 894 | 767 / 894 |
| Other 88 glyphs: training | 1,468 / 3,080 | 1,924 / 3,080 |
| All visible glyphs: training | 2,138 / 3,974 | 2,691 / 3,974 |
| Original six glyphs: previous internal check | 305 / 426 | 359 / 426 |
| Other 88 glyphs: internal check | 640 / 1,408 | 739 / 1,408 |
| All visible glyphs: internal check | 945 / 1,834 | 1,098 / 1,834 |

For the 88 new glyphs, mean training IoU rises from 84.87% to 90.46%, but the
worst training case is only 36.84%. Their mean internal-check IoU rises from
83.54% to 86.42%, with a worst case of 0%. The broader experiment therefore
**fails the every-case 90% target** despite the improved averages. The old
internal checks have already been examined and are not fresh validation.

The six-glyph narrow/short joint-search extension did not beat the selected
six-glyph run on its training objective. Its remaining narrow `g` failure is not
resolved by simply widening the small-square program's applicability.

The hint fitter shares immutable geometry/CVT data between local proposals and
copies only the glyph programs being edited. On one captured `$` neighborhood,
this reduced generation time from 14.48 to 3.63 seconds while preserving the
proposal contents. These are development timings, not a cross-machine benchmark.

## Independent audit and printer replay

The full-ASCII [audit plan](../zpl-font-extract/tests/fixtures/ascii-reconstruction-20261004/audit-plan.json)
pins both candidate fonts, the engine, and the validation requests before capture.
The [results](../zpl-font-extract/tests/fixtures/ascii-reconstruction-20261004/validation.json)
retain every field and canvas count:

| Independent measurement | Baseline | Fitted |
| --- | ---: | ---: |
| Visible glyph cases at ≥90% | 815 / 1,598 | 859 / 1,598 |
| Mean glyph IoU | 88.03% | 88.67% |
| Worst glyph IoU | 0% | 44.83% |
| Spacing fields at ≥90% | 125 / 285 | 141 / 285 |
| Exactly predicted advances | 254 / 285 | 254 / 285 |

All 94 large holdouts pass. The fitted font passes 216/376 upright stretched
cases and 549/1,128 rotated cases. The worst is the grave accent at width 21,
height 35, upright. Its eight missed and eight extra pixels give 44.83% IoU.
Pooled glyph IoU is 96.42%, dominated by large glyphs; that does not satisfy
the per-case target. Spacing has its own failures: at size 112 the predicted
space advance is 34 dots, while the measured advance is 33.

Both full fonts were also uploaded and replayed on all 1,504 small/transformed
holdouts. Each replay has 25 native pages, including identity/instruction
witnesses. Repeated controls and removal of both temporary fonts were verified.
The upload budget is now 256 KiB; these witnessed fonts use 68,436 and 119,048
bytes. Each campaign retains the transport's 32-preview limit, including controls.

| Printer-rendered font versus resident Font 0 | Baseline | Fitted |
| --- | ---: | ---: |
| Cases at ≥90% | 721 / 1,504 | 744 / 1,504 |
| Mean IoU | 87.37% | 87.77% |
| Pooled IoU | 87.52% | 87.85% |

The [paired printer report](../zpl-font-extract/tests/fixtures/ascii-reconstruction-20261004/printer-paired.json)
passes its baseline page gate on only 13/24 glyph pages. It therefore rejects
promotion despite the aggregate improvement. Against the *same uploaded font*,
the local renderer's total `(under, over, union)` is `(619, 690, 309108)` for the
baseline and `(1655, 1600, 311348)` for the fitted font. Richer fractional hints
increase execution disagreement, which must be addressed alongside fitting error.
The exact per-page residuals are pinned in the Rust printer regressions.

## Spacing follow-up

The first spacing audit suggested that a weighted continuous estimate was
choosing an advance near the edge of an equally valid integer interval. An
optional `midpoint` representative instead minimizes the worst design-unit
error within that interval. It changes 56 advances and still explains all 570
original development measurements exactly. It does not use validation pixels
or emit size-specific metrics.

This idea was frozen and tested on **380 new spacing observations** at sizes
23, 53, 101 and 157, after excluding the earlier development and audit sizes.
The [follow-up results](../zpl-font-extract/tests/fixtures/ascii-spacing-followup-20261004/validation.json)
show 327/380 exact advances, compared with 325/380 for the original estimate.
Text fields at ≥90% fall from 202 to 201, and mean field IoU falls from 87.25%
to 87.08%. Both variants still predict a 47-dot space at size 157 instead of 46.
This experiment does **not** establish a better default; the default remains
the original weighted estimate. The midpoint font was evaluated locally against
new resident captures, not uploaded in the full-font printer replay above.

As a post-audit diagnostic, all 1,235 development and audit spacing observations
remain compatible with one integer advance per character. The evidence therefore
does not require size-specific advance tables. It points to insufficiently narrow
training intervals. Those combined observations are not used to refit either
frozen font and cannot serve as a fresh validation set in a subsequent fit.

## Reproduction and checks

The experimental [full candidate TTF](../zpl-font-extract/tests/fixtures/ascii-reconstruction-20261004/targeted.ttf)
and its baseline are generated from `candidates.json`. Tests rebuild both byte
for byte, check full ASCII coverage and space metrics, and verify that merging
retains the separate fits. Geometry, hint search, assembly and audit remain in
`zpl-font-extract`; production rendering does not select these fonts.

Use `reconstruct_ascii_font.py outlines` for geometry, its `fit` action for the
initial policies/advances, `fit_ascii_hints.py` for constrained refinement, and
`merge_ascii_font.py` for assembly with the original six-glyph fit. Each accepts
`--help`; frozen plans and source snapshots record inputs, query partitions,
engine hashes and search settings. Rendering caches are disposable and excluded
from the fixtures. `audit_ascii_font.py evaluate` replays either frozen audit
with its pinned engine and separate cache directory; it performs no fitting.

Validation for this change passes 92 Python tests and the Rust `truetype`,
`printer_accuracy` and `conformance_preview` targets (17 tests). The TrueType
native-canvas regression now accounts for 119 pages and 6,501 fields. These
checks preserve measured failures; passing tests do not imply meeting 90% IoU.
