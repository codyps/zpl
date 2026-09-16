# Font refinement: first execution results

Campaign: September 14–15, 2026, ZD621 at 203 dpi, firmware `V93.21.33Z`.

**The measurement and calibration stages are implemented and exercised. The
calibration gate has not passed, so smooth resident outlines, a full hint graph,
and renderer integration remain pending.** The existing embedded font and
production PNG/SVG behavior are unchanged.

## Implementation and evidence

The dependency-free Rust [`font-refine` example](../zebra-http-api/examples/font-refine.rs)
provides:

- A deterministic dry-run manifest, adaptive tile packing, clipping/dimension
  checks, SHA-256 provenance, resumable captures, and separate sealed data.
- Single-glyph captures through 512 dots without the old extractor's 128-dot
  limit. The older extraction tool's limit is unchanged; this campaign uses its
  own bounded planner (768 × 1024 maximum canvas).
- A bounded SFNT writer producing known bars, overlaps, quadratic curves,
  diagonals, counters, negative bearings, and deliberately instructed variants.
- Explicit scan-conversion hypotheses and feasible rounding intervals for
  advances and observed stroke widths. No arbitrary per-size exceptions were fit.
- Offline analysis, a persistent request budget, a capture lock, and ownership
  checks for temporary RAM font installation and cleanup.

[Captured evidence](../zebra-http-api/tests/fixtures/font-refinement-v1/) includes
requests, PNGs, hashes, manifests, generated fonts, reports, and execution order.
The [analysis](../zebra-http-api/tests/fixtures/font-refinement-v1/analysis.json)
is derived entirely from development data.

There were **197 preview requests**: 131 original pages, 20 calibration retries,
30 `prep` control probes, 12 glyph control probes, and four adaptive diagnostics.
All accepted pages passed canvas/clipping checks. The start/end resident-font
repeat has **zero differing pixels**. All nine temporary RAM fonts were deleted;
the directory confirmed their absence. No physical print requests were sent.

The 22 sealed pages received capture-quality checks only. Their pixels have not
been used for fitting, model selection, or accuracy comparisons. Historical
20/31/33 samples remain development data.

## What we learned

### Download and font selection

An inline `~DY` in a preview request did not install our font. The printer rendered
ordinary letters instead of the known geometry. Those 20 rejected previews are
retained, rather than mislabeled as calibration results.

A separate binary `~DY` download over TCP 9100 installed the fonts in RAM, after
which `^A@` selected the correct shapes. Fresh calibration captures now require
that installation to be recorded and the two objects to exist. This follows the
local [ZPL specification](zpl-zbi2-pg-en.pdf), pages 181–183.

`Z:0.TTF` matches font 0 in all three tested configurations. However, a deliberately
missing font also matches font 0 exactly. Consequently, this experiment does not
prove the identity of `Z:0.TTF`. Explicit Swiss 721 selection differs by 1,488,
2,369, and 15,113 pixels in those configurations. No original font bytes were
recovered; the earlier unsuccessful export attempts were not repeated.

### TrueType instruction execution

Moving all outline points by one device pixel with `SHPIX` produces exactly the
predicted bitmap translation in all ten size/width/rotation configurations
(80 glyph observations). This verifies actual glyph instruction execution through
the downloadable-font path. It does not establish that resident font 0 uses the
same full pipeline.

Changing `SCANCTRL`/`SCANTYPE` in `prep` produced identical output for modes 0, 1,
2, 4, and 5. Repeating modes 2 and 5 inside glyph programs also produced identical
output, including thin strokes. We therefore cannot use these switches to isolate
this printer's dropout behavior. This is an observed lack of effect, not proof
that every scan-control instruction is universally ignored.

These controls and their intended semantics are documented in Microsoft's
[TrueType instruction reference](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions).

### Spacing is independently size-dependent

Measured pen advances, obtained by subtracting the `||` sentinel distance:

| Glyph | At 32 dots | At 64 dots | Doubling the 32-dot advance |
| --- | ---: | ---: | ---: |
| `l` | 8 | 17 | 16 |
| `H` | 20 | 39 | 40 |
| Space | 9 | 19 | 18 |

Repeated runs produce exact integer multiples in the tested lengths, with equal
advances under FO and FT. This supports per-glyph integer advances on this sample;
it does not establish the absence of kerning for arbitrary pairs.

Among the tested floor/nearest/ceil families, the additional predeclared probes
(`l` at 11 dots = 3; `H` at 12 dots = 7) leave nearest rounding feasible for all
three glyphs. The model is `advance = floor(height × dimension + 0.5)`:

| Glyph | Feasible dimension interval, upper endpoint excluded |
| --- | --- |
| `l` | [0.2578125, 0.2604166667) |
| `H` | [0.609375, 0.6145833333) |
| Space | [0.2890625, 0.296875) |

These are intervals, not recovered exact design units. The diagnostic probes
selected among hypotheses and are now development data, not sealed validation.

Across the 43 new normal sizes, visible stroke widths of `l` and `|` admit a
nearest-rounding thickness law, with minimum width one pixel. Their respective
intervals are [0.1315789474, 0.1326530612) and [0.0614035088, 0.0625). The same
simple families fail for the measured `H` and `I` strokes. Visible run width need
not equal a TrueType CVT stem distance; position and rasterization also matter.

### Calibration remains unresolved

Across 80 instruction-free synthetic glyph cases:

| Research scan-conversion hypothesis | Differing pixels | Exact cases |
| --- | ---: | ---: |
| Even-odd, half-open center sampling, no dropout | 1,154 | 44/80 |
| Best tested nonzero/dropout/edge-tie combination | 275 | 52/80 |

This is a development comparison of research hypotheses, not a production
renderer benchmark or acceptance result. The best model uses nonzero winding,
a horizontal thin-feature fill rule, and a positive device-coordinate edge tie.
Its residuals include curves, thin strokes, and rotated boundaries. Quantizing
control points to 26.6 fixed point did not distinguish the best hypotheses on
these samples; the original scaler precision is not identified.

The [TrueType scan-conversion specification](https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01)
and [FreeType's monochrome implementation](https://github.com/freetype/freetype/blob/master/src/raster/ftraster.c)
were examined as references. No FreeType code or dependency was incorporated.

## Gate decision and remaining work

The [plan](font-refinement-plan.md) requires explaining known-outline calibration
residuals before moving resident contours to compensate for them. **That gate
failed.** No smooth-outline optimization, full stem/zone graph, sparse exception
fit, sealed acceptance evaluation, or new renderer integration was performed.

The next experiment should distinguish scanline intersection precision,
horizontal versus vertical dropout handling, and endpoint/stub treatment with
controlled phase sweeps. Retain the measured advance intervals independently.
Only after that calibration should large resident captures constrain smooth
quadratics, followed by shared stem/zone hints and all-ASCII text validation.
There is currently no evidence justifying a neural model or new dependencies.

## Reproduce without the printer

From the repository root:

```sh
capture_root=zebra-http-api/tests/fixtures/font-refinement-v1
printer_url=http://printer.local/
direnv exec . cargo run --release -p zebra-http-api --example font-refine -- \
  --host "$printer_url" --analyze "$capture_root"
direnv exec . cargo run -p zebra-http-api --example font-refine -- \
  --host "$printer_url" --dropout-calibration --offline "$capture_root"
direnv exec . cargo run -p zebra-http-api --example font-refine -- \
  --host "$printer_url" --dropout-calibration --glyph-controls --offline "$capture_root"
direnv exec . cargo run -p zebra-http-api --example font-refine -- \
  --host "$printer_url" --diagnostic --offline "$capture_root"
direnv exec . cargo test -p zebra-http-api --example font-refine
```

These replay modes make no network requests. Reports are regenerated; source
requests and PNGs are checked rather than overwritten. For a new live campaign,
use a new output directory: dry-run first, `--install-calibration`, then
`--capture`; finally `--cleanup-calibration`. Credentials come from
`ZPL_USERNAME`/`ZPL_PASSWORD`. Use only one client against the shared preview.
The active capture lock is per output directory. Request reservations count
failed attempts as well as successful responses; the maximum is 200 per campaign.

## Validation

- `cargo test --workspace --all-targets`: 89 tests passed, including 13 tests in
  the new example (font structure, winding/dropout, rounding contradictions,
  request accounting, and captured font/spacing regressions).
- All 197 PNG hashes verified. Development, dropout, and diagnostic reports
  replayed byte-for-byte with a loopback host that provides no printer service.
- Targeted rustfmt and `git diff --check` passed. Clippy reported no warnings in
  the new example; pre-existing warnings remain in `zpl`.
