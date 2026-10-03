# Automatic geometry and hint reconstruction

[`reconstruct_font.py`](../zpl-font-extract/scripts/reconstruct_font.py) now runs
the reconstruction experiment end to end: fit geometry, infer features, generate
hints, freeze a TrueType font, and evaluate it against separate captures. It also
prepares generated-font printer campaigns. Production font selection is unchanged.

The first completed run improves some reserved cases, but fails the overall
nonregression gate. This is a repeatable research pipeline, not a completed Font 0
replacement. It currently reconstructs isolated visible ASCII glyphs; advances
are placeholders, so strings, spacing and wrapping are not validated.

## Fitting and acceptance

The fitter reads only pages marked `development` from the supplied geometry and
small-size campaigns. It verifies request and PNG hashes, native dimensions,
printer model/firmware/DPI, repeated controls, unclipped source silhouettes, and
consistent glyph coverage. Validation PNGs are not fitting inputs.

The geometry stage starts from the largest upright silhouette for each glyph:

1. Trace oriented pixel-cell boundaries and retain the prior polygon initializer
   as the measured acceptance floor.
2. Lock contour extrema, long straight edges and sustained corners. Fit quadratic
   control points by least squares with monotone nearest-parameter refinement;
   subdivide spans whose residual exceeds the candidate tolerance.
3. Insert explicit quadratic extrema, quantize to the 2048-unit design grid, and
   reject winding/containment changes or proper segment crossings. Subdivision
   is bounded; failures remain in the candidate report.
4. Render each candidate at all development geometry sizes. Select by mean
   foreground error plus `0.00001 * point_count`, while rejecting any choice
   with worse mean pixel error than the polygon initializer.

The recorded tolerance candidates are 0.65, 0.9, 1.25 and 1.75 source dots.
Fitting uses the measured ZD621 outline scale, not an assumed nominal ppem.
The algorithm and numerical search bounds are shared across glyphs; no glyph
names, manually entered Font 0 stems, or per-size point corrections drive it.

The hint stage finds extrema and long axis-parallel edges on the selected
geometry. It pairs nearby opposing features only when the space between them is
ink, then clusters their widths and height positions into shared stems and zones.
Each axis selects from six bounded policies: untouched, grid, zone, and stem
anchoring at its lower edge, upper edge or center. Programs use original
RCVT/ROUND/SCFS/IUP bytecode. The representation and instructions follow the
[OpenType glyf table](https://learn.microsoft.com/en-us/typography/opentype/spec/glyf)
and [TrueType instruction reference](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions).

Every third sorted size/rotation configuration is withheld inside the development
set. Policy selection uses the remaining cases. Only the training winner is
checked on that internal acceptance partition; a regression reverts the glyph
to no hints rather than searching the partition for another winner. These are
internal development checks, not an independent final holdout.

A font-wide instruction limit is inferred as
`floor(sqrt(max_small_dimension * min_geometry_dimension))`. Both axes must be
within this limit to execute hints; the recorded run yields 90 ppem. Every
candidate must leave the previously accepted large development rasters exact.
This prevents small-size snapping from damaging the large outline model without
adding a bitmap or a separate correction table for each size.

Evaluation is a separate command. It checks that the model and all font artifacts
are frozen and that the engine matches the recorded binary. It rejects fitting
query overlap in validation mode and reports every full native canvas, with no
registration, padding, trimming or rescaling. Both IoU and exact-case counts must
not regress on **each page** to pass a gate; a favorable aggregate cannot hide
a failing page. Optional production comparisons apply the same gate. Evaluation
never modifies model selection or enables a production font.

## Results on ZD621 V93.21.33Z

The [frozen run](../zpl-font-extract/tests/fixtures/reconstruction-20261003/) uses
the six captured glyphs `H O S g j @`, 256/384/512-dot development geometry, and
the prior small development cases. It evaluates 229 distinct font/query batches
and reduces the outline from 2,208 to **632 points**. The fitter retains the
polygon for `g`, selects curves for the other glyphs, and rejects `@`'s proposed
hints on the internal acceptance partition.

All values below are foreground IoU at the original canvas origin:

| Validation set | Cases | Polygon initializer | Geometry only | Geometry + hints | Current production |
| --- | ---: | ---: | ---: | ---: | ---: |
| Historical large, 320/448 dots | 12 | 98.08% | **98.29%** | **98.29%** | 81.93% |
| Historical small, first campaign | 90 | 82.39% | 82.53% | 82.52% | **86.17%** |
| Historical small, second campaign | 90 | **84.79%** | 84.60% | 83.88% | 79.09% |
| New reserved campaign | 90 | 88.48% | 88.24% | **89.65%** | 81.91% |

Only the new campaign passes the page-level outline gate. The large aggregate
improves, but individual pages still regress. Thus the overall gate rejects
promotion. The automatic model also remains worse than the previous fitted
model on some historical sizes. Full per-page/per-glyph errors and exact counts
are retained in [evaluation.json](../zpl-font-extract/tests/fixtures/reconstruction-20261003/evaluation.json).

The new reserved campaign was planned before evaluation and its pixels were
excluded from all fitting. It contains square sizes 33, 35, 37, 39, 41, 43, 45,
47, 49; normal width/height pairs 11/12, 12/11, 14/15, 15/14; 11/11 in R; and
13/13 in B. All configurations are disjoint from both prior small campaigns.
The historical sets had already informed earlier research, so they are not
claimed as new independent evidence.

An earlier automatic candidate applied its hints at all sizes and reduced the
large reserved aggregate to 97.94%. Its model and report are retained as
`unbounded-hints-model.json` and `unbounded-hints-evaluation.json`. That failure
motivated the inferred instruction limit and the large-geometry preservation
check. The final model was frozen before its new reserved pixels were evaluated;
no fitting changes followed that evaluation.

The automatically generated 6,364-byte replay font was then uploaded under a
fresh temporary RAM name. The [same-font printer replay](../zpl-font-extract/tests/fixtures/reconstruction-replay-20261003/)
matches the local engine at **99.52% IoU**: 47 underpaint and 57 overpaint pixels
over a 21,674-pixel union. This measures execution of the generated font, separately
from resemblance to resident Font 0. Rust regressions pin the three native
canvas residuals and hashes. All repeated controls matched and removal of the
temporary font was confirmed. This follow-up used ten serialized previews in
total and one temporary font installation; no physical labels were printed.

## Commands and artifacts

Build the existing original renderer:

```sh
export CARGO_TARGET_DIR=/home/cody-ai/.cache/cargo-targets/zpl-truetype
cargo build --locked -p zpl-font-extract -p zpl-cmd --bins
```

Fit a fresh run (offline once the Python dependencies are cached):

```sh
uv run --offline --no-project --with freetype-py==2.5.1 --with pillow==12.3.0 \
  python zpl-font-extract/scripts/reconstruct_font.py fit \
  zpl-font-extract/tests/fixtures/font0-outline-20261003 \
  zpl-font-extract/tests/fixtures/small-font-20261003 \
  _font-work/recovered --engine "$CARGO_TARGET_DIR/debug/ttf-examine"
```

The output contains `inputs.json` (source, script and engine hashes), `model.json`
(geometry, inferred features, choices and rejected candidates), `font.ttf`,
`geometry.ttf`, `initial.ttf`, and `progress.json`. The `cache/` directory contains
content-addressed render results and is transient. Keep work directories under
`_font-work/`; intentionally versioned fixtures omit the cache.

The default budget is 600 distinct font/query batches; `--max-evaluations` accepts
1..10000. On interruption or budget exhaustion, rerun the same fit command with
`--resume` and, if necessary, a larger budget. Resume refuses changed fitting
inputs, scripts or engine, and completed runs are immutable. Each renderer
subprocess has a 120-second timeout. A deliberately exhausted one-batch run was
resumed and reproduced all three fonts, model and input records byte-for-byte.

Evaluate all retained sets, optionally comparing the production renderer:

```sh
uv run --offline --no-project --with freetype-py==2.5.1 --with pillow==12.3.0 \
  python zpl-font-extract/scripts/reconstruct_font.py evaluate _font-work/recovered \
  zpl-font-extract/tests/fixtures/font0-outline-20261003 \
  zpl-font-extract/tests/fixtures/small-font-20261003 \
  zpl-font-extract/tests/fixtures/small-font-validation-20261003 \
  zpl-font-extract/tests/fixtures/reconstruction-validation-20261003 \
  --engine "$CARGO_TARGET_DIR/debug/ttf-examine" \
  --renderer "$CARGO_TARGET_DIR/debug/zpl-cmd" --output /tmp/reconstruction.json
```

Use `--group development` for a clearly labeled development report. A different
engine binary requires a new fit run, rather than silently reusing the old fit's
calibration identity.

[`reconstruction_probe.py`](../zpl-font-extract/scripts/reconstruction_probe.py)
plans new resident-font campaigns from `--characters`, comma-separated `--sizes`,
and optional `--transforms` such as `11x12xN,13x13xB`. It supports separate
`development` and `validation` groups, validates the size/preview budget, and
uses a separate `capture` action for printer access. Fit accepts the resulting
captures for either geometry or small-size inputs when their size bands differ.

To prepare a generated-font replay, run `reconstruct_font.py prepare-preview
MODEL CAPTURES --output DIRECTORY --object R:ZPUNIQUE.TTF`. Preparation is offline;
use a fresh RAM name for each different font. The existing
[`capture_font_probe.py`](../zpl-font-extract/scripts/capture_font_probe.py) then
owns identity checks, serialized previews, state witnesses and verified cleanup.
Replay currently requires unused `!` and quotation-mark slots for its witnesses;
the fitting/export path itself supports the full visible ASCII range.

Validation passed: 24 offline Python tests, eight TrueType integration tests,
four printer-accuracy tests and five conformance-preview tests. Python checks
cover analytic curves, counter preservation, independent FreeType execution of
all hint policies, size limits, exclusion of validation pixels, cache budgets,
frozen-artifact tampering and byte-exact replay preparation. The affected Rust
test target passes strict Clippy; Rust and Python formatting also pass.

```sh
uv run --offline --no-project --with freetype-py==2.5.1 --with pillow==12.3.0 \
  python -m unittest discover -s zpl-font-extract/scripts -p 'test_*.py'
cargo test --locked -p zpl --test truetype --test printer_accuracy --test conformance_preview
```

## Limitations

Topology checks operate on flattened fitted curves and detect winding,
containment and proper crossings; they are not a proof about all coincident or
tangent boundaries. Geometry fitting is intentionally bounded and keeps the
polygon when a candidate fails. Shared stems and zones are inferred from only
the supplied glyphs, so broader character coverage can change those estimates.
The output still lacks measured advances, shaping, kerning and production font
integration. New validation failures should inform a new, separately evaluated
model version, not an automatic retuning loop against reserved pixels.
