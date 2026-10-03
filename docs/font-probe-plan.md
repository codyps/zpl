# Constructed fonts as printer probes

Assessment and offline experiment: 2026-10-02. This extends the
[arbitrary-size font assessment](font-scaling-options.md). Constructing a font,
installing it in printer RAM, and observing its HTTP preview is a high-value way
to separate the font's data from the printer's behavior. The font can contain
both known geometry and deliberately chosen hint instructions.

**Live follow-up completed:** [October 2 results](font-probe-results-20261002.md)
record uploads to a ZD621 and ZQ610 Plus, 43 previews, native comparisons, and
verified cleanup. The sections below preserve the initial offline experiment and
the broader probe design; not every proposed variable was exercised in that pass.

**Use constructed fonts to calibrate the local engine, and later to test candidate
resident-font models on the printer itself.** This can proceed without the
external Font 0 TTF. It requires a finite set of experiments across sizes, rather
than a production bitmap asset for every size.

## Existing live evidence

The [September 14–15 campaign](font-refinement-results.md) already exercised this
route on a ZD621 at 203 DPI, firmware `V93.21.33Z`:

- Two small original TTFs contain bars, overlapping contours, a quadratic curve,
  a diagonal, a counter, a negative bearing, and a subpixel-width stem. One font
  adds a one-device-pixel `SHPIX` translation to every glyph's outline points.
- All 80 instructed observations matched the predicted translation of the
  corresponding printer-rendered plain glyph, across ten configurations.
- Additional fonts changed `SCANCTRL`/`SCANTYPE` in `prep` and in glyph programs.
  The tested changes produced identical printer pixels. This does not establish
  that all such controls are ignored in every configuration.
- Including `~DY` in the HTTP preview submission failed to install the font.
  Separate binary `~DY` transfers over TCP 9100 installed RAM objects; subsequent
  HTTP previews selected them using `^A@`. All nine temporary objects were removed.

The [writer](../zebra-http-api/examples/font_refine/sfnt.rs),
[capture planner](../zebra-http-api/examples/font_refine/campaign.rs), fonts, and
native previews are already available. These historical uploads established
feasibility before the new October 2 campaign.

## New offline comparison

I rendered the exact two constructed TTF files with FreeType 2.13.2 and compared
them against the 20 accepted calibration pages. The new
[comparison script](../zpl-font-extract/scripts/compare-calibration-freetype.py)
checks font, ZPL, and PNG hashes and uses each original canvas, explicit FT
baseline, and declared rotation. It performs no alignment, resampling, or clipping.
Per-glyph tile membership is used only to attribute errors; the reported page
differences include the entire original canvas.

The script also independently checks all ten plain/instructed printer-page pairs:
the declared one-pixel shift, transformed into the appropriate direction, still
has zero differing pixels. Sealed samples are not read.

| FreeType mode | Plain: differing pixels | Plain: exact ink cases | Instructed: differing pixels | Instructed: exact ink cases |
| --- | ---: | ---: | ---: | ---: |
| Embedded/native hinting, automatic hinting disabled | 425 | 52/80 | 450 | 52/80 |
| Hinting disabled | 440 | 54/80 | 4,604 | 1/80 |
| Automatic hinting forced | 617 | 48/80 | 4,683 | 1/80 |
| Default hint selection | 425 | 52/80 | 450 | 52/80 |

All modes use `FT_LOAD_RENDER | FT_LOAD_TARGET_MONO`. The native mode additionally
uses `FT_LOAD_NO_AUTOHINT`; the other ablations use `FT_LOAD_NO_HINTING` or
`FT_LOAD_FORCE_AUTOHINT`. See [FreeType glyph retrieval](https://freetype.org/freetype2/docs/reference/ft2-glyph_retrieval.html).
Each glyph uses a fresh face. This deliberately leaves cross-glyph interpreter
state and cache effects for a separate experiment.

The native plain result has 199 missing and 226 extra pixels, with 97.580553%
foreground IoU. The instructed result has 146 missing and 304 extra pixels, with
97.449558% IoU. Exact-case counts here cover **ink only**: isolated captures do not
measure advance widths. Reported local advances are diagnostic output, not a
successful printer-spacing comparison.

For context, the earlier custom research rasterizer's best selected hypothesis
had 275 differing pixels and 52/80 exact plain cases. It was selected using these
development samples; FreeType's configuration here was not optimized against
them. This is not a held-out contest between engines. It does show that adding
FreeType with these settings does not by itself resolve the calibration gate.

Error attribution suggests more useful probes than another full size sweep:

| Known glyph geometry | Earlier selected research model: XOR | FreeType native: XOR |
| --- | ---: | ---: |
| A: integer-phase thick bar | 0 | 0 |
| B: phase-offset thick bar | 0 | 48 |
| C: overlapping rectangles | 22 | 22 |
| D: quadratic curve | 25 | 24 |
| E: thin diagonal | 73 | 131 |
| F: rectangular counter | 31 | 17 |
| G: negative bearing and descender | 0 | 56 |
| H: thin vertical stem | 124 | 127 |

The FreeType B/G differences occur at 64 dots, where those contours have
half-dot X coordinates. Edge decisions are a plausible explanation to test;
the counts alone do not identify the rule. Most other residuals involve thin
features, curves, and contour boundaries. The plain/instructed FreeType error
totals also differ, so the one-dot instruction must remain an explicit control
rather than assuming identical residuals after translation.

Full results, flags, versions, hashes, and per-glyph directional counts are in
[font-probe-freetype-v1.json](font-probe-freetype-v1.json). Reproduce with:

```sh
uv run --no-project --with freetype-py==2.5.1 --with pillow==12.3.0 \
  python zpl-font-extract/scripts/compare-calibration-freetype.py \
  zebra-http-api/tests/fixtures/font-refinement-v1 \
  --output /tmp/font-probe-freetype.json
```

The output must be a new file. Python dependencies are research-only; Cargo
manifests and the production renderer are unchanged. The underlying FreeType
version is recorded separately from its Python binding version.

## The next constructed fonts

The following describe the broader proposed probe families. The linked live
results identify the subset actually executed and its observations. Give every
variant a distinctive visible identity marker. Put many glyph variants in one
font to amortize uploads and preview requests, while changing one explanatory
variable at a time.

### 1. Scaling and hint-state probes

Use small glyph programs to turn numeric or Boolean state into well-separated
solid marker patterns. Add an unconditional instruction-driven marker and a
geometry-only marker as controls; a blank conditional marker by itself cannot
distinguish a false result from ignored instructions or failed font selection.

Probe X/Y pixel size with `MPPEM` under each projection direction, and selected
`GETINFO` flags for rotation, stretching, and rendering mode. These are documented
in the [TrueType instruction reference](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#measure-pixels-per-em)
and its [GETINFO section](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#get-information).
Treat the result as the interpreter's reported state, not identification of the
printer's engine vendor or proof of the entire transform order.

Pair queried state with measured geometry under natural width, explicit equal
dimensions, unequal dimensions, and N/R/I/B. Use separate `prep` and glyph-program
markers and repeat after alternating sizes/fonts. This tests which state the
program actually sees and whether prior requests affect it. It is especially
useful before deciding how a single-size hinting API could support ZPL's two
independent dimensions.

### 2. Edge, precision, and dropout probes

Use an unhinted geometry family and a separately instructed translation family.
At 32 ppem with 2048 units/em, one design unit is 1/64 dot. This permits explicit
phase samples immediately below, on, and above a half-dot boundary without
changing the ZPL field origin or aligning captured pixels afterward.

| Family | Vary | Distinguishes |
| --- | --- | --- |
| Thick bars | X/Y phase around half dots; positive/negative coordinates | Inclusion of boundary centers, quantization, axis/sign asymmetry |
| Thin bars | Widths around half/one dot; X/Y orientation | Horizontal versus vertical thin-feature filling |
| Stubs and junctions | Same thin segment isolated, capped, or attached to a thick stem | Connectivity and endpoint rules versus unconditional minimum-width filling |
| Diagonals | Slope, phase, short/long extent | Intersection precision and directional dropout |
| Quadratics | Same curve encoded whole and as exactly equivalent subdivisions | Curve subdivision and arithmetic effects |
| Overlap/counters | Winding direction, contour order, equivalent decompositions | Fill and contour-boundary handling |

Use integral coordinates for exactly equivalent curve variants, and verify that
the encoded curves are mathematically equal before attributing differences to
the printer. Run local candidate renderers first; choose phase/shape combinations
where their predictions disagree. A few discriminating requests have more value
than repeatedly sampling sizes on which every hypothesis agrees.

The relevant rasterization contract is the
[TrueType scan converter](https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01#the-scan-converter).
The earlier scan-control experiments did not separate dropout modes. New controls
should explicitly distinguish enabling/disabling dropout, with an instruction
execution witness, as well as testing geometry where proposed rules differ.

### 3. Metrics probes

Hold contours fixed while changing `hmtx` advance and side bearing independently.
Then construct a variant that changes only the advance-defining phantom point.
Use repeated characters and visible sentinels, with paired FO/FT fields on each
page. This tests whether metrics come from scaled font tables, hinted points, or
some additional ZPL layout rule without confusing a shifted outline with a
changed pen advance. Phantom points are described in
[Instructing TrueType glyphs](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructing_glyphs#phantom-points).

The writer must declare its behavior and memory requirements correctly, including
the [head flags](https://learn.microsoft.com/en-us/typography/opentype/spec/head)
for size-dependent instructions/changed advances and accurate
[maxp bounds](https://learn.microsoft.com/en-us/typography/opentype/spec/maxp).
The existing two-font writer is intentionally narrow; it is not already a
general generator for these more elaborate programs.

## Testing a constructed Font 0 candidate

Uploading a reconstructed candidate adds a second use beyond calibration. Let
`P(F, s)` mean the printer rendering constructed font F at setting s, `L(F, s)`
the local rendering of the same bytes, and `R(s)` the resident Font 0 preview.

| Comparison | What it tests |
| --- | --- |
| `L(F, s)` against `P(F, s)` | Local font-engine behavior for known data |
| `P(F, s)` against `R(s)` | Candidate outlines, hints, metrics, and font-selection path on the printer |
| `L(F, s)` against `R(s)` | Actual end-to-end Font 0 approximation |

This helps prevent modifying outlines to hide local scan-conversion errors.
Once calibration is credible, encode compact smooth contours plus shared stem
and alignment rules as a TTF; test it on the printer, then locally. Font-table
and hint-program variants can be compared directly without fitting a separate
bitmap at each size. Keep geometry and spacing objectives separate.

Resident font selection and downloaded-font selection might use different
defaults or processing. Therefore the middle comparison does not isolate font
data perfectly. If the original external TTF becomes available, rendering the
unchanged file through the download path against resident `^A0` would be a useful
bridge control, with a distinctive selection check. The earlier missing-font
fallback demonstrated why selecting a filename alone is insufficient evidence.

Do not use the live printer for thousands of optimizer iterations. Fit and rank
candidates offline, submit a bounded set that distinguishes mechanisms, then
evaluate independent sizes/aspect ratios/strings. A compact rule may include
size-dependent thresholds; that is still one model, rather than a bitmap or
independently tuned parameter set for every requested size.

## Proposed bounded live pass

A reasonable initial target is about 20 pages: start/end resident and custom-font
controls; phase/overlap/curve atlases across four orientations; thin-feature
atlases; state probes; paired-origin metric probes; and two untouched confirmation
pages. Finalize the actual count only after dry-run packing verifies every canvas
and glyph boundary. Reuse the measured native canvas limits and a persistent
request budget; do not append this to the nearly exhausted 197-request campaign.

Prepare all fonts, requests, hashes, predicted outcomes, and selection markers
before the live run. Use a fresh directory and unused campaign-specific RAM object
names, with an ownership record and presence checks. Upload font bytes separately;
render formats only through the HTTP preview. Serialize requests against its
shared preview object, retain errors, and verify repeated controls and cleanup.
The subsequent live pass used 19 probe pages and two resident controls per
printer. It did not include untouched confirmation pages, alternating-font
lifecycle probes, or the full proposed matrix. Its observations are development
evidence, not a sealed generalization result.

The new comparison ran successfully across 640 glyph renderings (20 pages × four
modes × eight glyphs), with native page comparisons and all ten printer shift
controls passing their provenance/translation checks. The observed pixel errors
are the reported outcome, not a passing parity result. This offline comparison
performed no printer requests; the separate live results document the subsequent
uploads and previews. Neither experiment evaluated the sealed refinement samples.
