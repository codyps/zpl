# ZD621 preset fonts Q and R

33 unmodified ZD621 203-DPI V93.21.33Z HTTP previews. The original font-id-Q/R
cases were copied from the complete 2026-09-19 conformance capture after
checking its source/PNG hashes. The other 31 frames were captured 2026-09-20
with serialized requests and five-second pacing; every request completed.
Images were not resized, padded, cropped or registered. Every preview width
is a multiple of 64. manifest.tsv pins source and native PNG hashes, exact
underpaint/overpaint counts and rendered pixel hashes.

31 frames are pixel exact, including both original cases, the independent
alphabet controls, all extraction pages and composition checks. The rotated
origin frames retain eight underpaint and ten overpaint dots in total. Each
of 16 isolated FO/FT N/R/I/B regions exceeds 80% foreground IoU; minimum
99.7555%. Native clipping at label boundaries remains part of the comparison.

Q and R have proportional native 28x24 and 35x31 matrices. Their measured
baselines are 22 and 28 respectively. Using font 0's three-quarter baseline
misplaced their ink and left the original cases below 80%. Both A and CF now
select these actual resident faces, with the existing bitmap size quantization.
Each strike contains all 95 printable ASCII characters. Uncaptured sizes are
scaled approximations; this suite does not claim complete size/encoding support.

The existing preset_font_fo_last_dot option controls measured rotated FO
placement. Q, like P, subtracts the height magnification from its vertical
pivot. R retains the vertical matrix boundary. Both subtract one dot from the
horizontal proportional advance. FT retains its baseline origin. The option
is enabled in ZD621_203_DPI and disabled in SPECIFICATION; baseline metrics
belong to the sampled faces in both profiles.

Each font-* directory holds twelve sampling pages, an independent composed
verification, capture settings and measured glyphs. zpl-font-extract's
preset_qr integration test re-extracts all glyphs, reproduces the two embedded
assets byte-for-byte, and checks composition against the independent native
image. All 26 sampling/verification frames also run through the renderer's
printer comparison test. The tests need no printer or network.

References: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
Table 31 p. 1584, ^A pp. 60–61, ^CF p. 154, ^FO p. 201, ^FT p. 205.
Table 29 p. 1582 omits the preset baselines; these are measured from independent
normal FO/FT controls, then validated with alphabet and rotated holdouts.

Font extraction commands and extraction-only tests in this document now run
from the separate [private font research repository](https://github.com/codyps/zpl-font-extract).
The runtime assertions and captured bytes remain in ZPL.
