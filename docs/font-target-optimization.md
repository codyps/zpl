# Optimizing toward 90% foreground IoU per case

The reconstruction target is **at least 90% foreground IoU for every measured
case**, not 90% pooled overlap across differently sized glyphs. The target now includes [all printable ASCII](font-ascii-reconstruction.md),
including measured advances and space. The original `H O S g j @` experiment
remains a regression set on ZD621 D7J211001302, 203 DPI, firmware V93.21.33Z.
Its old artifacts retain placeholder advances; the ASCII assembly adds fitted
metrics. Production font selection and captured strikes are unchanged.

This continues the [structured-hint experiment](font-accuracy-examination.md#structured-hint-follow-up).
The constraint baseline is its frozen `structured_robust` font. The original
894 search cases and 426 previously examined internal-check cases retain their
roles. Warm starts are selected using search cases only; the old check is an
audit, not new validation.

## Target and constraints

[`fit_font_target.py`](../zpl-font-extract/scripts/fit_font_target.py) ranks
admissible candidates lexicographically. For case error `e = 1 - IoU`, define
`d = max(0, e - 0.1)`:

1. Minimize `max(d)`, the worst shortfall from the target.
2. Minimize mean squared deficits, the mean squared worst quarter, and squared
   deficits in each glyph/size bucket's mean and pooled error.
3. Minimize aggregate error and a small description-length/numeric-parameter
   cost.

Thus a better average or a shorter program cannot defeat a candidate meeting
the target for every case. The existing exact-case, large-case, six-percentage-
point case-regret, glyph/size and glyph/transformation constraints remain hard
requirements against the original baseline. The target is an acceptance
criterion, not a claim that a greedy search will find a feasible solution.

The fitter records both case counts above 90% and the worst IoU, as well as mean
and pooled IoU. Full reports retain underpaint, overpaint and union counts.
Measurements use the original native canvases and origins without alignment,
rescaling or padding.

## More useful hint controls

The compiler and feature graph add bounded controls shared across size ranges:

- Rounding phase, fractional post-rounding displacement, and a small design-unit
  anchor correction. These change rounding thresholds and fitted edge positions.
- Two nested coarse ranges per axis, with conditions based on X ppem, Y ppem,
  their minimum or their maximum. An optional independent-axis guard preserves
  small-axis hinting when the other dimension exceeds 90 ppem; large output
  still retains the original guard.
- One optional optical program overrides those normal ranges in a coarse region
  selected by X ppem, Y ppem, their minimum or their maximum. The default requires
  both dimensions to be small. Its fallback retains the complete existing hint
  program, including narrow/tall and wide/short behavior. Its coarse cutoffs
  include the measured ten-ppem minimum, shared by requests below ten dots.
- Additional curved-stroke relationships, including offset extrema and valid
  overlapping stem pairs previously missed by greedy pairing.
- Curve-shoulder controls, and separate controls for disconnected contour runs
  that happen to share a coordinate. Splitting retains original reference points
  and hint origins, and must preserve every initial training raster.
- A post-interpolation shoulder correction that leaves established stem/zone
  points fixed. Its optional optical-size factor is
  `clamp((end_ppem - measured_ppem) / (end_ppem - 10), 0, 1)`, with shared endings
  at 16, 24, 32 or 48 ppem. Inactive corrections do not touch the point.

The output is an actual TrueType font, using SCFS, IP, IUP, ROUND, MPPEM and
ordinary arithmetic from the
[OpenType instruction definitions](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions).
There are no per-size bitmap substitutions or exact-size delta tables. Existing
frozen fonts without these extensions still compile to identical bytes.
Outlines and shared CVT values stay fixed during this hint search.

Whole-axis templates, paired stem edits and coordinated shifts supplement
single-node descent. Sparse templates retain only one anchor, allowing tiny
glyphs to shed constraints that only become useful at larger sizes. Introducing
a new range includes a local edit in the same
proposal: an identical branch would only add complexity and could never be
accepted before it becomes useful. Numerical refinement uses progressively
smaller steps, visiting the worst glyphs first.

A bounded joint search also addresses the two worst training glyphs. It ranks
axis proposals from both the warm start and original baseline within shared
small-size regions and retains twelve distinct
per-case error patterns per axis, then checks their Cartesian product against
the full constrained objective. Ranking may consider a temporarily worse axis;
only an admissible combined program can be accepted. The selected override
preserves the normal branches outside its coarse region. The full training audit also
checks stretched cases and every previous exact case. These proposals improve
search coverage but do not constitute a global optimization guarantee.

## Execution and validation

Scores come from the original Rust renderer. The active-program cache includes
feature membership, hint origins, zone origins and the query, as well as the
active instructions. Optional screening rejects a proposal only after measuring
a hard per-case violation; every survivor receives the complete constrained
objective. Unchanged glyph scores can be reused because each proposal changes
one glyph. Final whole-font execution must reproduce every cached search count.

Independent FreeType tests exercise fractional placement, fading in either
axis, nested ranges, post-interpolation corrections, disconnected features and
cache behavior. They complement printer comparisons; they are not substitutes
for native printer evidence.

[`target_font_probe.py`](../zpl-font-extract/scripts/target_font_probe.py) reserves
384 cases across 64 previously unused size/rotation configurations using only
request metadata. It covers small, stretched, rotated and mixed small/large-axis
cases. Candidate fonts and the audit plan must freeze before these pixels are
captured. Both the baseline and candidate are replayed on the printer, separately
from comparison of the local renderer with the same uploaded font bytes.

A separate 72-case minimum-size control campaign verifies the new optical
program using nominal dimensions from one to ten dots. These share effective
scales and are reported separately from the 384-case fresh audit; they do not
count as 72 independent effective sizes.

To reproduce fitting, use the frozen expanded model, the structured experiment
as `seed`, a new output directory, `--engine`, `--cache`, and `--captures` listing
the source/development roots pinned in the expanded model's `inputs.json`.
`--warm-start` accepts the saved training-selected state. The fitter saves that
state, its source snapshot, query split, engine hash, candidates, TTFs and exact
counts. Validation captures must never be passed as fitting inputs.

The frozen [six-glyph run](../zpl-font-extract/tests/fixtures/target-hints-20261004/report.json)
finishes at 767/894 training cases above 90%, versus 670/894 for its constraint
baseline. Mean training IoU improves from 92.49% to 95.02%, while the worst case
improves from 33.33% to 43.75%. On the previously examined internal check it
reaches 359/426 cases above 90%, versus 305/426; mean IoU is 94.16% and worst IoU
is 54.55%. Every original training guard passes, and the full-font execution
matches cached counts. **The per-case target remains unmet.**

The broader [ASCII experiment](font-ascii-reconstruction.md) supersedes the
planned six-glyph-only fresh audit. It keeps the original partitions and reserves
new dimensions/rotations for all visible ASCII glyphs, plus spacing holdouts for
all printable characters. The unexecuted six-glyph audit and minimum-clamp plans
remain optional controls, not claimed measurements.
