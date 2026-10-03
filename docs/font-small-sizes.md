# Small-size font investigation

The small-size Font 0 reconstruction improves some unseen cases, but **does not
yet generalize reliably enough to replace the production bitmap fallback**.
The first validation set exposed a failure of simple point relationships; a
revised model improves a fresh reserved set while regressing the earlier set.
Both results are retained below. Production font selection is unchanged.

The handler also now scales the ZD621 control-value table (CVT) on the larger
requested axis and projects it with a 16.16 ratio. Original diagnostic fonts
validate this correction exactly. It does not change the Swiss small-size
scores in this campaign.

All observations below are from ZD621 D7J211001302, 203 DPI, firmware V93.21.33Z,
on 2026-10-03. Captures use explicit FT origins and native canvases. Comparisons
include every canvas pixel without alignment, resizing, cropping or padding.
IoU means foreground intersection/union, not white-background agreement.

## Font 0: original hint programs from captured geometry

The [large-outline pilot](swiss-font-rendering.md#font-0-pilot) supplied the six
reconstructed glyphs `H O S g j @`. Their 512-dot polygon outlines remain fixed.
The new [hint compiler](../zpl-font-extract/scripts/font0_hint_model.py) identifies
contour extrema and long axis-parallel edges, groups nearby points, and generates
original TrueType instructions to:

- Snap a point group to a grid or shared height zone.
- Position a group relative to another group, including separated stems.
- Center a stem while rounding its thickness.
- Use shared stem distances within a fixed cut-in and interpolate untouched
  outline points with IUP.

The [fitter](../zpl-font-extract/scripts/fit_font0_hints.py) selects these
relationships using development pixels only. It minimizes the sum of per-case
`1 - IoU`, with a secondary cost that prefers relative constraints to additional
absolute anchors. Single-group and paired-stem moves run for at most three
passes in the recorded experiment. The history's `constraints` field is this
weighted constraint cost, not a point count.

The common design-space stem values are 272 horizontally and 208 vertically;
the cut-in is 24/64 dot. These are reconstruction hypotheses, not recovered
original CVT entries. Height zones come from the captured geometry. There are
no per-size bitmaps, point-delta tables, or per-image translations in the model.
Programs execute afresh at each requested size. They follow the
[OpenType instruction semantics](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions)
for CVT access, conditional arithmetic, SCFS and IUP.

The [frozen model and font](../zpl-font-extract/tests/fixtures/font0-hints-20261003/)
contain six useful glyphs plus selection/execution witnesses. **Advances are
placeholders**: this experiment measures isolated geometry, not strings, wrapping,
kerning or character coverage. The font is a research fixture, not a usable
replacement resident font. No original Font 0 font bytes or programs were used.

## Results and selection history

[Initial small-size captures](../zpl-font-extract/tests/fixtures/small-font-20261003/)
include nine development sizes (10, 12, 14, 16, 18, 20, 24, 28, 32), nine initially
reserved sizes (11, 13, 15, 17, 19, 21, 23, 27, 31), and separate stretched and
rotated cases. Each group has 54 normal and 36 transformed glyph cases.

Naive extrema snapping did not improve the development aggregate. A first
fitted graph reached 88.21% on development normal cases, but only 83.76% on its
reserved normal cases and regressed on reserved transforms. Its model and results
are retained as `initial-model.json` and `initial-evaluation.json`.

Inspection of that failure motivated allowing relationships between separated
stems and preferring fewer absolute anchors. The revised fitter still reads only
the original development pages, but the first validation set **informed this
structural change and is no longer independent**. A fresh campaign was therefore
selected and captured before evaluating the revised model:

- Normal sizes: 22, 25, 26, 29, 30, 34, 36, 38, 40.
- Width/height pairs: 15/25, 25/15, 22/30, 30/22 in N; 19/19 in R; 25/25 in B.

The final table reports every group, including the adverse earlier result:

| Group | Cases | Current bitmap renderer | Unhinted outline | Revised hinted outline | Hinted under / over |
| --- | ---: | ---: | ---: | ---: | ---: |
| Development normal | 54 | 88.17% | 81.48% | 90.67% | 269 / 216 |
| Development transforms | 36 | 88.66% | 78.64% | 89.90% | 142 / 130 |
| Earlier validation normal, reused | 54 | 87.36% | 82.80% | **81.43%** | 492 / 518 |
| Earlier validation transforms, reused | 36 | 84.08% | 81.64% | **76.23%** | 374 / 362 |
| Fresh reserved normal | 54 | 80.16% | 86.45% | **88.19%** | 702 / 773 |
| Fresh reserved transforms | 36 | 75.96% | 80.00% | **80.20%** | 399 / 453 |

The revised model improves the fresh reserved aggregate from 79.09% for the
current renderer to 86.14%, but is worse on the earlier validation aggregate
(79.54% versus 86.17%). It also has fewer exact development cases than the bitmap
renderer (22 versus 48 of 90). The current exact strikes must therefore remain.
A favorable aggregate or a favorable choice of size range is not an acceptance
gate. Most of the fresh-set gain over bitmap scaling already comes from the
large-source outline; hints add a smaller improvement there.

The full [evaluation report](../zpl-font-extract/tests/fixtures/font0-hints-20261003/evaluation.json)
retains per-page and per-glyph underpaint, overpaint, union and exact counts, plus
the production comparison. The [fresh reserved captures](../zpl-font-extract/tests/fixtures/small-font-validation-20261003/)
remain separate from fitting inputs. The older sealed font-refinement pages were
not read.

## Checking the generated font on the printer

The frozen reconstructed font was uploaded as a temporary RAM object and selected
with `^A@`. A distinct geometry marker and a 13-dot instruction witness confirmed
font selection and hint execution. The printer rendered development and fresh
reserved fields using exactly the generated outlines and programs.

Local-versus-printer comparison of that **same generated font** has 49 differing
pixels over a 23,248-pixel union: **99.79% foreground IoU**. The directional totals
are 22 underpaint and 27 overpaint pixels. This is separate from comparison to
resident Font 0 above. It indicates that most of the remaining resident-font
error in this pilot is in the reconstructed geometry/hints, rather than failure
to execute the generated programs.

The original PNGs and requests, font/model hashes, dimensions, controls and
cleanup record are retained in the model fixture directory. Rust regressions
pin every same-font canvas's residuals and union counts, including rotations.

## CVT scaling correction

The new [axis probes](../zpl-font-extract/tests/fixtures/cvt-axis-20261003/) expose
integer and fractional CVT values as integer-width bars. The prior environment
scaled the CVT using Y and adjusted other directions from that value. The
measured model is:

```text
base = max(requested_x, requested_y)
scaled_cvt = round(design_value * effective_ppem(base) * 64 / units_per_em)
axis_ratio = round(effective_ppem(axis) / effective_ppem(base) * 65536) / 65536
projected_cvt = round(scaled_cvt * axis_ratio)
```

`effective_ppem` remains the [shared point-quantization rule](swiss-font-rendering.md#what-changed).
The change is confined to `Environment::Zd621V93`; the standard environment and
previous scan policies remain available. CVT initialization and WCVTF use the
same base. Vector projection uses the corresponding X/Y matrix ratios.

The correction removes all 1,086 differing pixels on the original five probe
canvases. A [fresh-name discriminator campaign](../zpl-font-extract/tests/fixtures/cvt-axis-validation-v2-20261003/)
also matches all five canvases exactly, including values that distinguish nearest
16.16 rounding from truncation. Across both campaigns there are 736 integer or
fractional CVT witnesses plus four state witnesses. Tests retain zero underpaint
and overpaint rather than applying an IoU tolerance.

An intervening campaign reused the previous RAM font name with new CVT contents.
Its output matched the preceding font's CVT values, despite directory-level
upload/deletion checks and passing geometry/SCFS controls. That
[capture is retained as diagnostic-only](../zpl-font-extract/tests/fixtures/cvt-axis-validation-20261003/observation.json),
and comparison tooling refuses to count it as accuracy evidence. Repeating with
a fresh name resolved the stale data. A geometry witness alone does not establish
that a revised font's CVT contents are active.

## Swiss small-size controls

Swiss ASCII remains at 98.78% foreground IoU across 475 development cases
(10, 12, 14, 18 and 24 dots), and 98.68% across 380 reserved cases (11, 13, 17 and
21 dots). The individual 10/11-dot pages are approximately 96%; tiny curves and
diagonals remain difficult. These values are unchanged by the CVT correction.
A perpendicular dropout pass was also tried and rejected because it increased
Swiss errors. No new scan-conversion mode is enabled.

## Reproduce and remaining work

```sh
export CARGO_TARGET_DIR=/home/cody-ai/.cache/cargo-targets/zpl-truetype
cargo build --locked -p zpl-font-extract -p zpl-cmd --bins
uv run --offline --no-project --with freetype-py==2.5.1 --with pillow==12.3.0 \
  python zpl-font-extract/scripts/evaluate_font0_hints.py \
  zpl-font-extract/tests/fixtures/font0-outline-20261003 \
  zpl-font-extract/tests/fixtures/small-font-20261003 \
  zpl-font-extract/tests/fixtures/small-font-validation-20261003 \
  zpl-font-extract/tests/fixtures/font0-hints-20261003 \
  --engine "$CARGO_TARGET_DIR/debug/ttf-examine" \
  --renderer "$CARGO_TARGET_DIR/debug/zpl-cmd" --output /tmp/font0-hints.json
# Optionally add --swiss /path/to/swiss.ttf for the known-font controls.
cargo test --locked -p zpl --test truetype --test printer_accuracy --test conformance_preview
uv run --offline --no-project --with freetype-py==2.5.1 --with pillow==12.3.0 \
  python -m unittest discover -s zpl-font-extract/scripts -p 'test_*.py'
```

`fit_font0_hints.py SOURCE DEVELOPMENT --engine BINARY --output /tmp/model.json
--passes 3` reproduces the relationship fit. `hinted_font_probe.py` creates a
reviewable upload campaign; tests never contact a printer. All 49 previews in
this follow-up were serialized. Four temporary font installations were removed
with absence checks, and all repeated resident controls matched. No physical
printing was requested.

Validation passed: 80 Rust library tests, eight TrueType integration tests, five
conformance-preview tests, four printer-accuracy tests, and 12 offline Python
tests. The evaluation report reproduces byte-for-byte. Rust formatting, strict
Clippy on the changed targets, and a `zpl-wasm` compile check for
`wasm32-unknown-unknown` also pass. Broader strict Clippy remains blocked by three
pre-existing constant-assertion diagnostics in `zpl/tests/profiles.rs`.

Further work should constrain stem relationships using independent geometry and
spacing measurements, expand beyond six glyphs, and test the smallest sizes and
strings. Additional unrestricted search over point relationships is not justified
by the current holdout results. None of the reconstructed fonts is embedded in
the production renderer.
