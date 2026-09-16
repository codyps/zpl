# Font refinement plan

Prepared 2026-09-14. This is a proposed research and implementation sequence;
no new captures, font uploads, fitter changes, or renderer changes were performed
while preparing it. Implement in Rust without new external dependencies.

**Execution update (September 15):** the capture/calibration tools and pilot are
complete; calibration residuals blocked later fitting and integration. See the
[execution report](font-refinement-results.md) for evidence and pending stages.

## Recommendation

**Calibrate the rendering pipeline before fitting more font parameters.** Then
recover smooth geometry and spacing, followed by structured hint rules. Keep
original-font recovery as a separate, bounded investigation.

Our [stroke experiment](font-stroke-fitting.md) reduced its normalized training
loss by 40.5%, yet increased unseen normal-size pixel errors from 1,445 to 1,682.
Lowercase `l` matches the sampled normal sizes exactly, so some rounding behavior
is recoverable. The aggregate failure argues against adding more unconstrained
vertices or per-size corrections to the current model.

Two concrete implementation gaps deserve early attention:

- Both our fitter and output model use even-odd filling. TrueType's specified
  scan converter uses nonzero winding and includes pixels whose centers lie on
  contours. Its optional dropout rules can fill otherwise missing thin features.
  These distinctions are plausible contributors to boundary/rotation residuals,
  **not a demonstrated explanation of this printer**. [TrueType fundamentals](https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01)
- The renderer scales advances from its 32-dot strike. TrueType can adjust
  spacing during hinting, independently of the visible ink. A good isolated-glyph
  fit therefore does not establish correct text layout. [Horizontal metrics](https://learn.microsoft.com/en-us/typography/opentype/spec/hmtx),
  [phantom points](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructing_glyphs)

## 1. Define what we are recovering

Maintain three separate claims:

1. **Original font bytes:** requires obtaining and identifying the actual font
   file; pixel agreement cannot prove byte-for-byte recovery.
2. **Equivalent outline and hint model:** may use different control points and
   programs while reproducing the observed output.
3. **Exact renderer on a declared domain:** measured by zero mismatches across
   specified glyphs, sizes, transforms, layout commands, and device/firmware.

Use normalized design coordinates initially. A canonical em size such as 2048
is a representation choice, not a recovered fact. Bitmap observations generally
cannot distinguish proportional rescalings of design coordinates and units per
em. Also measure the mapping from ZPL requested height/width to the scaler's
pixels-per-em (ppem); do not assume they are identical.

### Bounded original-file investigation

The previous directory snapshot listed `Z:0.TTF`; it has not been proven to be
font 0. First compare resident `^A0` with explicit `^A@` selection of that file
at several sizes and orientations. Compare Swiss 721 separately rather than
assuming its installed file is equivalent. Use explicit field selection rather
than changing persistent default font mappings.

If a documented export or vendor-supplied matching asset is available, inspect
its table directory and identify its font family/version, glyph repertoire, and
instruction presence. Earlier FTP and `file.type` attempts did not retrieve the
file; do not repeat them unchanged. Stop this branch after the mapping test and
supported export review if no new route appears. The capture-based plan remains
useful regardless of the outcome.

## 2. Parameters and details to investigate

These are **candidate mechanisms**, not assertions that the resident font uses
all of them. Prioritize the first seven rows.

| Layer | Potential data or behavior | Recovery experiment |
| --- | --- | --- |
| ZPL scaling and origins | Requested dimensions → ppem; natural width; FO/FT offsets; integer versus fractional scaling; rotation order | Fixed-baseline size/width sweeps, explicit/natural width pairs, translated origins, and a known calibration font. `head` flags can affect ppem rounding. [head](https://learn.microsoft.com/en-us/typography/opentype/spec/head) |
| Outline geometry | `glyf` contours, on/off-curve points, quadratic control points, holes, overlaps, extrema, component transforms | Fit curves jointly to multiple large captures. Preserve winding and components. A file, if obtained, directly supplies `glyf`/`loca`; screenshots do not reveal original point numbering or decomposition. [glyf](https://learn.microsoft.com/en-us/typography/opentype/spec/glyf) |
| Metrics | `hmtx` advances/left bearings; baseline, cap height, x-height, ascenders, descenders; hinted phantom-point movement | Sentinel probes and repeated characters across sizes; compare one concatenated field with separately placed glyphs. Measure ink bounds and pen advance separately. [hmtx](https://learn.microsoft.com/en-us/typography/opentype/spec/hmtx), [glyph instruction overview](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructing_glyphs) |
| Shared design dimensions | `cvt ` values for recurring stem widths and alignments | Estimate font-wide stem classes and vertical zones from `H I l n m O o`, then allow small justified glyph-specific deviations. [control value table](https://learn.microsoft.com/en-us/typography/opentype/spec/cvt) |
| Hint state | Reference points, freedom/projection vectors, minimum distance, cut-ins, single-width behavior | Fit anchor relationships and distances, including diagonals, rather than moving all points on an axis together. Compare related strokes with different thicknesses. [graphics state](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_graphics_state) |
| Rounding and exceptions | Whole/half-grid and super-round period/phase/threshold; `DELTAP`/`DELTAC` size-specific adjustments | Dense transition sweeps; enumerate small discrete rule families and fit continuous dimensions inside each. Add sparse exceptions only after a repeatable discontinuity survives other explanations. [instruction set](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions) |
| Rasterization | Fixed-point precision, exact-edge decisions, winding, curve subdivision, dropout/stub handling | Known-outline bars, overlaps, shallow diagonals, tiny counters, and curves at controlled pixel phases; test N/R/I/B and stretching. [scan conversion](https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01) |
| Program lifecycle | `fpgm` initialization, `prep` size/transform setup, per-glyph programs, interpreter-dependent branching | Compare identical probes alone, reordered, and after another size. If bytes are recovered, inspect programs before implementing the instruction subset they actually need. [fpgm](https://learn.microsoft.com/en-us/typography/opentype/spec/fpgm), [prep](https://learn.microsoft.com/en-us/typography/opentype/spec/prep) |
| Device metrics and bitmap strikes | `hdmx`, `LTSH`, `VDMX`, optional embedded bitmaps and `gasp` rendering preferences | Inspect tables if available. Investigate discrete size anomalies, but do not infer a bitmap strike merely from a sudden pixel change. These tables need not be consumed by the printer. [hdmx](https://learn.microsoft.com/en-us/typography/opentype/spec/hdmx), [LTSH](https://learn.microsoft.com/en-us/typography/opentype/spec/ltsh), [VDMX](https://learn.microsoft.com/en-us/typography/opentype/spec/vdmx), [EBDT](https://learn.microsoft.com/en-us/typography/opentype/spec/ebdt), [gasp](https://learn.microsoft.com/en-us/typography/opentype/spec/gasp) |
| Character mapping and shaping | `cmap`, composite accents, pair positioning, fallback behavior | Establish all printable ASCII first; later test code pages/Unicode, accented pairs and missing glyphs. Test whether pair positioning is used before fitting kerning or attributing it to GPOS. [cmap](https://learn.microsoft.com/en-us/typography/opentype/spec/cmap), [GPOS](https://learn.microsoft.com/en-us/typography/opentype/spec/gpos) |

Font names, timestamps, table packing, original instruction order, and component
reuse cannot generally be recovered from raster samples and are not prerequisites
for an equivalent renderer. Variable-font axes and complex shaping are deferred
unless font inspection or observations establish their relevance.

## 3. Capture campaign and experimental controls

### A. Repair the measurement harness first

Extend `zpl/src/font_extract.rs` and `zebra-http-api/examples/font-study.rs`:

- Replace the 128-dot limit and fixed two-column/tile geometry with planned
  per-glyph canvases bounded by the printer's measured preview limits.
- Keep advance probes separate when a sentinel string would exceed the canvas.
  Reject unplanned clipping, empty responses, and altered image dimensions.
- Record source identity/firmware where available, DPI, command bytes, hashes,
  origins, requested sizes, and sequence. Repeat selected pages at start/end.
- Add a dry-run request manifest and request cap. Begin with at most 200 new
  preview requests, at the existing sequential pacing; expand only after the
  pilot is informative. Do not run competing clients against the shared preview.

Larger captures are not automatically unhinted. Use several sizes to distinguish
stable geometry from remaining rounding effects. Attempt 256, 384, and 512 dots
first. Use larger or anisotropic probes only when the full glyph and adequate
margins fit; increasing a limit alone will not fix an oversized atlas.

### B. Establish three datasets before fitting

1. **Historical development set:** all captures already inspected, including
   20/31/33. Stop calling these an untouched test set.
2. **New development captures:** diagnostic glyphs `HIl|OonmgjpAV_.`, dense
   normal-size sweeps through the small-label range, and selected independent
   width sweeps. Refine around observed stroke-width transitions.
3. **Sealed acceptance set:** choose and record size blocks before acquisition,
   for example 34–38 and 58–62, plus a large size such as 448, fresh stretched
   pairs, text strings, and rotations. Keep their pixels out of optimization and
   model selection. A failed acceptance test becomes development data; create
   a replacement sealed set for the next claim.

Use adaptive page packing to fit multiple small glyphs/sizes in one preview;
the manifest must distinguish glyph observations from HTTP request count.
Integer origin translations test placement consistency but do not create a
fractional-pixel phase sweep. For controlled subpixel phases, vary known glyph
geometry in a calibration font or use changes in scale with a known outline.

### C. Calibrate with known outlines

Create a small synthetic TrueType calibration font in Rust: bars, rectangles,
overlaps, a quadratic curve, a diagonal, and a tiny counter. Include known
metrics and both instruction-free and deliberately instructed variants. A
bounded SFNT writer is sufficient; no general font editor dependency is needed.
Prepare the font locally and use a dedicated temporary printer object when
running this experiment, with preview-only requests.

Compare printer output with our rasterizer for positive/negative edges, exact
pixel-center contacts, quarter turns, and unequal X/Y scales. Known instructions
can distinguish size rounding, dropout behavior, and hint execution. Verify that
this downloadable-font path is relevant to the resident font; identical engines
must not be assumed. If unsupported, retain multiple rasterizer hypotheses.

**Gate:** explain the calibration residuals before changing resident glyph curves
to compensate for them. A generic path test alone does not establish conformity
to the printer's font rasterizer.

## 4. Recover the font in separable stages

### Stage 1 — Metrics and scaling

Measure `|glyph|`, `||`, and repeated runs with several lengths and sentinels.
Fit advance versus run length, then test `AV`, `To`, `WA`, spaces, punctuation,
and mixed-case text. Distinguish per-glyph rounding, accumulated fractional
advances, kerning, and sentinel interactions. Repeat FO/FT and natural/explicit
width cases. Determine baseline and overshoot from flat and round glyphs.

**Deliverable:** measured metrics, uncertainty intervals, and a tested scaling
contract. Gate on exact held-out string origins/advances independently of ink
shape. Do not let the shape optimizer hide spacing errors with per-image shifts.

### Stage 2 — Smooth geometry

Fit quadratic contours jointly to the large development captures. Start with
extrema, straight segments, corners, and tangent constraints. Use a small control
point budget and preserve holes/winding; allow asymmetric glyphs when supported.
Fit in normalized font units and anchor the baseline to remove scale/translation
ambiguity. Hold large-size geometry approximately fixed during subsequent hint
fitting so improvements at 12 dots cannot destroy the 128-dot shape unnoticed.

Use a distance-to-contour or supersampled loss for coarse curve fitting, then
exact binary rasterization for selection. A quadratic can be converted exactly
to a cubic for the existing path representation; however, its current 0.3-dot
flattening tolerance must be assessed against the calibrated font rasterizer.

**Deliverable:** compact curves plus geometry-only errors at unseen large sizes.
If a smaller smooth model cannot match those, revisit scaling/rasterization
before adding hints. Do not assume the original `unitsPerEm` or point count was
recovered merely because one canonical representation fits.

### Stage 3 — Structured hint graph

Replace “one set of six parameters per glyph axis” with explicit stem-edge pairs,
vertical alignment zones, shared width classes, and reference-point relations.
Pin one edge or a center, fit its paired distance, then interpolate untouched
points. Learn whether thin/wide stems share a class; do not impose it blindly.
Handle diagonal measurements separately from horizontal/vertical ones.

For a candidate width law `q = floor(s*t + b)`, an observed integer width gives
`q - b <= s*t < q + 1 - b`. Intersect these constraints across sizes to estimate
feasible thickness/threshold regions before raster-level optimization. An empty
intersection is evidence against that rule or the stroke measurement. Enumerate
rounding modes, minimum-width and cut-in hypotheses rather than fitting only an
arbitrary `small/s` correction.

Use bounded multistart search with a complexity penalty. Model parameters at
three levels: font-wide dimensions, stem-class rules, and rare glyph-specific
exceptions. Add a sparse size exception only with repeatable evidence, charge
for every exception, and validate neighboring sizes. Preserve original and moved
point positions so interpolation behavior can be investigated.

**Deliverable:** an interpretable hint graph with ablation reports: geometry;
+metrics; +zones; +stems; +rounding; +exceptions. Include nonzero coefficient
counts, worst-case errors, and per-glyph regressions, not just one training score.

### Stage 4 — Rotations and scan conversion

Compare: rotated final bitmap; transformed outline with unchanged hints; and
transform-aware hints/rasterization. Use all four orientations and asymmetric
widths. TrueType permits size/rotation/stretch-sensitive behavior; test which
hypothesis explains residuals before introducing rotation-specific glyph data.
Sparse isolated edge differences should first be checked against exact-edge and
dropout rules, not patched with independent per-rotation outlines.

### Stage 5 — Text and renderer integration

Expand to all 95 printable ASCII characters and new multi-line strings before
integration. Include proportional advances, spaces, descenders, overlapping ink,
FO/FT placement, field wrapping, and unsupported-character behavior. Repeat on
another printer/firmware when available, keeping device-specific results distinct.

Keep the pipeline output-neutral:

```text
ZPL text and dimensions
  → glyph mapping and metrics
  → scaled/hinted glyph geometry
  → font fill and dropout treatment
  → generic scene
  → PNG / SVG adapters
```

Add an explicit fill rule or normalize glyph contours before entering the scene;
do not globally change existing shape semantics. If exact monochrome dropout
pixels need extra geometry, emit them explicitly. SVG exactness applies at the
requested dot dimensions; resizing an already-generated SVG does not rerun hints.

## 5. Acceptance criteria and stop rules

The following are proposed gates, not already achieved guarantees:

- Preserve zero mismatches for existing exact 32-dot fixtures.
- First scalable release: at least 20% fewer total mismatches than the current
  best vector baseline **in each** sealed normal-size, stretched-width, rotation,
  and text group; no group-level regression masked by averaging.
- Report raw XOR, normalized ink error, missing/extra ink, exact-case fraction,
  worst glyph/size, advance error, and model size. Show every regressed case.
- Metrics/layout must pass their exact gates independently of the ink objective.
- Reserve “exact” for zero differences on the entire declared domain, not for a
  threshold improvement or a few glyphs. Keep approximation warnings otherwise.
- If two materially different parameter sets predict the same measured pixels,
  report non-identifiability and request an observation that separates them.
- If new hint parameters improve training but fail sealed groups, keep them out
  of the renderer. Return to measurement or model structure instead of repeating
  unrestricted coordinate descent on the same validation set.

## 6. Machine learning and implementation order

Start with curve fitting, interval constraints, discrete rule enumeration, and
small program/graph search. These suit the existing dependency-free Rust tools
and keep failures understandable. Differentiable rasterization can assist smooth
outline fitting; [DiffVG](https://cseweb.ucsd.edu/~tzli/diffvg/) is a research
reference, not a required package or proof of exact monochrome emulation.

Only consider a neural residual model after collecting substantially broader
independent data and exhausting the structured baseline. Predict constrained
hint parameters rather than arbitrary output pixels; require compact export,
ablations, and sealed tests. Training dependencies would require a separate
scope decision under the current no-new-dependencies constraint. Never treat
an inferred hint model as recovered original bytecode.

Suggested implementation commits, in dependency order:

1. Capture manifests, dynamic canvases, sealed splits, and metric measurements.
2. Known-font calibration and explicit font scan-conversion tests.
3. Smooth quadratic outlines and independently validated scaling/advances.
4. Stem/zone hint graph with discrete rounding and sparse exception fitting.
5. Transform-aware evaluation, ASCII/text coverage, and grouped acceptance report.
6. Renderer integration only after the gates pass; otherwise retain research tools.

**Immediate next task:** implement the new capture/metrics manifest and the
known-outline calibration probes. These have higher diagnostic value than another
unstructured fit on the 15 existing glyphs.
