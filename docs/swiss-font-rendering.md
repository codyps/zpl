# Swiss TrueType rendering and Font 0 follow-up

On 2026-10-03 the original bounded TrueType handler reached **99.21% foreground
IoU on 738 Swiss development glyph cases** and **99.12% on 285 reserved cases**.
It uses one scaling, hinting and scan-conversion policy across sizes; it contains
no per-glyph or per-size Swiss bitmap data or correction coefficients. The
production ZPL renderer still uses its existing strikes. This is an explicit
low-level experimental backend, not a completed `^A@` or Font 0 integration.

Swiss is Swis721 BT Roman,
169,188 bytes, 979 glyphs, UPEM 2048; SHA-256
`da20a4d8c58b3ed09fb9177e09378bd1b824894751add6c1267c64d58f026456`.
External font bytes and Swiss outlines are not bundled in this repository.

## What changed

[`zpl::truetype`](../zpl/src/truetype/mod.rs) loads quadratic SFNT outlines and
executes bounded `fpgm`, `prep`, CVT and glyph programs. Scan conversion lives in
[`output::raster::truetype`](../zpl/src/output/raster/truetype.rs), with nonzero
winding isolated from the scene's even-odd semantics. No runtime font dependency
was added. [`ttf-examine`](../zpl-font-extract/src/bin/ttf-examine.rs) provides
opcode auditing, machine-readable glyph output and local PNG specimens.

The explicit `Environment::Zd621V93` plus `ScanMode::Zd621V93` applies these
measured mechanisms on the ZD621, 203 DPI, firmware V93.21.33Z:

1. Clamp requested dot dimensions to at least 10, independently in X/Y.
2. Convert nominal 203-DPI dots to sixteenths of a point. Round **down** for
   outlines/CVT, **up** for unhinted hmtx text spacing.
3. Convert that point size through the fixed-point 8-dot/mm matrix.
4. Execute hints before rotation; use positive edge ties and the existing
   horizontal dropout hypothesis. Subdivide quadratic segments using a shared
   device-space flatness threshold of 1/4 dot.

For requested dimension `d`, the inferred scaling model is:

```text
n = max(d, 10) * 72 * 16
outline_sixteenths = floor(n / 203)
layout_sixteenths  = ceil(n / 203)
effective_ppem = sixteenths * 184958 / (16 * 65536)
184958 = ceil((203.2 / 72) * 65536)
```

Scale design coordinates to 26.6 using that full effective value; rounding ppem
first discards the evidence. `Instance::layout_advance` rounds the independently
scaled hmtx advance to integer dots. It does not use hint-modified phantom
advances. The model explains both the small-size minimum and spacing previously
misattributed to ordinary nearest-integer size scaling.

These are inferred device mechanisms, not claims about Zebra's source code.
The curve threshold was selected on development cases from 1/1024, 1/8 and 1/4
dot candidates, then frozen before reserved Swiss comparisons. Remaining errors
include stretched hinting and edge/dropout behavior. Some individual glyphs are
substantially worse than the aggregate; this is not full-image parity.

## Evidence

All new captures used ZD621 serial D7J211001302, firmware V93.21.33Z, 203 DPI.
Requests, original PNGs, timestamps, dimensions and SHA-256 hashes are in the
linked fixture directories. Comparisons use complete native canvases at their
original origins, with exact underpaint and overpaint counts, no registration,
reference resizing or ignored boundaries. Per-glyph counts are diagnostic;
whole-page counts include all pixels. IoU is foreground intersection/union.

| Swiss group | Cases | Previous original engine IoU | Calibrated IoU | Under / over | Exact ink cases |
| --- | ---: | ---: | ---: | ---: | ---: |
| Development isolated glyphs | 738 | 95.7074% | 99.2053% | 231 / 461 | 506 |
| Development strings | 24 | 83.6860% | 99.5171% | 20 / 16 | 20 |
| Reserved isolated glyphs | 285 | 97.3352% | 99.1179% | 80 / 167 | 191 |

The development set covers ASCII at 16, 32 and selected 64-dot sizes,
independent dimensions 19×32 and 32×19, all rotations at 32, composite letters,
and repeated-character spacing. Reserved cases are 37×23 N, 23×37 N and 23×37 R;
none was used to select the candidate. FreeType 2.13.2 native monochrome scores
95.5307% and 97.2112% on the same isolated development/reserved sets.
The 24 string tests have matching measured spacing under the new model;
remaining string errors are ink differences. This does not cover arbitrary
kerning, shaping, wrapping or character mapping.

[Swiss captures and reports](../zpl-font-extract/tests/fixtures/swiss-font-20261003/)
retain three comparison modes: previous original engine, calibrated device mode,
and independent FreeType where applicable. The comparison tool can regenerate
additional diagnostic spacing policies. Standard-mode reference checks retain
464 exact constructed-font outlines/advances from the earlier investigation.

Constructed fonts expose intermediate values by magnifying their fractional
parts into integer-width bars, avoiding ambiguity from outline appearance:

- [Rounding](../zpl-font-extract/tests/fixtures/rounding-font-20261003/): ordinary
  ROUND thresholds for gray/black/white and fractional RCVT scale witnesses.
- [Scaling](../zpl-font-extract/tests/fixtures/scaling-font-20261003/): X/Y CVT
  and original-coordinate scale for every integer size 1–64, and 108 spacing
  cases across three dimensions.
- [Reserved scaling](../zpl-font-extract/tests/fixtures/scaling-font-validation-20261003/):
  the frozen formula predicts every X/Y witness at sizes 65–128.

**All 18 constructed-font canvases are pixel-exact**, including 1,024 scale
witnesses and the spacing cases. Rust tests pin zero underpaint/overpaint,
hashes, dimensions and provenance. Each campaign verifies font selection,
repeated resident controls and deletion of its temporary RAM font. Swiss uses
the already installed font and verifies its bytes before capture. These 42
previews included three original font uploads; cleanup was confirmed each time.
No physical labels were printed. The earlier general geometry probes still
have nonzero residuals under the calibrated mode; separate Rust baselines retain
those curve, edge and rotation errors. Exact scale witnesses do not imply an
exact general scan converter.

## Font 0 pilot

The Swiss result justified a bounded larger-outline capture:
[`font0-outline-20261003`](../zpl-font-extract/tests/fixtures/font0-outline-20261003/).
It contains `H O S g j @` at 256, 384 and 512 dots, and independently reserved
320 and 448-dot pages. Eighteen sequential previews include repeated large-glyph
and resident controls, both pixel-identical. Every silhouette fits within its
native 768×1408 canvas without clipping.

A fixed initializer traces oriented pixel-cell boundaries at 512 dots, preserves
holes and components, simplifies at the prior experiment's 0.65-dot tolerance,
and creates an unhinted polygon TTF locally. The original handler renders that
single outline at every evaluation size. There is no size-specific fitting,
translation adjustment, quadratic fitting or inferred hint program.

| Font 0 size | Role | Under / over | Foreground IoU |
| --- | --- | ---: | ---: |
| 256 | Development diagnostic | 982 / 1,239 | 97.4520% |
| 384 | Development diagnostic | 1,841 / 2,446 | 97.8122% |
| 512 | Source / initializer check | 68 / 61 | 99.9626% |
| 320 | Reserved | 763 / 1,130 | 98.5992% |
| 448 | Reserved | 1,488 / 3,377 | 98.1665% |

This establishes useful scalable **large-size geometry**, not recovery of
`Z:0.TTF`. The initializer still has 2,208 polygon vertices across six glyphs;
curves need compact quadratic fitting, and small-size hints and spacing remain
unmeasured by this pilot. Large-size IoU cannot validate small-size text.
The next reconstruction step is a compact multi-size quadratic/stem model,
followed by new small-size and string holdouts. Existing exact strikes should
remain until a candidate preserves their stronger baselines. The older sealed
font-refinement validation pages were not opened.

## Reproduce offline

```sh
export CARGO_TARGET_DIR=/home/cody-ai/.cache/cargo-targets/zpl-truetype
cargo build --locked -p zpl-font-extract --bin ttf-examine
"$CARGO_TARGET_DIR/debug/ttf-examine" specimen /path/to/swiss.ttf /tmp/swiss.png 37 23 'Swiss 0123' zd621
uv run --offline --no-project --with freetype-py==2.5.1 --with pillow==12.3.0 \
  python zpl-font-extract/scripts/compare_swiss.py \
  zpl-font-extract/tests/fixtures/swiss-font-20261003 /path/to/swiss.ttf \
  --engine "$CARGO_TARGET_DIR/debug/ttf-examine" --output /tmp/swiss.json
# Add --validation for the already-evaluated reserved set.
uv run --offline --no-project --with freetype-py==2.5.1 --with pillow==12.3.0 \
  python zpl-font-extract/scripts/analyze_font0_outline.py \
  zpl-font-extract/tests/fixtures/font0-outline-20261003 \
  --engine "$CARGO_TARGET_DIR/debug/ttf-examine" --output /tmp/font0.json
cargo test --locked -p zpl --test truetype --test printer_accuracy --test conformance_preview
```

`compare_constructed.py ROOT --engine BINARY --output REPORT` replays a rounding
or scaling campaign. Capture scripts have separate explicit preparation/capture
commands and never run as part of tests. Use a new output directory for live
runs; preview operations on the same printer must be serialized.

The engine remains intentionally constrained: no CFF/variable font support,
transformed composite components or complete TrueType instruction coverage.
Unsupported operations return errors rather than silently dropping hints.
It has instruction, recursion, stack, point, coordinate and bitmap budgets.
The offline references are development dependencies only. See the earlier
[engine scope and validation](font-scaling-options.md#original-engine-implementation)
and Microsoft's [TrueType instructions](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions)
and [scan conversion](https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01#the-scan-converter).

Final validation: 97 Rust tests passed (core/font-extract libraries, TrueType,
printer accuracy and conformance), plus seven offline Python tests. Changed
Rust targets pass strict Clippy, rustfmt and a `wasm32-unknown-unknown` compile
check. All 3,932 Swiss cmap/size cases execute under the calibrated profile; this
is an execution check, not a printer-parity claim for the full character set.
