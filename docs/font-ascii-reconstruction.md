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

## Weak-case diagnosis and repair (2026-10-05)

The [native-coordinate diagnosis](../zpl-font-extract/tests/fixtures/repair-font-20261005/diagnosis.json)
separates outline error, hint placement, search failure, and interpreter error.
`diagnose_font_fit.py` reproduces it from the frozen development split, with
unhinted controls, exact foreground counts, bounds and centroids. It performs
no registration or per-size fitting. Examples explain why aggregate IoU hides
important failures:

- `(` at 11×11 has zero IoU: its ten ink pixels lie one column to the right.
  The same outline without hints scores 83.33%. `|` at 20×20 similarly has its
  entire one-dot stroke one column left. These are previously observed check
  cases, not fresh validation or training inputs to this repair.
- `<` at 17×17 has five missed and seventeen extra pixels, scoring 43.59%.
  Its diagonals are too thick. The existing feature graph controls the inner
  apex in X but omits it in Y: the shoulder detector stops at the union of
  anchors, even when an anchor exists on only one axis.
- Grid fitting creates plateaus. The old ±16/8/4 numerical search cannot cross
  some of them; a necessary first edit can also violate a regression guard.
  Identical error counts do not imply identical rasters or equivalent programs.

[`repair_font_search.py`](../zpl-font-extract/scripts/repair_font_search.py)
adds missing axis controls while preserving the initial font bytes, larger
numerical moves, and a bounded beam. It keeps an acceptable incumbent separately
from exploratory programs, including temporarily regressed programs that may
lead to coordinated repairs. Final acceptance still requires all historical
and incumbent exact-case, large-case, per-case-regret, size and transformation
guards. The target objective remains native per-case IoU. Search identity uses
the program rather than its error counts. Outlines, CVTs and all 95 advances
remain unchanged; this introduces no size-specific bitmap tables.

The [frozen experiment](../zpl-font-extract/tests/fixtures/repair-font-20261005/plan.json)
selects the ten worst training glyphs, then runs four rounds with a beam of four
and 128 sampled structural proposals per axis. Some accepted `g`, `[` and `;`
repairs require intermediate regressions. These are **minimum training IoUs
across all cases of each glyph**, not just improvements at a chosen size:

| Glyph | Previous | Repaired |
| --- | ---: | ---: |
| `m` | 53.26% | 69.92% |
| `[` | 47.17% | 56.52% |
| `;` | 50.00% | 60.00% |
| grave accent | 36.84% | 37.50% |

The original worst `[` case, 17×17, improves from 47.17% to 97.30%; another
case then becomes its minimum. Across the full font, training cases at ≥90%
increase from 2,691 to 2,695 of 3,974; the old check rises from 1,098 to 1,099
of 1,834. Its zero-IoU cases remain. The global training minimum only increases
from 36.84% to 37.14%.

### Alternatives and interpreter control

Two separate ablations test whether different proposals solve the remaining
barriers. Projection-guided search ranks row/column foreground distributions,
then evaluates paired axis changes with actual two-dimensional IoU and the
same guards. It improves `;` but leaves the other nine glyphs unchanged; it is
available through `--strategy projection`, not the default.

A geometry-derived diagonal primitive uses SFVTL[1]/SHPIX to move both edges
along the normal of the scaled stroke, including anisotropic sizes. It has
one bounded displacement over a coarse optical range. All 152 tested settings
for each of `<` and `>` fail to produce an acceptable improvement. This
`--strategy diagonal` experiment remains optional; no diagonal correction was
selected for the repaired font. The
[projection](../zpl-font-extract/tests/fixtures/repair-font-20261005/projection-ablation.json)
and [diagonal](../zpl-font-extract/tests/fixtures/repair-font-20261005/diagonal-ablation.json)
reports preserve the negative results and source hashes.

The diagonal experiment also exposed an actual interpreter defect: the line
vector instructions popped their two point operands in reverse order. The fix
covers SPVTL, SFVTL and SDPVTL, including separate zones and original/current
coordinates. The
[original vector control font](../zpl-font-extract/tests/fixtures/line-vector-20261005/manifest.json)
matches independent FreeType 2.13.2 outlines exactly on all 18 cases. On ten
native printer canvases, same-font disagreements fall from 389 to 35 pixels;
pooled IoU rises from 80.04% to 98.02%. The remaining `(under, over, union)` is
`(0, 35, 1772)`, retained exactly in Rust tests. This measures execution of a
known font, not reconstruction of Font 0. Both full repair fonts retain exactly
the same rasters and metrics under the corrected engine on all 5,808 development
queries, as recorded in
[engine-migration.json](../zpl-font-extract/tests/fixtures/repair-font-20261005/engine-migration.json).

### Fresh printer evidence and limits

After candidate freeze, the
[audit plan](../zpl-font-extract/tests/fixtures/repair-font-20261005/audit-plan.json)
reserves 536 previously unused cases: 16 configurations for the ten weakest
training glyphs and four for all 94 visible ASCII characters. The combined set
covers N/R/I/B and stretched sizes. Spacing is unchanged and was not remeasured.
Resident captures and both uploaded full-font previews retain native canvases,
repeated controls, identity/execution witnesses and verified RAM-font cleanup.
All evidence identifies ZD621 D7J211001302, 203 DPI, V93.21.33Z.

| Both fonts rendered by the printer against resident Font 0 | Previous | Repaired |
| --- | ---: | ---: |
| Cases at ≥90% | 212 / 536 | 220 / 536 |
| Mean IoU | 81.92% | 82.47% |
| Pooled IoU | 81.39% | 82.04% |
| Worst individual IoU | 28.00% | 31.03% |
| Disagreement pixels | 9,210 | 8,857 |

The [paired printer report](../zpl-font-extract/tests/fixtures/repair-font-20261005/paired-printer-evaluation.json)
fails two of six page gates. In particular, `;` at width 13, height 17 loses
exactness in R and I orientations, adding one missed and one extra pixel per
case. No changes were fitted after observing this audit. The font remains an
experimental candidate and is **not promoted to production**. The local audit
predicts the aggregate improvement but is not substituted for printer evidence.

The design needs more than a larger search budget. Missing axis controls and
non-greedy exploration help, but independent point adjustments and constant
stroke corrections still overfit scale transitions. The next model should
coordinate stroke centerlines, widths, counters and contour placement, impose
geometric validity across intervals, and fit outline and hint parameters
jointly. Projection loss can guide proposals; only native two-dimensional
foreground IoU should accept them. Existing observed checks must be labelled
as development data if incorporated into a later fit, followed by new holdouts.

Two preliminary control captures are retained as diagnostic-only: one referenced
the generator's old hard-coded font object; the other used a state-page name
that skipped the transport's witness check. Neither contributes positive accuracy
evidence. The corrected generator uses each probe's font object, and its test
requires `00-state` and byte-identical regeneration of the successful campaign.

Reproduce the repair with `repair_font_search.py FIXTURES SEED OUTPUT --engine
ENGINE --glyphs 10 --rounds 4 --beam 4 --structural-budget 128`. The seed is
`ascii-reconstruction-20261004`. If the executable differs from the seed's
pinned engine, supply `--previous-engine`: migration is accepted only after
both seed fonts reproduce every development raster and metric exactly.
`evaluate_font_experiment.py` evaluates the frozen fresh audit; no fitting occurs.
The experiment's `source/` snapshot preserves the driver used for these numbers;
the current driver additionally exposes the later projection/diagonal ablations.

Validation passes 105 Python tests and 88 Rust tests across `zpl --lib`,
`truetype`, `printer_accuracy` and `conformance_preview`. The native TrueType
regression now pins 143 canvases and 7,606 fields. Rust formatting and diff
whitespace checks pass. The captured failures above remain part of the evidence;
passing regression tests do not mean that the candidate passes promotion gates.
