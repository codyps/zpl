# Offline font parameter fitting

`fit-font` is a Rust experiment that learns vector outlines from the saved
[font study](font-reconstruction.md). It adds no external dependencies, makes no
printer requests, and does not replace the renderer's embedded font.

## Reproduce

```sh
direnv exec . cargo run --release -p zebra-http-api --example fit-font -- _font-fit
# Smaller experiment:
direnv exec . cargo run --release -p zebra-http-api --example fit-font -- \
  --characters AgO1 _font-fit-small
```

Output must be a new directory. `--captures PATH` selects a compatible font-study
capture directory; the default uses repository fixtures. `--passes 1..4` controls
coordinate-descent passes at each step size (default 1). Results are deterministic
for the same inputs, parameters, and floating-point behavior.

## Model and objective

The initializer traces oriented pixel boundaries from the 128-dot normal
capture, preserving separate components and holes. Closed contours are simplified
with a 0.65-source-dot distance tolerance. Each vertex has two fitted coordinates;
these are piecewise-linear vector outlines, not recovered quadratic curves.

Bounded coordinate descent tries positive and negative moves of 0.5, 0.25, and
0.125 source dots. Each vertex stays within 1.5 dots of its traced position in
each axis. Only moves reducing the training objective are accepted. A second
stage fits four parameters per glyph:

- Constant X/Y pixel phase in `[-0.5, 0.5]` dots.
- X/Y bounding-box snapping strengths in `[0, 1]`.

Snapping interpolates between unsnapped bounds and bounds rounded to whole dots.
It is a deliberately small grid-fitting model, not a TrueType hint interpreter
or a learned stem-specific program. The objective averages binary differing
pixels divided by reference ink across training sizes, giving small sizes weight
instead of letting the largest image dominate. Reports also retain raw mismatch
counts and ink IoU; these are different metrics from the optimization objective.

Training uses N captures at 12, 16, 24, 32, 48, 64, 96, and 128 dots. Model fitting
has no access to the validation samples. Evaluation subsequently loads 20, 31,
and 33 dots; stretched width cases; and R/I/B rotations. These are held out from
this fit, but were inspected in the preceding exploratory study. Rotation tests
inverse-rotate printer pixels without searching for a best alignment. Since the
model has no orientation-dependent parameters, these tests measure an important
remaining limitation.

## Outputs

- `model.json` and `outline-model.json`: fitted contours and hint parameters, training-image SHA-256
  hashes, objective histories, and coordinate/algorithm metadata.
- `outlines.svg`: unhinted vector contours at the source size for inspection.
- `report.json`: initial, outline-only, and hinted errors per glyph and case,
  explicitly labeled `train` or `validation`.
- Selected `*-diff.png`: printer-only pixels magenta; hinted-model-only pixels cyan.

The default models the 15 nonblank study glyphs (`AgjQMWil1O0@%&|`). It does not
fit advance widths, kerning, Unicode coverage, or text layout. Space and unsampled
characters are not inferred. There is no original-TTF or all-sizes exactness claim.
Contour simplification and bounded local search may leave local minima; even
successful bitmap fits do not uniquely identify an original font.

## Checks

```sh
direnv exec . cargo test -p zebra-http-api --example fit-font
```

Tests check tracing with holes and diagonal contacts, recovery of a synthetic
coordinate displacement, monotonic training loss, and rasterization agreement
with the repository's generic path adapter at multiple sizes and aspect ratios.

## First experiment: 2026-09-14

The default one-pass run fitted **1,372 vertices (2,744 coordinates)** across
15 glyphs, then tested 60 additional phase/snapping parameters. Fitted parameters,
SVG outlines, and the full per-glyph report are preserved in
[`font-fit-v1/`](font-fit-v1/). `outline-model.json` disables the second-stage
hints; `model.json` contains them. Neither is installed in the renderer.

The mean normalized training objective fell from **0.18544** to **0.14199** with
outline fitting, then **0.13623** with hints. That does not imply that raw pixel
errors improve everywhere:

| Evaluation group | Traced initializer | Fitted outlines | Outlines + hints |
| --- | ---: | ---: | ---: |
| Training: 8 normal sizes | 5,766 | 4,153 | 4,604 |
| Held-out normal sizes: 20, 31, 33 | 1,438 | 1,445 | 1,553 |
| Held-out normal stretched widths | 2,377 | 2,035 | 2,079 |
| Rotations: 42 cases | 28,796 | 22,978 | 24,765 |
| All validation cases | 32,611 | 26,458 | 28,397 |

Counts cover the same 15 glyphs per case. The 18.9% aggregate validation
improvement from outline fitting is substantially driven by rotation cases,
many of which share training sizes. **Unseen normal-size performance did not
improve in aggregate.** The simple hint model also regressed raw validation error.
This is evidence against promoting either model as an exact scalable replacement.

Comparison with the existing renderer at the held-out normal sizes:

| Height | Existing 32-dot bitmap scaling | Initial vector | Fitted vector | Fitted + hints |
| --- | ---: | ---: | ---: | ---: |
| 20 | 369 | 292 | 269 | 285 |
| 31 | 659 | 591 | 636 | 642 |
| 33 | 596 | 555 | 540 | 626 |

At the existing renderer's exact 32-dot size, the fitted outlines have **510**
mismatches versus the bitmap's **zero**. At 128 dots the initializer has only
72 errors, but fitting increases this to 257 while reducing errors at other
training sizes. These tradeoffs are expected from the chosen size-balanced loss,
not evidence of recovering the original font.

### What this tells us

A scalable vector approximation is feasible and the optimizer can reduce its
training error. This small model does not explain the font's size-specific
rasterization. More vertex passes alone risk overfitting pixel edges. The next
experiment should fit explicit **stem widths and positions, rounding thresholds,
and vertical alignment zones**, preferably initialized from higher-resolution
captures and smooth quadratic curves. Keep new sizes and text as validation;
measure advances separately before attempting complete text rendering.

A neural model is still not required to test those hypotheses. This experiment
uses ordinary numerical parameter search and leaves inspectable parameters. It
has not recovered an original TTF, a quadratic-curve font, or hinting bytecode.

## Stroke-width follow-up

The [stroke rounding experiment](font-stroke-fitting.md) implements explicit
stem widths and size-dependent rounding with `--strokes`, including the fitted
parameters and validation regressions.
