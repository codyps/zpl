# Font rendering at arbitrary sizes

Initial assessment: 2026-10-02.

**2026-10-03 update:** The user confirmed that protected `Z:0.TTF` cannot be
downloaded. The original handler has now been calibrated against the available
Swiss font and constructed probes; reserved Swiss glyphs reach 99.12% foreground
IoU. A large-size Font 0 outline pilot has also been captured. See
[Swiss rendering and Font 0 follow-up](swiss-font-rendering.md) for current
results; the original-file acquisition recommendations below are historical.

 The target is printer-faithful text at arbitrary requested
dot sizes, including independent width and height, without adding a captured
strike for each combination. This investigation includes offline comparisons and
a [live constructed-font campaign](font-probe-results-20261002.md); the production
renderer is unchanged.

The follow-up now includes an **original experimental TrueType engine** in
[`zpl::truetype`](../zpl/src/truetype/mod.rs), with scan conversion under
[`output::raster::truetype`](../zpl/src/output/raster/truetype.rs). It generates
glyphs at the requested size without per-size captures or coefficients. Its
measured scan-conversion residuals still prevent enabling it as the default
Font 0 fallback. See [implementation and validation](#original-engine-implementation).

**Prioritize the original Font 0 outlines and hinting programs.** Evaluate them
offline against the existing development captures, then choose how to execute
them. The later [Font 0 TTF comparison](font0-ttf.md) changes the priority from the
earlier reconstruction studies: it identifies a font file and reports promising
monochrome results. Reconstructing outlines from pixels is now a fallback if that
asset cannot be used.

The follow-up [constructed-font investigation](font-probe-plan.md) adds another
immediate route: use known outlines and deliberately written hints to probe the
printer pipeline. Its fresh offline comparison finds that FreeType does not
exactly reproduce the existing synthetic-font captures with the tested settings.
This calibration work can proceed without the external Font 0 file. The live
follow-up uploaded one original font to a ZD621 and ZQ610 Plus: all 19 probe pages
matched between devices. It confirmed independent X/Y hinting and exposed specific
spacing, half-dot edge, dropout, and rotation differences from FreeType.

## Current behavior and evidence

[`selected()` and `fallback()`](../zpl/src/render/font.rs) use exact captured
Font 0 strikes where available; otherwise they scale the 32-dot bitmap and its
advances. Font 0 does **not** select the nearest available strike. That selection
exists for P–V presets. The A–H bitmap faces have a different contract: native
matrices and integer magnification. Smooth rendering should initially target
Font 0; other resident faces need their own evidence.

The difficulty has three independent parts: glyph shape, device-grid fitting,
and text metrics. For example, the [calibration study](font-refinement-results.md)
measured `H` advancing 20 dots at height 32 but 39 at height 64; `l` advanced 8
then 17. Scaling a captured advance cannot reproduce both. Those two heights
already have exact strikes; these measurements explain why the same scaling
strategy is insufficient at other heights.

Existing results, all limited to the documented ZD621 203-DPI evidence:

| Approach already examined | Result | Implication |
| --- | --- | --- |
| Scale a larger bitmap | 128-to-32 scaling differed by 711 pixels across 16 study glyphs; the native 32-dot strike was exact | More source resolution alone does not recover small-size hinting |
| Fit polygon outlines and simple hints | Outline fitting changed unseen normal-size errors from 1,438 to 1,445; adding simple hints increased them to 1,553 | A better training fit did not establish generalization |
| Fit stroke rounding | Unseen normal-size errors rose from 1,445 to 1,682 despite a 40.5% reduction in the normalized training objective | Avoid another unconstrained fitting pass or a growing exception table |
| Original TTF through FreeType 2.13.2 monochrome rendering | ASCII foreground IoU was 97.1540% at 40x24 and 92.1348% at 16x10 | Strong candidate for broader evaluation, not proof of arbitrary-size parity |

Sources: [bitmap scaling study](font-reconstruction.md),
[outline fitting](font-fitting.md), [stroke fitting](font-stroke-fitting.md),
and [TTF comparison](font0-ttf.md). These are recorded results, not fresh runs.
The historical reconstruction reports predate the TTF identification; their
statements that no original file was available describe those experiments.

The TTF comparison used the direct Unicode cmap. Only 38/95 and 64/95 ASCII
glyphs, respectively, matched both pixels and advances exactly. Some individual
glyphs fell below 80% IoU. The Unicode supplement scored 80.8162% overall and
included characters missing from the TTF. Preserve the renderer's ZPL mappings,
blank/default-glyph behavior, and missing-character advances when evaluating a
new backend; a font engine's default mapping is not the complete printer policy.

The external `Z-0.TTF` was absent from the documented artifact path in this
checkout and the corresponding main checkout during this assessment. Its recorded
identity is CG Triumvirate Condensed Bold v4.01, 125,904 bytes, with `fpgm`, `prep`,
and `cvt ` tables. Asset availability and permission to distribute it remain
separate deployment questions. A caller-supplied font path avoids assuming the
file can be bundled, but would not by itself provide the public browser preview
with a default font.

## Options

| Option | Expected benefit | Cost and limitation | Assessment |
| --- | --- | --- | --- |
| Original TTF + FreeType monochrome rendering | Executes existing hint programs and generates each requested size on demand; strongest measured starting point | New C runtime/build integration if shipped; native and Wasm paths need validation; remaining printer differences still need explanation | Use as the offline reference first; consider a runtime backend if dependencies are acceptable |
| Original TTF + Skrifa + a monochrome rasterizer | Rust outline loading and embedded hint execution; potentially a better integration fit for native and Wasm | Skrifa supplies hinted paths, not a complete printer-compatible monochrome pipeline; anisotropic hinting and rotations require investigation | Evaluate as a candidate, not an assumed drop-in replacement |
| Original TTF + an original bounded interpreter/rasterizer | Retains the original outlines and hint data while honoring an original-implementation approach | Substantial work: font parsing, composites, hint state, instruction execution, metrics, and scan conversion | Preferred implementation direction if adding a runtime font engine is outside scope; estimate from an actual font-program audit first |
| Smooth outlines + compact shared stem/zone rules | One small scalable model; no per-size bitmap assets; could use known outlines or reconstructed ones | Needs a calibrated rasterizer and held-out validation; previous simpler models failed | Research fallback if the original programs cannot be used |
| Unhinted outlines or a substitute scalable face | Smooth arbitrary-size text with relatively little rendering machinery | Small-size stems, counters, widths and line wrapping can differ from the printer | Suitable only for an explicitly approximate preview mode |
| Nearest-strike scaling, bitmap interpolation, or a distance field | Can improve the appearance of some magnifications using existing assets | Does not supply missing hint decisions or size-dependent advances; nearest-strike changes can introduce discontinuities | Possible interim approximation; not the main solution |

FreeType documents separate [hinting targets and rendering modes](https://freetype.org/freetype2/docs/reference/ft2-glyph_retrieval.html#ft_load_target_xxx),
including a monochrome target, and independent width/height inputs to
[`FT_Set_Pixel_Sizes`](https://freetype.org/freetype2/docs/reference/ft2-sizing_and_scaling.html#ft_set_pixel_sizes).
Use embedded instructions as the first hypothesis rather than replacing them
with an automatic hinter or thresholding a default grayscale render.

Skrifa documents [scaled/hinted outline paths](https://docs.rs/skrifa/latest/skrifa/outline/index.html)
and an explicit interpreter with a monochrome target in its
[hinting implementation](https://github.com/googlefonts/fontations/blob/main/skrifa/src/outline/hint.rs).
Its public [`HintingInstance`](https://docs.rs/skrifa/latest/skrifa/outline/struct.HintingInstance.html)
takes one `Size`; the API reviewed here does not establish equivalence to separate
X/Y ppem. Stretching an already hinted outline is a different operation and must
not be assumed to reproduce independent ZPL width/height hinting. Wasm suitability
is an integration expectation, not a build or accuracy result from this review.

For an original interpreter, inspect the real font's programs and reachable
functions before choosing a subset. Include font initialization, size-dependent
setup, composite glyphs, and hinted advance/side-bearing points. A runtime trace
at a few sizes cannot prove that all size/transform-dependent branches were
covered. Unsupported instructions must fail explicitly. This route is substantial
even when it targets one known font rather than arbitrary OpenType inputs.

## Integration constraints

The scalable pipeline should resolve ZPL character mapping, dimensions, origin,
and orientation; obtain hinted metrics and glyph geometry; then produce the
existing output-independent scene. Layout measurement and drawing must use the
same metrics so that `^FB` wrapping and alignment remain consistent.

There are two practical scene representations:

- **Generate monochrome glyph coverage at the requested size and emit merged
  pixel runs.** This fits the existing bitmap-to-path machinery and gives the
  adapters the same dot geometry. An on-demand glyph cache is an optimization;
  it requires no printer capture or hand tuning at each size. Bound it and key
  it by font identity, glyph, both dimensions, orientation, and relevant rendering
  state. Account for device phase if placement is fractional.
- **Emit hinted vector contours.** This preserves curves, but needs a compatible
  font fill rule and scan conversion. The scene currently specifies even-odd
  filling; TrueType specifies nonzero winding, edge-center rules, and dropout
  treatment. Moreover, `font::union_lines()` and several text-layout callers
  assume glyph ink consists of rectangles. Passing arbitrary curves through
  those paths requires restructuring, not just a new outline loader.

The [TrueType scan-conversion specification](https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01#the-scan-converter)
explains those fill/dropout rules. The existing calibration campaign still had
275 differing pixels across 80 known-outline cases with its best research
rasterizer. Do not distort recovered outlines to compensate for that unresolved
pipeline error, or change all scene shapes to a different fill rule.

Prefer generated pixel runs for the first parity experiment. Keep any new
rasterization implementation under `output::raster`; adapters should continue to
consume scenes. Transform behavior needs explicit testing: existing printer
captures show that rotating a finished bitmap is not always pixel-identical to
the printer's rotated font output. Resizing an exported SVG also does not rerun
hinting; request a new render when the intended dot size changes.

## Recommended next experiment

1. **Calibrate using constructed fonts.** The initial live phase/state/metrics
   pass is [complete](font-probe-results-20261002.md). Use its same-font local and
   printer comparisons to implement/test shared edge and dropout rules, then
   distinguish the remaining metric-rounding and curve-precision hypotheses.
   Do not fit away unexplained engine differences by changing the outlines.
2. **Re-establish the original-font baseline offline.** Supply the external TTF,
   verify its recorded hash, and extend the existing comparison to the committed
   development strikes. Record the engine version and settings. First reproduce
   the direct-cmap baseline, then measure the effect of the existing ZPL mapping
   policy separately. Keep the sealed refinement pages untouched.
3. **Separate the remaining causes.** Compare unhinted, embedded-hinted, and
   automatic-hinted output as diagnostic ablations. Check advances independently
   from ink, then independent width/height, rotations, FO/FT placement, and long
   wrapped strings. Use one declared configuration across sizes; corrections
   should describe repeatable mechanisms rather than one strike's exceptions.
4. **Choose an implementation after that evidence.** If a font-engine dependency
   is acceptable, benchmark FreeType and the Rust candidate, including Wasm cost
   and transform support. Otherwise audit the original programs and implement a
   bounded original interpreter plus calibrated monochrome rasterizer. Keep
   measurement/fitting tools in `zpl-font-extract`, outside core rendering.
5. **Introduce a scalable fallback while preserving exact captures.** Validate
   the candidate at existing captured sizes with strike lookup deliberately
   bypassed in the experiment. Then evaluate independent sizes and strings held
   out from configuration selection. Ship it for unsampled sizes only after those
   gates pass; replace exact strikes separately when their stronger baselines are
   preserved. A finite sample can validate generalization without becoming a
   per-size production lookup table.

Report native-coordinate foreground IoU, underpaint/overpaint, worst glyphs,
exact pixels-plus-advance counts, and string layout errors separately. Do not
align, crop, pad, or rescale references. Group small/large sizes, stretched widths,
rotations, encodings, and layouts so a good average cannot hide a failed group.
Retain model/DPI/firmware provenance and independently selectable compatibility
behavior. Renderer integration must run the accuracy suites required by
[printer accuracy](printer-accuracy.md), alongside affected font/layout tests
and the [Wasm/browser checks](web-preview.md).

The initial review used source inspection, existing reports, and upstream
documentation. Follow-ups added a reproducible offline benchmark and 43 live
previews, including one retained diagnostic failure. Temporary fonts were removed
and the ZQ610 label-length settings restored. The missing input for a fresh
original-Font-0 comparison remains the external font file; synthetic-font
calibration does not depend on it.

## Original engine implementation

The engine is original Rust code with no added runtime dependency. It loads
quadratic SFNT outlines, Unicode cmap formats 4/12, simple glyphs and translated
composites, horizontal metrics, CVT, font/size programs, and glyph instructions.
Native hinting uses independent X/Y ppem. An instance retains prepared size state;
each glyph executes in an isolated copy. The instruction subset covers the
constructed probes and all mapped Swiss-font characters at the four tested sizes.
The audit tool counts framed opcodes in all programs; it does not prove branch
reachability or support at every size.

The scan converter uses nonzero winding and converts quadratic outlines into
monochrome coverage after rotation. `Center` applies center sampling;
`ZebraExperimental` adds the existing shared positive-edge and horizontal-dropout
hypothesis. Both are explicit research policies. `SCANCTRL`/`SCANTYPE` state is
retained in the outline but does not yet select a complete TrueType dropout
implementation. The existing scene's even-odd fill behavior remains unchanged.

The supported subset is deliberately limited: no CFF, variable fonts, transformed
components, shaping, kerning, or complete vertical metrics. `ISECT`, `IDEF`,
`S45ROUND`, and point-size-dependent `MPS` are explicit errors. GETINFO's version
is fixed at 35 as a reference configuration; the Zebra environment adds only the
measured stretch-flag behavior and does not establish the printer's engine version.
Unsupported instructions report the opcode and byte offset. Size, file, table,
stack, function, recursion, coordinate, composite expansion, execution-work, curve
segment, and bitmap budgets reject excessive inputs. These checks and deterministic
malformed-input tests are not a claim of exhaustive fuzzing.

The APIs accept caller-owned font bytes. Existing ZPL mappings, layout, and
captured strikes continue through the established renderer; there is no automatic
font download or production Font 0 substitution. This preserves the repository's
accuracy gate while making the new engine independently usable and measurable.

### Run it

```sh
cargo build --locked -p zpl-font-extract --bin ttf-examine
cargo run --locked -p zpl-font-extract --bin ttf-examine -- audit /path/to/font.ttf
cargo run --locked -p zpl-font-extract --bin ttf-examine -- \
  specimen /path/to/font.ttf /tmp/specimen.png 37 23 'Arbitrary sizes 0123456789'
```

`specimen` uses direct Unicode mapping, native hints, center sampling, and rounded
hinted advances. It emits black pixel runs into a scene, then uses the existing
PNG adapter. It is a visual smoke check, not a printer-layout comparison.

For exact baseline-relative data, run `ttf-examine glyphs FONT [center|zebra]
[native|none]` and send one `x_ppem y_ppem codepoint quarter_turns` request per
stdin line. Quarter turns 0/1/2/3 correspond to N/R/I/B. Each response includes
26.6 outlines, hinted and linear advances, bitmap origin, dimensions, and packed
rows, or an explicit error.

The [comparison script](../zpl-font-extract/scripts/compare_truetype.py) checks
outlines and advances against an independent FreeType installation:

```sh
uv run --no-project --with freetype-py==2.5.1 python \
  zpl-font-extract/scripts/compare_truetype.py /path/to/font.ttf \
  --engine target/debug/ttf-examine --all --report /tmp/font-comparison.json
```

Adjust the executable path when `CARGO_TARGET_DIR` is set. Reports record the
actual FreeType version and font hash; they contain aggregate errors, not external
font outlines. To reproduce the committed 464-case reference byte-for-byte, use
the original `font-probes-20261002/probe.ttf` with `--reference-output PATH`.

### Measured results

[Machine-readable results](truetype-engine-results-v1.json) retain the native
printer residuals and independent Swiss-font comparisons.

| Comparison | Result |
| --- | --- |
| Original constructed font, eight X/Y sizes | All 464 hinted outlines and advances exactly match FreeType 2.13.2 |
| Printer hint-state probes | All 56 cases exact with the measured Zebra GETINFO policy |
| Printer geometry probes | 171/317 exact; 1,204 missing and 728 extra pixels; 1,932 XOR; 87.8856% foreground IoU |
| Swiss ASCII, four X/Y sizes | All 380 advances exact; 309/380 outlines exact; other outline coordinates differ by at most 1/64 dot |
| Swiss nonzero Unicode cmap entries, same four sizes | 3,932/3,932 execute and match advances; 2,438 outlines exact; maximum coordinate difference 33/64 dot |

The geometry result is **worse than the existing FreeType baseline** of 1,826
XOR and 175/317 exact cases. These adversarial geometry probes are not an estimate
of ordinary text quality. Do not replace captured strikes or claim arbitrary-size
printer parity from the current results. Independent holdouts and complete ZPL
mapping/spacing validation remain necessary before that integration.

The Swiss composite investigation isolated fractional component-offset rounding:
our `ROUND_XY_TO_GRID` rounds both X and Y, as specified in the
[OpenType component flag and placement paragraphs](https://learn.microsoft.com/en-us/typography/opentype/spec/glyf#composite-glyph-description).
The examined FreeType composite retained fractional X placement. Original synthetic
regressions now cover both-axis rounding, no-round offsets, metric inheritance,
phantom origins, cycles, and expansion limits. This explains one source of the
Swiss differences; it does not establish every residual's cause or the printer's
composite policy.

### Font assets

The Swiss font used for evaluation is `E:TT0003M_.TTF` from the ZD621:
Swis721 BT Roman, 169,188 bytes, 979 glyphs,
SHA-256 `da20a4d8c58b3ed09fb9177e09378bd1b824894751add6c1267c64d58f026456`.
The downloaded bytes remain in ignored local work files and are not bundled.

Resident Font 0 is listed as `Z:0.TTF`, 125,904 bytes. Its external font file is
unavailable for auditing and validating this engine.
The Swiss font is a real-font execution check, not a substitute for that validation.

### Validation

Executed `zpl` library tests, the TrueType integration tests, and the required
`printer_accuracy` / `conformance_preview` suites. The constructed-font Python
evidence tests pass with their pinned FreeType/Pillow environment, and regenerating
the outline reference produces identical bytes. The local PNG specimen was
rendered and visually inspected. `cargo check --target wasm32-unknown-unknown -p zpl`
passes; this is compilation evidence, not browser execution or browser integration.

Strict Clippy passes for the changed library, TrueType tests, and all
`zpl-font-extract` targets. A wider strict check of all `zpl` targets encounters
three pre-existing `assertions_on_constants` diagnostics in
`zpl/tests/profiles.rs:1413`; those unrelated tests were left unchanged.
