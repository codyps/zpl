# Font reconstruction accuracy examination

The next useful work is improving the feature representation and the fitting
objective. More observations helped the [expanded fit](expanded-font-sampling.md),
but the remaining errors expose constraints that a larger sample set cannot
remove. This examination keeps the six-glyph research scope (`H O S g j @`),
original TrueType engine, ZD621 D7J211001302 at 203 DPI, and V93.21.33Z firmware.
Production font selection and its captured strikes are unchanged.

The [structured-hint follow-up](#structured-hint-follow-up) below implements
those two improvements. Paired printer previews demonstrate a small-size gain
from 83.478% to 85.153% IoU on 240 fresh cases spanning small and medium sizes.
One previously exact historical case regresses, so this remains a research font.

## Where the error is

The expanded generated font matches the printer's rendering of those same
font bytes at 99.5367% foreground IoU overall, including its state witnesses.
Reconstructing Font 0 itself is substantially harder. On the preceding campaign's
96 cases with maximum requested dimension at most 32 dots:

| Glyph | Local reconstruction vs Font 0 | Printer reconstruction vs Font 0 | Same-font local vs printer |
| --- | ---: | ---: | ---: |
| H | 99.256% | 98.855% | 99.594% |
| O | 88.867% | 87.427% | 98.142% |
| S | 82.092% | 82.236% | 97.988% |
| g | 86.533% | 86.653% | 99.555% |
| j | 94.148% | 94.522% | 99.358% |
| @ | 78.728% | 78.542% | 94.151% |

These are 16 cases per glyph, mostly stretches and rotations. They do not imply
that H is solved everywhere: its earlier small-size holdouts aggregate to
89.453% IoU. Earlier small holdouts also put O at 77.964%. The full breakdowns,
including source hashes, are in
[diagnostics.json](../zpl-font-extract/tests/fixtures/accuracy-examination-20261004/diagnostics.json).

Most reconstruction error therefore remains in the inferred outlines and hints.
Renderer calibration still matters for @: it accounts for 113 of the 184
same-font disagreement pixels in the 96 small cases. That observation does not
identify whether its cause is hint execution, curve subdivision, or dropout
handling. It is a reason for targeted constructed-curve probes, rather than
fitting outlines to compensate for an unmeasured renderer discrepancy.

## A concrete feature omission

The current automatic feature detector selects contour-wide extrema and long
axis-parallel lines. It misses local extrema inside a contour. S consequently
has **no detected horizontal stems**, despite having top and bottom strokes
whose thickness should be controllable. Its inner Y extrema at 208 and 1323
design units are absent; pairing them with -32 and 1563 produces two 240-unit
strokes.

[`refine_font_features.py`](../zpl-font-extract/scripts/refine_font_features.py)
tests an additive fix. It preserves existing groups and instructions, adds
explicit on-curve local extrema, and permits new links only when nine samples
through the proposed stroke lie in ink. A counter test rejects links across a
hole. The starting font remains byte-identical; only subsequent hint search
changes its output. This introduces no new outline points, per-size deltas or
bitmap strikes.

The experiment runs two complete one-edit neighborhood sweeps. It improves S
and @ on the previously reserved internal check. Its S improvement selects a
new X-axis terminal link; the newly available Y-axis stem links are not selected
in these two sweeps. Detecting a feature is necessary to make it searchable,
but does not prove that the current search or instruction language uses it well.

There is also an objective mismatch. For g on that check's small cases, mean
case error improves slightly, from 18.908% to 18.861%, while pooled foreground
IoU falls from 85.375% to 83.315%. The objective can exchange errors between
different sizes of the same glyph in a way the release gate rejects.

## Controlled experiments

[`examine_font_accuracy.py`](../zpl-font-extract/scripts/examine_font_accuracy.py)
preserves the expanded fit's 894 search cases and 426 internal-check cases.
It tests removing hints, retaining one axis, restoring seed geometry or hints,
lifting the hint cutoff, two exhaustive hint-neighborhood sweeps, and six more
geometry sweeps. Geometry keeps the original topology and a 12-unit bound from
the automatic seed. Every accepted proposal is scored with the exact renderer;
every large training case retains its seed nonregression constraint.

All variants freeze before the old internal check is scored. That check was
already used in earlier work and is explicitly reported as an audit, not new
validation. The feature experiment uses the same split and starts from the same
expanded fit, making comparison with the two-sweep original graph meaningful.

Removing all hints increases the search objective from 0.080538 to 0.111278.
Keeping only X-axis hints scores 0.086080; keeping only Y-axis hints scores
0.108284. X-axis hints provide most of the existing gain. Lifting the common
90-ppem hint cutoff to 4096 regresses 25 individual large training cases and is
not an admissible candidate. Removing that safeguard wholesale is not a fix.

The original-graph hint search scored 0.078346, the extra geometry search
0.079549, and their combination 0.077516. The geometry arm accepted 28 further
coordinate moves. The feature-graph arm scored 0.077045. These are training
objectives, not estimates of new-size accuracy. The frozen states, actual TTFs,
complete page counts, search settings and input hashes are retained in
[the experiment artifacts](../zpl-font-extract/tests/fixtures/accuracy-examination-20261004/).

## Fresh audit and printer replay

Before reading new pixels, the
[audit plan](../zpl-font-extract/tests/fixtures/accuracy-examination-20261004/audit-plan.json)
pinned all candidate artifacts and seven comparison arms. The three new
[capture campaigns](../zpl-font-extract/tests/fixtures/accuracy-validation-20261004/)
contain 114 cases across 19 previously unused configurations: 72 small cases,
24 medium stretched cases, and 18 large cases at 176, 272 and 432 dots.
Candidates froze before capture started; repeated controls matched.
No model was changed or reselected using these pixels.

Foreground IoU against the fresh resident Font 0 captures:

| Candidate | Up to 32 | 33–64 | 65–90 | Large |
| --- | ---: | ---: | ---: | ---: |
| Expanded-fit baseline | 83.879% | 92.996% | 95.262% | 97.865% |
| No hints | 82.791% | 92.440% | 93.659% | 97.865% |
| Unlimited hint range | 83.879% | 92.996% | 95.262% | 98.025% |
| Deeper original-graph hints | 83.843% | 92.078% | 94.870% | 97.865% |
| Extra geometry refinement | 83.887% | 92.820% | 95.288% | 97.915% |
| Combined refinements | 83.850% | 91.906% | 94.896% | 97.915% |
| Extended feature graph | 84.061% | 92.094% | 94.861% | 97.865% |

**Every alternative fails the native-page nonregression gate against the
expanded-fit baseline.** Even the improved large aggregate for unlimited hints
conceals a page regression. More search is not demonstrated to improve
generalization. The richer graph helps small S (80.203% to 81.046%) but is not
sufficient by itself: medium sizes regress, and small-case mean error worsens
despite its slight pooled IoU gain. Full results are in
[fresh-evaluation.json](../zpl-font-extract/tests/fixtures/accuracy-examination-20261004/fresh-evaluation.json).
[`evaluate_font_experiment.py`](../zpl-font-extract/scripts/evaluate_font_experiment.py)
checks the frozen hashes, capture timestamps, excluded fitting queries, source
provenance and full-canvas accounting before reporting those comparisons.

The feature-graph font was also uploaded and
[replayed on the printer](../zpl-font-extract/tests/fixtures/accuracy-feature-replay-20261004/)
for all 96 fresh small/medium cases plus two state witnesses. The font was
6,524 bytes, its identity and instruction witnesses passed, resident controls
matched, and removal was confirmed. Same-font local/printer IoU is 99.5494%:
41 underpaint and 90 overpaint pixels over a 29,072-pixel union. The three
native page residuals are pinned in the Rust TrueType regression test.

The printer's feature-font output compared directly with its Font 0 output
scores **83.805%** for the 72 small cases, versus the local prediction of
84.061%. The predicted gain over the local baseline was only 0.181 percentage
points, smaller than that prediction discrepancy. There was no matching new
printer replay of the baseline, so this does not establish a measured
printer-to-printer improvement or regression against that baseline.
Medium-size printer results are 91.964% and 94.876% for the two bands, close to
the local predictions. The full direct comparison is in
[printer-versus-resident.json](../zpl-font-extract/tests/fixtures/accuracy-feature-replay-20261004/printer-versus-resident.json).

This examination used 20 resident-font previews and five generated-font
previews, without physical printing. It leaves the expanded fit and all
production behavior unchanged. Validation passes 50 Python tests, including
independent FreeType instruction execution for all eleven experimental fonts,
and 17 Rust TrueType, printer-accuracy and conformance tests.

## Priorities identified by the first examination

1. **Complete the feature graph and its constraints.** Retain local curve
   extrema, distinguish opposing stroke boundaries from counters, and search
   related changes together. Add counter-width constraints and relationships
   between height zones and inner curves. Currently links are restricted to
   detected ink stems, so some useful relationships cannot be expressed.
2. **Make the objective reflect the accuracy contract.** Balance glyph, size
   band and transformation; track pooled IoU alongside mean per-case error.
   Include explicit guards for previously exact cases and unacceptable glyph
   regressions. Keep final native-page gates independent and unchanged.
3. **Improve the outline model before expanding coordinate search.** The
   632-point seed includes a 382-point polygon for g. Small moves with fixed
   point count cannot turn those staircase fragments into a compact smooth
   curve. Fit a shared quadratic model against multiple large sizes, with
   tangent, curvature, winding and counter constraints, then rebuild its hint
   graph. Treat this as a separate model experiment: its benefit is not proved
   by the present coordinate-only ablation.
4. **Calibrate the remaining renderer discrepancy with diagnostic fonts.**
   Prioritize @-like thin curved strokes, opposing curves and rotated dropouts.
   Compare hinted and unhinted versions of identical constructed outlines on
   the printer. Derive shared raster rules before allowing the reconstruction
   optimizer to compensate for those differences.

TrueType separates scaling, instruction execution and scan conversion; the
experiments retain that separation. See the OpenType
[rendering sequence and scan-converter rules](https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01)
and the [instruction definitions](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions)
for SCFS, GC, RCVT, ROUND and IUP used by the original compiler.

## Structured-hint follow-up

The better candidate combines a richer, bounded hint language with an objective
that protects individual glyph/size groups. Changing either alone did not pass
all fresh page gates. The experiment keeps the expanded fit's 632 outline points,
shared width/zone parameters, 90-ppem outer guard, 894 search cases and 426 old
internal-check cases fixed. No fresh validation pixel participates in fitting.

### Relationships instead of independent edge decisions

[`structured_font_hints.py`](../zpl-font-extract/scripts/structured_font_hints.py)
adds these choices to the existing anchor, zone, stem-link and centered-stem
operations:

- **Stroke and counter relationships.** Distinguish ink thickness from white
  gaps using overlapping feature supports and interior winding samples. A
  counter can retain its scaled distance from a fitted edge, round that distance,
  or enforce a one-pixel minimum. It does not inherit a shared ink-stem width.
  Coordinated stroke–gap–stroke edits allow several dependent edges to move
  together.
- **Interpolation between fitted anchors.** Place an inner feature at its
  original relative position between two already fitted features, with optional
  final rounding. This lets inner curves or crossbars follow outer dimensions
  without independently rounding every edge. References must bracket the
  feature and form an acyclic dependency graph.
- **One coarse small-size alternative per glyph axis.** Choose a threshold from
  16, 24 or 32 projected ppem. The alternate program applies below that threshold;
  the ordinary program applies above it. Width and height can choose independently,
  which matters for stretched glyphs. These are calibrated ppem thresholds,
  not requested ZPL dot sizes. The existing outer guard still disables both axes
  when either exceeds 90 ppem.

The compiler emits ordinary TrueType `IP`, `SRP1`, `SRP2`, `GC`, `SCFS`, `IUP`
and projected-axis `MPPEM` branches. See the OpenType
[IP, reference-point and MPPEM instruction definitions](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions).
There are no bitmap strikes, per-size coordinate corrections or DELTA
instructions. All previous frozen fonts still compile byte-for-byte identically.

The selected program uses seven coarse branches. Examples include counter-relative
placement in O, interpolation of @'s inner features, and a rounded interpolated
H crossbar below its Y threshold. Its TTF grows from 6,312 to 7,928 bytes.
The compiler may add derived distances to the CVT table, but the fitted shared
widths, zones and cut-in do not change.

### Objective and hard constraints

[`fit_structured_hints.py`](../zpl-font-extract/scripts/fit_structured_hints.py)
uses foreground error `e = 1 - IoU`. Each glyph × size-band bucket combines the
mean case error and pooled pixel error with equal weights:

```text
bucket_loss = 0.5 × mean(case_error) + 0.5 × sum(xor_pixels) / sum(union_pixels)

objective = mean(bucket_loss)
          + 0.25 × mean(worst quarter of bucket_losses)
          + 2 × mean(max(0, case_error - baseline_case_error))
          + 0.00005 × description_length_edits
```

The size bands end at 16, 24, 32, 48, 64, 90, 128 and 4096 requested dots, using
the larger requested dimension. Cases with both dimensions at least 128 dots
have their own large-outline bucket. Each populated glyph/size bucket receives
equal weight. The final term charges for changed feature nodes, alternate
programs and their dependencies; it discourages added structure without gain.

A proposal is also rejected if any of these training conditions fail:

- Every previously exact case remains exact, and every large case is
  nonregressing.
- No case worsens by more than six percentage points of foreground error.
- Both mean and pooled error are nonregressing in every glyph × size bucket.
- Both are also nonregressing in every glyph × transformation class: square,
  wide, tall or rotated.

Pooled error closes the earlier loophole where improving the average case
could worsen foreground overlap. The tail and regression terms discourage
concentrating the remaining error in one difficult glyph or trading away
individual cases. These are search constraints, not guarantees about unseen
sizes; the historical H regression below demonstrates that distinction.

### Controlled comparison

Four predeclared arms cross the original/structured hint language with the
legacy/new objective. Each starts from the same baseline and receives two
deterministic sweeps, capped at 512 deterministically sampled neighbors per
glyph axis. The new objective with structured hints was designated before any
audit. There is no selection or refitting using the audit results.

The search evaluates 6,855 distinct font/query batches. A cache keyed by the
active axis programs reduces repeated work; whole-font execution subsequently
reproduces all cached search counts exactly for every arm. The old internal
check is evaluated only after all fonts freeze. The structured candidate's new
objective falls from 0.123228 to 0.111029, a 9.90% reduction. Objective magnitudes
are not comparable between the legacy and new definitions.

The [frozen fit and audit plan](../zpl-font-extract/tests/fixtures/structured-hints-20261004/)
pin font bytes, candidate states, engine/source hashes, selection rules and
capture manifests. After freezing, four
[new resident campaigns](../zpl-font-extract/tests/fixtures/structured-hints-validation-20261004/)
collect 240 cases across 40 previously unused configurations. They cover both
sides of the coarse thresholds, wide/tall glyphs and R/I/B rotations. The
following values are local predictions against those resident captures:

| Hint structure / objective | Up to 32 dots, 120 cases | 33–64 dots, 120 cases | All four native page gates |
| --- | ---: | ---: | --- |
| Unchanged baseline | 83.779% | 87.787% | Pass |
| Original / legacy | 83.658% | 87.671% | Fail |
| Original / new | 83.779% | 87.787% | Pass, no accepted change |
| Structured / legacy | 84.628% | 88.061% | Fail |
| Structured / new | **85.524%** | **88.128%** | **Pass** |

Each page must preserve pooled foreground IoU and its count of exact cases;
this gate is unchanged. The selected candidate improves fresh pooled IoU for
each of the six glyphs. Exact cases increase from 25 to 32, with no formerly
exact fresh case lost. On the previously examined internal check, small-size
IoU increases from 85.229% to 86.682%; this is an audit of old data, not another
independent validation set. Full fresh results are in
[fresh-evaluation.json](../zpl-font-extract/tests/fixtures/structured-hints-20261004/fresh-evaluation.json).

### Paired printer evidence and remaining regression

Both the unchanged baseline and designated candidate were uploaded to the same
ZD621 and previewed at all 240 fresh configurations. Their output is compared
directly with the same resident Font 0 PNGs on the original native canvases:

| Requested size range | Baseline font on printer | Structured font on printer |
| --- | ---: | ---: |
| Up to 32 dots | 83.478% | **85.153%** |
| 33–64 dots | 87.654% | **88.011%** |
| All 240 cases | 86.462% | **87.198%** |
| Exact cases | 22 | **27** |

The candidate removes 350 disagreement pixels (6,102 to 5,752), a 5.74%
reduction. All four page gates pass; no exact baseline case is lost. This
paired measurement establishes a printer-rendered gain, independently of the
local predictor's residual error. The
[paired report](../zpl-font-extract/tests/fixtures/structured-hints-20261004/paired-printer-evaluation.json)
links both source reports by hash and records every page and size stratum.

On these stressed configurations, local/printer agreement for identical generated
font bytes is 98.597% for the baseline and 98.583% for the candidate. Those
same-font figures measure a different error from reconstruction against Font 0.
They also use different cases from the earlier 99.5% measurements above.

The candidate passes 14 of 15 historical campaign gates. The exception is
`small-font-validation-20261003`: its unseen-size page improves from 87.348%
to 87.412% IoU but loses one exact case. **H at 34×34 dots in N orientation now
has 12 disagreement pixels.** That regression is retained in the
[historical report](../zpl-font-extract/tests/fixtures/structured-hints-20261004/historical-evaluation.json),
and no hint was changed after observing it. Large historical output is unchanged.
Resolving that transition with a general constraint and new independent cases
is necessary before promoting this candidate. Recovering smooth outlines and
real spacing also remains necessary: the font still covers only `H O S g j @`
and uses placeholder advances.

The follow-up uses 16 resident and 14 generated-font previews, without physical
printing. Repeated controls and font identity/execution witnesses pass. Both
temporary RAM fonts were removed and absence verified. Captures identify
ZD621 D7J211001302, 203 DPI, firmware V93.21.33Z. Rust regressions pin all ten
new replay canvases, their hashes and exact underpaint/overpaint counts.
Production font selection and captured strikes remain unchanged.

### Reproduction and checks

Run `fit_structured_hints.py` with the frozen `expanded-optimization-20261004`
model, a new output directory, `--engine`, `--cache`, and `--captures` containing
all source/development campaigns pinned in that model's `inputs.json`. The
loader rejects missing, changed or duplicate inputs and preserves its original
search/check split. Do not pass validation campaigns into fitting.

To recompute the fresh local audit from its frozen plan:

```sh
uv run --offline --no-project --with freetype-py==2.5.1 --with pillow==12.3.0 \
  python zpl-font-extract/scripts/evaluate_font_experiment.py \
  zpl-font-extract/tests/fixtures/structured-hints-20261004/audit-plan.json \
  zpl-font-extract/tests/fixtures _font-work/structured-audit.json \
  --engine "$CARGO_TARGET_DIR/debug/ttf-examine" \
  --cache _font-work/structured-audit-cache
```

[`font_experiment_replay.py`](../zpl-font-extract/scripts/font_experiment_replay.py)
provides `prepare`, `compare` and `paired` commands. Preparation reexports a
frozen candidate with two witnesses using request metadata alone; it does not
read validation PNGs. `compare` verifies capture provenance, requests, controls,
cleanup, matching native canvases and complete foreground accounting. `paired`
requires the same printer, resident PNGs and corresponding cases. Capturing a
prepared replay remains a separate explicit operation through
`capture_font_probe.py`.

Validation passes **60 Python tests**, including independent FreeType execution
across branch boundaries, exact regeneration of all five fonts, metadata-only
replay preparation and rescoring both printer comparisons from their PNGs.
The **17 Rust TrueType, printer-accuracy and conformance tests** pass; the
native TrueType fixture test now covers 62 canvases and 3,228 fields.
