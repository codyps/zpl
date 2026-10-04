# Expanded font sampling experiment

This follow-up tests whether more observations improve the
[joint outline/hint optimizer](joint-font-optimization.md). It keeps the original
six glyphs (`H O S g j @`), automatic seed, objective, three search rounds,
beam width/depth, and acceptance rules. The independent variable is the amount
and variety of development data, including additional large outline constraints.

## Sampling design

[`expanded_font_probe.py`](../zpl-font-extract/scripts/expanded_font_probe.py)
predeclares the complete design before capture. The
[plan](../zpl-font-extract/tests/fixtures/expanded-font-20261003/plan.json)
contains **1,536 glyph cases across 256 distinct configurations**:

| Family | Configurations | Design |
| --- | ---: | --- |
| Square | 71 | Previously unused sizes from 8 through 120 dots |
| Stretched | 129 | Height 10–96; adjacent widths, 3/4 and 4/3 ratios, half and double width |
| Rotated | 47 | R/I/B at selected sizes from 10 through 96 dots |
| Large | 9 | Additional outline constraints and separate validation sizes |

The plan excludes every configuration in the six preceding campaigns, including
their old development and validation pages. It reserves every fifth new square,
stretch and rotation configuration for final validation before reading any new
pixels. Large development sizes are 128, 192, 352 and 480; final large validation
uses 144, 208, 304, 400 and 464. All data remain specific to ZD621 D7J211001302,
203 DPI, firmware V93.21.33Z.

There are **1,212 new development cases** and **324 final validation cases**.
Adding the original 108 development cases gives 1,320 available for fitting and
its internal check. The original internal split stays fixed; the new development
configurations receive their own every-third-configuration internal split.
Old internal checks therefore cannot move into search merely because new sizes
were added. The resulting search has 894 cases and the internal check has 426.

Each capture batch has at most 32 previews, including repeated controls, and
uses native 384- or 768-dot canvases. The design contains 124 resident-font
previews in total. Batches run serially and verify printer identity, firmware,
dimensions, hashes and repeated controls. Completed batches can be skipped when
continuing; a failed batch requires diagnosis and is not automatically retried.
Final-validation pixels are captured only after the new model is frozen.

## Fitting support

`optimize_font.py` now accepts `--extra-development`, `--extra-source` and
`--reserved`. Extra inputs must match the seed's printer environment and glyph
coverage, have no duplicate or reserved query, and keep ink away from tile edges.
Reserved manifests are read without opening their PNGs. The source/development
campaigns used by the seed are still required unchanged.

The exact-scoring cache reuses a glyph's large-size results across hint-program
changes only when the compiler's instruction guard disables hints on that
configuration. The conservative test is calibrated maximum ppem greater than
the fixed limit plus one; borderline cases are rendered normally. Every changed
outline is rendered again, and size, rotation and origin remain part of the
observation. An audit against uncached rendering reproduced every original
development score for both the seed and the first joint proposal exactly.
This avoids repeatedly rasterizing large unchanged outlines during hint search;
it does not substitute an approximate objective.

## Reproduction

The complete manifests and request bytes are retained with the plan. To continue
an interrupted collection of development batches:

```sh
uv run --offline --no-project --with freetype-py==2.5.1 --with pillow==12.3.0 \
  python zpl-font-extract/scripts/expanded_font_probe.py capture \
  zpl-font-extract/tests/fixtures/expanded-font-20261003 --group development
```

Pass the plan's development campaigns with `role: small` to
`optimize_font.py --extra-development`, its development `role: large` campaign
to `--extra-source`, and all old and newly reserved validation manifests to
`--reserved`. Use the same original seed/source/development positional arguments
as the first joint experiment. Frozen artifacts retain all source, engine and
script hashes. Final evaluation and generated-font previews use the existing
`reconstruct_font.py` commands.

## Results

The [frozen expanded fit](../zpl-font-extract/tests/fixtures/expanded-optimization-20261004/)
evaluated 4,329 distinct font/query batches. It retained 632 outline points,
accepted 12 coordinate moves, reduced the CVT cut-in from 24 to 16, changed one
shared horizontal width from 296 to 294 units, and selected 21 anchors, ten links,
two zone operations and two centered stems. The normal font is 6,312 bytes.
The 90-ppem instruction limit remains fixed, as in the first experiment.

Training objective decreased from 0.097727 to 0.080538 (17.59%). Unlike the first
joint proposal, the expanded proposal passed the internal check:

| Internal-check group | Seed mean error | Expanded mean error |
| --- | ---: | ---: |
| Square | 7.101% | 6.621% |
| Transformed | 12.760% | 10.266% |

These objectives and internal-check percentages use a different dataset from
the first experiment; their absolute values are not a direct comparison between
the two proposals. Both proposals were instead evaluated on identical external
captures below. The new fit received no changes after freezing.

### Newly reserved cases

Foreground IoU on native, unaligned canvases:

| Requested size range | Cases | Vector seed | Prior joint proposal | Expanded proposal |
| --- | ---: | ---: | ---: | ---: |
| Up to 32 dots | 96 | 84.963% | 86.329% | **87.074%** |
| 33–64 dots | 72 | 91.638% | 91.553% | **92.536%** |
| 65–90 dots | 66 | 94.021% | 94.123% | **94.295%** |
| Above 90, small/stretched family | 60 | 94.745% | 94.721% | **94.906%** |
| Large square outlines | 30 | 98.215% | **98.323%** | 98.290% |
| All small/transformed families | 294 | 93.742% | 93.792% | **94.114%** |

Ranges use the larger requested dimension, with square cases at least 128 dots
reported separately as large outlines. The small/transformed aggregate has 31
exact cases for the seed, 29 for the prior proposal, and 34 for the expanded
proposal. The largest improvement is at the small end: up-to-32 IoU improves
2.111 percentage points over the seed and 0.745 points over the prior proposal.
The large-outline aggregate improves over the seed but falls slightly below the
prior proposal.

Four of the six new capture batches pass the strict page-level seed gate. The
65–96-dot batch loses exact cases (four to two), and individual large pages also
regress. **The overall gate still fails.** Passing the internal check selects
this run's `font.ttf`; it does not promote it into production rendering.

### Earlier holdouts

The expanded proposal improves aggregate IoU over the prior joint proposal on
all six earlier campaigns:

| Earlier campaign | Vector seed | Prior proposal | Expanded proposal |
| --- | ---: | ---: | ---: |
| Large 320/448 | 98.293% | 98.168% | **98.368%** |
| Small, first campaign | 82.520% | 81.589% | **82.658%** |
| Small, second campaign | 83.884% | 84.093% | **85.962%** |
| Reconstruction holdout | **89.649%** | 87.670% | 88.776% |
| Joint small holdout | **90.043%** | 89.836% | 89.854% |
| Joint large holdout | 98.076% | 98.111% | **98.177%** |

Pooled earlier cases up to 32 dots improve from 80.951% for the prior proposal to
81.996% for the expanded proposal (seed: 80.451%). However, none of those six
campaigns passes the per-page gate. Additional observations reduced overfitting
and improved generalization overall, while leaving specific regressions to solve.

The full native-canvas and per-case counts are in
[evaluation.json](../zpl-font-extract/tests/fixtures/expanded-optimization-20261004/evaluation.json)
and [historical-evaluation.json](../zpl-font-extract/tests/fixtures/expanded-optimization-20261004/historical-evaluation.json).
[`compare_font_fits.py`](../zpl-font-extract/scripts/compare_font_fits.py) verifies
identical source provenance and seed results before computing
[comparison.json](../zpl-font-extract/tests/fixtures/expanded-optimization-20261004/comparison.json)
and [historical-comparison.json](../zpl-font-extract/tests/fixtures/expanded-optimization-20261004/historical-comparison.json).
It also requires per-case counts to account for every pixel of each native
canvas, so size grouping cannot hide unassigned foreground.

### Generated-font printer replay

The expanded proposal was uploaded under a fresh temporary RAM name, with two
state witnesses, and previewed on all 294 newly reserved small/transformed cases.
The [replay](../zpl-font-extract/tests/fixtures/expanded-replay-20261004/)
matches local rendering at **99.5367% IoU**: 499 underpaint and 712 overpaint
pixels over a 261,369-pixel union, including the witnesses. This is execution
accuracy for the same generated font, distinct from its Font 0 reconstruction
accuracy. On the 96 up-to-32 cases alone, same-font IoU is 97.792%; small-size
renderer differences remain measurable.

Comparing the **printer's generated-font previews directly with its resident
Font 0 previews**, on the same native canvases, yields 94.1053% IoU across the
294 cases (86.8054% on the 96 up-to-32 cases). The overall result is close to the
local prediction of 94.1145%. Full counts and both sets of PNG hashes are retained
in [printer-versus-resident.json](../zpl-font-extract/tests/fixtures/expanded-replay-20261004/printer-versus-resident.json).

The uploaded font was 6,440 bytes. Selection and instruction witnesses passed,
repeated controls matched, and removal was confirmed. Capturing resident samples
and replaying the generated font used 136 serialized previews in total and no
physical labels. Rust regressions preserve the ten new native-canvas residuals
and hashes alongside all previous baselines.

Validation passed 44 Python tests, 17 Rust TrueType/accuracy/conformance
integration tests, targeted strict Clippy, and formatting checks. The fonts
remain research artifacts; production font selection and its existing bitmap
strikes were not changed.
