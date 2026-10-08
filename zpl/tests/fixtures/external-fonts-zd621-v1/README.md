# Supplied TrueType baseline regressions

These are unchanged native canvases from the controlled-font capture in
[codyps/zpl-comparison](https://github.com/codyps/zpl-comparison/tree/main/references/font-controlled).
`manifest.json` pins the comparison commit, font and input hashes, original
submission hashes, printer identity and repeated/restored controls. The captures
use a ZD621, 203 DPI, V93.21.33Z; they do not establish other-device parity.

`ComparisonHerosCondensedBold.ttf` is the exact renamed quadratic conversion of
TeX Gyre Heros Condensed Bold supplied to that printer and the comparison adapter.
Its GUST license accompanies it. Conversion provenance and source hashes are in
`benchmarks/fonts/sources.json` and `benchmarks/fonts/build.py` at the pinned
comparison revision. It has 1000 units per em, an hhea ascender of 1105, and
OS/2 cap height of 718. Neither metadata height is the ZPL cell baseline.

The original `.zpl` files are rendered with the font registered as ID 0, as in
the comparison adapter. `.submitted.zpl` preserves the exact printer submissions,
including aliases to the previously uploaded fonts. No device requests are made
by these tests. The saved session verified repeat and restoration pixel controls.

The 16/32/64-dot probes establish the removed 6/11/23-dot baseline displacement.
17/33/65-dot holdouts exercise whole-dot FO baseline flooring; rotations and
descenders cover positioning beyond the reported example. Comparisons use the
full original canvas and origin, with exact underpaint/overpaint/shared-ink
counts and raster hashes. Rasterization and advance differences remain: these
are pinned residuals, not full-image parity claims. In particular, the 16-dot
example improves from 12.93% to 50.14% foreground IoU; its baseline is corrected
but glyph spacing still differs. The 32- and 64-dot examples reach 96.69% and
99.13% IoU respectively.
