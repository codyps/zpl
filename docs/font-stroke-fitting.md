# Stroke width and size-dependent rounding fit

The offline Rust fitter now has a `--strokes` experiment. It learns stroke-width
adjustments and rounding rules on top of the existing fitted contours, using the
same eight training sizes and validation captures as the
[outline experiment](font-fitting.md). It adds no external dependencies or
network requests. The renderer's embedded font is unchanged.

## Reproduce

```sh
direnv exec . cargo run --release -p zebra-http-api --example fit-font -- \
  --strokes _font-strokes
# Focus on a few captured glyphs:
direnv exec . cargo run --release -p zebra-http-api --example fit-font -- \
  --strokes --characters 'Ogil|' _font-strokes-small
```

Output directories must be new. Without `--strokes`, the original experiment is
unchanged. Results are saved in [stroke-model.json](font-strokes-v1/stroke-model.json)
and [report.json](font-strokes-v1/report.json). The new report adds per-glyph
`strokes` statistics and `stroke_mismatch` totals alongside the original baselines.
Selected diff PNGs from this mode compare the printer against the stroke model.
`outlines.svg` still shows the underlying unwarped source contours.

## Parameterization

The detector scans the 128-dot training strike along both axes, finding identical
thin ink intervals repeated on at least six scan lines. It retains up to three
non-overlapping intervals per axis, ranked by support. Widths must be 2–24 source
dots. This detects recurring axis-aligned strokes; it does not identify every
curved or diagonal stroke semantically.

Each glyph has six parameters per axis. For source interval `[low, high]` and
axis size `s` (`width` for X, `height` for Y):

```text
stroke_width = max(1, floor((high - low + delta) * s / 128 + bias + small / s))
stroke_start = floor(low * s / 128 + position_bias) + phase
```

| Parameter | Range | Meaning |
| --- | --- | --- |
| `delta` | −2 … 2 source dots | Adjust the underlying stroke thickness |
| `bias` | 0 … 1 | Threshold for rounding the scaled thickness |
| `small` | −16 … 16 | Additional size-dependent rounding bias, divided by `s` |
| `position_bias` | 0 … 1 | Threshold for rounding the stroke's location |
| `strength` | 0 … 1 | Blend between the original and rounded edges |
| `phase` | −0.5 … 0.5 output dots | Constant position offset |

All detected strokes on one glyph axis share its six parameters. Their source
locations, widths, and support counts are exported separately. A monotone
piecewise-linear mapping moves contour coordinates between the constrained
edges; points beyond the outer constraints translate by the nearest displacement.
If target intervals meet at small sizes, target knots are clamped to be
nondecreasing. Zero strength leaves the fitted outline unchanged.

The search tries coarse combinations of width adjustment, rounding thresholds,
and warp strength, followed by bounded coordinate descent. It refines two
starting strategies independently: a grid seeded at zero inverse-size bias and
a grid including biases −8 and +8. Both allow subsequent inverse-size refinement.
The lower **training** objective wins, with ties favoring the zero-seed trial.
This avoids forcing a worse local optimum just because more seeds were tried.
Neither seed selection nor optimization reads validation pixels.

## Results: 2026-09-14

All 15 study glyphs were fitted. Original initializer, outline-only, and old
bounding-box-hint results reproduced exactly. The size-balanced training loss
fell from **0.14199 to 0.08443**, a **40.5% reduction**. Raw pixel totals expose
important regressions:

| Evaluation group | Outline-only baseline | Stroke rounding |
| --- | ---: | ---: |
| Eight training sizes, normal orientation | 4,153 | 4,503 |
| Held-out normal sizes: 20, 31, 33 | 1,445 | 1,682 |
| Held-out normal stretched widths | 2,035 | 1,841 |
| Rotations, 42 cases | 22,978 | 24,155 |
| All validation cases | 26,458 | 27,678 |

The objective averages per-size mismatch divided by reference ink, so gains on
small glyphs can outweigh more raw errors on large glyphs. For example, total
12-dot errors fell from 224 to 111, while 128-dot errors rose from 257 to 839.
Stretched-width errors improved **9.5%**, but unseen normal-size errors worsened
**16.4%**. This model is not suitable as a general renderer replacement.

There is a useful narrow success: lowercase **`l` matches all eight training
sizes and all three held-out normal sizes exactly**. Its learned X parameters are
`[-0.5, 0.625, 0, 0.5, 0.5, 0]`. The vertical bar matches 31 and 33 dots but
still differs by 40 pixels at 20 dots. Thus a rule fitting several sampled sizes
can still fail at another size.

The selected models retain a nonzero inverse-size bias (+8 on X) for `&` and
`g`; other glyphs retain zero. These coefficients are fitted approximations,
not identified original hint instructions. The same validation captures have
been examined in earlier experiments; they are held out from optimization,
not an untouched independent benchmark.

## Limits and next experiment

The fit confirms that explicit stroke rounding can recover some behavior, but
it does not recover a complete font or a reliable general hinting system. Shared
axis parameters may couple unrelated strokes, and a 128-dot bitmap does not
reveal the original subpixel geometry. Advances, kerning, baseline alignment
zones, orientation-specific rules, and unsampled glyphs are still outside this
model. It is an alternative to the earlier bounding-box hints, not their sum.

A stronger follow-up would fit individual stem constraints and vertical alignment
zones from substantially larger captures, then evaluate on fresh sizes and text.
The present fit is retained as a reproducible negative result for aggregate
validation accuracy, alongside its exact single-glyph cases.

## Validation

```sh
direnv exec . cargo test -p zebra-http-api --example fit-font
direnv exec . cargo clippy -p zebra-http-api --example fit-font
```

Seven tests pass. Tests cover hand-calculated width transitions at different
sizes, monotone coordinate warps, zero-strength identity, synthetic training and
held-out recovery, contour topology, optimizer loss monotonicity, and agreement
with the generic path rasterizer. Clippy reports only the pre-existing warnings
in `zpl`; it reports no new fitter warnings.
