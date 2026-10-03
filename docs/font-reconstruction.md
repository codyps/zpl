# Resident font reconstruction study

The [automatic reconstruction pipeline](automatic-font-reconstruction.md) implements
curve fitting, inferred hints, frozen export and independent acceptance checks.

## Result

The 32-dot bitmap is exact only in its measured context. It does **not** contain
sufficient information to render arbitrary sizes exactly. A separate strike at
20 or 64 dots reproduces held-out text exactly, but scaling a 128-dot strike back
to 32 dots still differs by 711 pixels across 16 isolated glyphs.

An [initial outline and rasterizer parameter fit](font-fitting.md) is now
implemented. It improves some cases but does not generalize to unseen normal
sizes in aggregate. A neural network is not yet justified. An exact original TrueType file has not been
extracted, and no inferred vector font or learned model has been embedded.

## Capture method

On 2026-09-14, the available ZD621 (203 DPI) produced 60 sequential HTTP previews:

- Natural-width heights: 12, 16, 20, 24, 31, 32, 33, 48, 64, 96, 128 dots.
- Independent height/width pairs: 32/16, 32/48, 64/32.
- All four orientations: N, R, I, B, plus a repeat of 32/0 in all orientations.
- Sixteen isolated glyphs: space, A, g, j, Q, M, W, i, l, 1, O, 0, @, %, &, |.

Each glyph occupies a 192-dot tile on a 768-square canvas, with an explicit
`^FT` baseline. Rotated requests transform **edge coordinates**; inverse pixel
rotation uses `cell_size - 1`. Confusing these conventions produces spurious
one-pixel shifts. Measurements do not search for an alignment or ignore edges.
The tool checks canvas size, blank tiles, missing ink, and tile boundaries.
Inputs are decoded in strict black/white mode, with no threshold hiding gray.

Raw requests, PNGs, source hashes, and reports are in
[`font-study/`](../zebra-http-api/tests/fixtures/font-study/). Large colored diff
PNGs are regenerated locally. `report.json` compares printer output against the
current 32-dot renderer and compares normalized printer rotations against N.
`scaling.json` tests nearest-neighbor resampling from 96- and 128-dot captures,
anchored at the same baseline. These are bitmap baselines, not vector fits.

## Measurements

Counts below are differing pixels across the 16 isolated glyphs. Rotation
columns compare printer captures to the printer's own N image, not our renderer.

| Height / width | Local vs printer N | Printer R vs N | I vs N | B vs N |
| --- | ---: | ---: | ---: | ---: |
| 12 / default | 253 | 0 | 1 | 2 |
| 16 / default | 354 | 0 | 0 | 0 |
| 20 / default | 369 | 6 | 12 | 7 |
| 24 / default | 501 | 2 | 2 | 1 |
| 31 / default | 659 | 3 | 4 | 3 |
| 32 / default | 0 | 2 | 3 | 4 |
| 33 / default | 596 | 11 | 19 | 8 |
| 48 / default | 1,712 | 5 | 4 | 3 |
| 64 / default | 3,499 | 5 | 10 | 6 |
| 96 / default | 6,724 | 18 | 21 | 20 |
| 128 / default | 11,885 | 31 | 38 | 19 |
| 32 / 16 | 573 | 5 | 5 | 4 |
| 32 / 48 | 1,144 | 13 | 20 | 9 |
| 64 / 32 | 685 | 6 | 11 | 6 |

The repeated 32-dot N preview is pixel-identical; repeated rotations have the
same residual counts. A larger source bitmap helps many sizes but not all:
128-to-64 scaling reduces mismatches from 3,499 to 1,329; 128-to-32 has 711
mismatches where the native 32 strike has zero. At 12 dots its ink IoU is 0.579,
versus 0.582 for the existing strike. There is no universal scale-only solution.

Separate complete 95-character captures at 20 and 64 dots pass held-out text
verification with zero mismatches. The 64-dot run exposed a sampling bug: a fixed
16-dot margin rejected the underscore touching that boundary. The margin now
scales with height; an original printer PNG guards the correction. These new
strikes are test assets, not additional renderer defaults.

## Can the original font be read?

Read-only printer directory inspection lists `Z:0.TTF` (125,904 bytes, P flag)
and separate Swiss 721 files (`E:TT0003M_.TTF` and `Z:TT0003M_.TTF`, 169,188
bytes). The filename makes `Z:0.TTF` a candidate for font 0; this is not a
verified mapping or identification of its font family. `Z:FONT0.FNT` is another,
smaller file and must not be assumed equivalent.

Anonymous FTP retrieval of `Z:0.TTF` returned 550. The documented read-only
`! U1 do "file.type" "Z:0.TTF"` request on TCP 9100 returned no bytes within the
read timeout. These attempts did not retrieve outlines and do not prove that
every supported export route is unavailable. No font copying, deletion,
configuration changes, or physical print requests were performed.

Zebra's [Link-OS release notes](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/release-notes/Link-OS-v6.2-ReleaseNotes.pdf)
identify `TT0003M_.TTF` as Swiss 721. It should not be substituted for font 0 on
that basis alone. The local [ZPL guide](zpl-zbi2-pg-en.pdf), printed pages 833
and 1582–1583, documents `file.type`, font baselines, and scalable fonts.

## Outline fitting and machine learning

An outline is only part of the target. TrueType applies grid fitting before
scan conversion; glyph instructions and shared programs can change coordinates
at each size or transformation. See Microsoft's
[TrueType fundamentals](https://learn.microsoft.com/en-us/typography/opentype/otspec190/ttch01)
and [glyph table](https://learn.microsoft.com/en-us/typography/opentype/otspec183/glyf).
Our rotation residuals are consistent with orientation-dependent rasterization;
they do not identify the printer's specific hinting algorithm.

Recommended experiment:

1. Capture much larger isolated glyphs (256–1024 dots, smaller batches) to constrain
   curves. The present extractor is limited to 128 dots and its two-column atlas
   can exceed printer width at large sizes; adapt geometry before that capture.
2. Trace contours and fit quadratic Bézier control points, preserving holes and
   extrema. Fit advances and bearings independently using sentinel probes.
3. Jointly optimize shared stem widths, baseline/x-height/cap-height alignment,
   rounding thresholds, overshoot, and dropout rules across multiple sizes.
   Rotation must enter the rasterizer before final pixel decisions.
4. Reserve sizes 31 and 33, stretched widths, and new strings as validation data;
   do not tune per-image offsets against these. Also validate FO/FT origins,
   repeated characters, punctuation, all ASCII glyphs, and multiple printers.
5. Score exact binary mismatches and directional ink counts. A smooth or
   supersampled loss can guide fitting, but final acceptance uses binary pixels.

This is an inverse-rendering problem with relatively few interpretable parameters.
A std-only Rust coordinate-search or finite-difference optimizer can be tried
first. [DiffVG](https://cseweb.ucsd.edu/~tzli/diffvg/) demonstrates fitting Bézier
geometry through differentiable rasterization; it is a research reference, not a
new dependency or proof of exact printer emulation. A neural predictor of hints
could be considered if the simpler model fails on held-out captures. It would
need substantially more training data and would still not establish recovery of
the original font program.

Finite binary observations do not uniquely determine original control points or
hint bytecode: multiple outlines can cover the same sampled pixels. Distinguish
**exact matches on a declared test domain** from **recovering the original TTF**.
Keep the renderer's approximation warning until broader evidence supports it.

## Reproduce

```sh
direnv exec . cargo run -p zebra-http-api --example font-study -- \
  --host http://printer.local/ _font-study
# Repeat with the same host/output and --resume, or --offline for cached inputs.
direnv exec . cargo test -p zebra-http-api --example font-study --example extract-font
direnv exec . cargo test -p zpl --lib
```

The Rust tool adds no external dependencies. It issues 60 previews sequentially
with a 500 ms minimum pause and 30-second request timeout. Use an idle preview
endpoint because the API shares its preview object. Authentication uses
`ZPL_USERNAME`/`ZPL_PASSWORD`; credentials are never saved. Existing requests and
derived outputs must match; new output directories are preferred after changing
the renderer. Diagnostic RGB PNGs currently occupy about 203 MiB per run.
