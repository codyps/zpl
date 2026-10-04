# Joint outline and hint-program optimization

[`optimize_font.py`](../zpl-font-extract/scripts/optimize_font.py) implements a
bounded inverse-rendering experiment. Starting with the automatic reconstruction,
it alternates changes to outline coordinates, shared CVT parameters, and individual
hint operations. It exports an actual TrueType font. Runtime rendering requires
neither captured strikes nor a table of corrections for each size.

This remains a six-glyph research model with placeholder advances. Production
font selection is unchanged. Matching captured pixels cannot uniquely identify
the original outlines or bytecode: multiple fonts can produce the same samples.
The objective is a small, consistent program that generalizes to unseen sizes.

The [expanded sampling experiment](expanded-font-sampling.md) follows this run
with substantially more development data and a new independently reserved test set.

## Objective and constraints

For each observation `q`, the exact calibrated renderer produces `R(θ, c, P, q)`:

- `θ`: integer TrueType design coordinates for endpoints and quadratic controls.
- `c`: shared stem widths, height zones and the CVT cut-in threshold.
- `P`: a discrete program over measured outline features.

The observation includes dimensions, rotation, the declared baseline origin and
the measured printer environment. Its error is `e(q) = 1 − foreground IoU`.
Let `E_large`, `E_square` and `E_transformed` be mean per-glyph errors within
those groups. The minimized objective is:

```text
mean(E_large, E_square, E_transformed)
+ 0.2 × max(E_large, E_square, E_transformed)
+ 0.00001 × mean hint-program complexity
+ 0.000001 × mean squared coordinate displacement from the seed
+ 0.0000001 × mean squared shared-parameter displacement from the seed
```

Equal group weighting prevents the many foreground pixels in large samples from
dominating small text. The maximum-group term penalizes the weakest group.
Program complexity counts an anchor or zone as one and a stem link or center as
two. The objective is distinct from the full-canvas, pixel-weighted IoU reported
in evaluation tables.

Every candidate must also leave **each large development sample** at least as
accurate as the seed. Outline changes preserve the seed's contour count, winding,
containment and absence of proper segment crossings. Straight edges, exact axis
tangents and semantic feature coordinates move together. Each coordinate stays
within 12 design units of the seed. Widths stay positive and ordered, within 32
units; zones remain ordered, within 16 units; the cut-in stays within eight
F26Dot6 units of its seed and within 0–64. The inferred 90-ppem hint operating
limit is fixed in this experiment; both axes must be within it.

The topology check works on flattened curves and detects proper crossings; it
is not an exact symbolic proof for all possible tangencies or coincident edges.
Search does not add or remove contours or control points, change advances, or
invent size-specific DELTA instructions.

## Search

[`joint_font_geometry.py`](../zpl-font-extract/scripts/joint_font_geometry.py)
builds signed Manhattan-distance fields from large development silhouettes.
Bilinear field samples and quadratic basis derivatives rank coordinate moves.
Both directions of semantic feature moves are also proposed, because an outline
change that helps a hint need not improve the smooth surrogate. Every proposal
is quantized and evaluated by the original TTF engine before acceptance. There
is no claim that binary rasterization or bytecode execution is differentiable.

[`joint_hint_program.py`](../zpl-font-extract/scripts/joint_hint_program.py)
provides a typed language: leave a feature untouched, anchor it, attach it to a
shared height zone, center a stem, or link its other edge. Links must follow an
observed stem pair and form an acyclic dependency graph. Grid, half-grid, round
down and round up states follow TrueType's signed-distance rounding semantics.
Programs compile to bounded RCVT/GC/ROUND/SCFS/IUP bytecode, with explicit
projection-axis and rounding state. See the
[OpenType instruction reference](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions)
and [quadratic outline representation](https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01).

The default run uses three alternating rounds:

1. For each glyph, choose the best legal coordinate move. The distance guide
   contributes 24 ranked proposals, supplemented by both directions of feature
   moves. Coordinate steps are 4, 2 and 1 design units.
2. Choose the best shared-parameter move, rescoring all affected glyphs. Steps
   are 8, 4 and 2 in the parameter's stored units.
3. Search per-feature hint programs with beam width two, depth two, and at most
   64 distinct neighbors per parent. A deterministic hash order varies which
   part of a larger neighborhood is visited. The beam retains an alternative
   whose first edit is worse if a second edit can improve it.

This is a bounded local search, not a global optimality guarantee. A shared
parameter with no active users has no useful move until the program search
starts using it. Subsequent rounds let geometry, parameters and programs respond
to each other's changes.

## Data separation and reproducibility

Fitting reads only development pages and requires the same source and small
development provenance as its automatic-reconstruction seed. Every third sorted
size/rotation configuration is withheld from the search, matching the seed's
internal split. At the end, the proposed whole font receives one atomic check:
neither the square nor transformed group mean may regress. A rejected proposal
is retained as `proposal.ttf`, while `font.ttf` reverts to the seed. There is no
per-glyph reselection using these pixels. This is an internal development check;
the seed already used that partition during its earlier acceptance decisions.

`inputs.json` seals the seed, source pages, engine, scripts, objective and search
settings. Content-addressed rendering caches are bounded by `--max-evaluations`.
An interrupted run can replay deterministic decisions with `--resume`, but any
input, engine, script or search-setting change rejects the resume. Completed
models are frozen and recompile byte-for-byte. The working cache is transient.

`initial.ttf` is the compiled seed with its existing hints; `geometry.ttf` is the
selected geometry without hints; `font.ttf` is the accepted model; `proposal.ttf`
preserves the search winner even if internal acceptance fails. These initializer
semantics differ from the older fitter's polygon-only `initial.ttf`.

## Running it

```sh
export CARGO_TARGET_DIR=/home/cody-ai/.cache/cargo-targets/zpl-truetype
cargo build --locked -p zpl-font-extract -p zpl-cmd --bins

uv run --offline --no-project --with freetype-py==2.5.1 --with pillow==12.3.0 \
  python zpl-font-extract/scripts/optimize_font.py \
  zpl-font-extract/tests/fixtures/reconstruction-20261003 \
  zpl-font-extract/tests/fixtures/font0-outline-20261003 \
  zpl-font-extract/tests/fixtures/small-font-20261003 \
  _font-work/joint --engine "$CARGO_TARGET_DIR/debug/ttf-examine"
```

Use the existing `reconstruct_font.py evaluate` and `prepare-preview` commands
with the resulting model directory. They recognize both model schemas, verify
frozen artifacts and preserve native canvases and original origins. Evaluation
reports both the accepted font and the proposal, with separate gate results.
To inspect a rejected proposal on the printer, use `prepare-preview --variant
proposal`; the manifest records that choice explicitly. Its default remains the
accepted font. Printer transport remains in the separate identity-checked capture
tool. See [automatic reconstruction](automatic-font-reconstruction.md) for those
commands.

Unit checks execute migrated and neighboring programs in FreeType's independent
interpreter, reject cyclic links, verify continuous gradients against finite
differences, enforce the large-sample constraint, and demonstrate escape from a
worse intermediate program. The final checks passed 37 Python tests, 17 Rust
TrueType/accuracy/conformance integration tests, targeted strict Clippy and
formatting checks. Existing accuracy baselines were preserved.

## Measured run: useful training gains, rejected generalization

The [frozen run](../zpl-font-extract/tests/fixtures/joint-optimization-20261003/)
searched 3,712 distinct font/query batches. It accepted 15 coordinate moves,
changed the shared horizontal stem width from 212 to 214 design units, and
changed the per-feature hint programs. It retained 632 outline points. The
6,032-byte proposal contains 24 anchors, seven links and one centered stem;
shared zones and the cut-in threshold did not change.

Training objective fell from **0.137859 to 0.101871**, a 26.10% reduction. Mean
per-case errors show where that improvement occurred:

| Development partition | Seed error | Proposal error |
| --- | ---: | ---: |
| Search: large | 1.246% | 1.121% |
| Search: small square | 17.667% | 12.784% |
| Search: transformed | 11.824% | 8.965% |
| Internal check: small square | 16.294% | **17.260%** |
| Internal check: transformed | 17.247% | 16.333% |

The internal square regression rejects the **whole proposal**. The exported
`font.ttf` therefore equals `initial.ttf`; `proposal.ttf` and its full state
remain available for inspection. No fitting or candidate reselection followed
the internal check or subsequent validation.

Full native-canvas foreground IoU gives the following external results. The seed
column is the previously accepted automatic font, including its hints:

| Validation campaign | Cases | Seed | Joint proposal | Current production |
| --- | ---: | ---: | ---: | ---: |
| Historical large, 320/448 dots | 12 | 98.293% | 98.168% | 81.931% |
| Historical small, first campaign | 90 | 82.520% | 81.589% | 86.167% |
| Historical small, second campaign | 90 | 83.884% | 84.093% | 79.091% |
| Previous reconstruction holdout | 90 | 89.649% | 87.670% | 81.907% |
| Fresh small and transformed | 84 | 90.043% | 89.836% | 82.266% |
| Fresh large, 288/416 dots | 12 | 98.076% | 98.111% | 81.851% |

Every campaign fails the proposal's page-level nonregression gate. Improvements
in aggregate IoU do not override a worse page or fewer exact cases. The fresh
small set has 11 exact cases for the seed and ten for the proposal; the fresh
large set has zero and one respectively. Full counts, hashes and per-glyph
residuals are in [historical-evaluation.json](../zpl-font-extract/tests/fixtures/joint-optimization-20261003/historical-evaluation.json)
and [evaluation.json](../zpl-font-extract/tests/fixtures/joint-optimization-20261003/evaluation.json).

Fresh configurations were declared before fitting completed, then captured
after freezing. They include square sizes 42/44/46/48/50/52, width/height pairs
10/11, 11/10, 12/13, 13/12, 15/17 and 17/15, 14 dots in I, and 31 dots in B.
Large cases use 288 and 416 dots. The first large campaign exposed clipping of
`g` and `j` at 416 dots: their 78-dot descenders exceeded the fixed 64-dot margin.
That campaign is retained as diagnostic-only and excluded from accuracy reports.
The planner now reserves larger margins in the native ZPL request; the
[corrected capture](../zpl-font-extract/tests/fixtures/joint-large-validation-v2-20261003/)
uses those original native canvases without adjusting captured images.

The frozen proposal was also uploaded as a 6,160-byte font including two state
witnesses. On the **same generated font**, local rendering matches the ZD621 at
**99.5509% IoU**: 38 underpaint and 55 overpaint pixels over a 20,709-pixel union.
The [printer replay](../zpl-font-extract/tests/fixtures/joint-replay-20261003/)
records ZD621 serial D7J211001302, 203 DPI, firmware V93.21.33Z. Rust tests pin all
three native canvas residuals and hashes. Repeated controls matched, hint execution
was verified, and removal of the temporary RAM font was confirmed. This work used
28 serialized previews including the diagnostic campaign and one temporary font
installation; it printed no physical labels.

The optimizer is functioning, but this objective and search space still overfit
the available development samples. The much smaller same-font renderer error
supports focusing next on reconstruction constraints and validation across
development folds, rather than adding more hint-program freedom. The result
does not justify replacing production strikes or claiming recovery of the
original Font 0 outlines and instructions.
