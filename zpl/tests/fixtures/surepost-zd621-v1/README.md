# SurePost label, MaxiCode and native font evidence

Zebra ZTC ZD621-203dpi ZPL, firmware V93.21.33Z, 203 DPI. These are
unmodified HTTP Preview Label responses, not physical prints or scans.
`provenance.json` records the original comparison capture and fresh capture
session, state reset, and the local transport bridge used for font sampling.
No print or firmware operations were performed.

`label.zpl` and `label.png` are the exact normalized 832×1524 comparison inputs
from `zpl-comparison/benchmarks/accuracy/external-reference`, originally based on
[trevordcampbell/zpl-toolchain's SurePost example](https://github.com/trevordcampbell/zpl-toolchain/blob/3da58518c1013fffd46d2147b0927a1d5b4aeab5/samples/usps_surepost_sample.zpl).
The upstream MIT license is retained in `UPSTREAM-LICENSE.txt`. The comparison
normalization removes MN/MF/MC, sets PW832 and LL1524, and retains the label
content. This does not claim support for the untouched configuration commands.
The old capture's submitted bytes also included the reset recorded in provenance;
`label-repeat.zpl` includes that reset explicitly and produces identical pixels.

The full label improved from 81.7965% foreground IoU (30,760 underpaint and
25,790 overpaint pixels) to 100% (zero of either). Its 284,865 ink pixels match
at their original origin, with no padding, alignment, cropping or rescaling.
The isolated MaxiCode and its repeated capture are also pixel-exact.

`maxicode_printer_run_boundaries` enables the sampled ZD621 run choices.
It is disabled in SPECIFICATION and ZQ610_PLUS_203_DPI (no matching ZQ610
capture evidence). The sampled encodings end set-B runs at characters
available in A. In particular,
`>` followed by RS needs a one-character shift, not a latch and a later return.
A full secondary message must not acquire an extra padding latch. Seven fresh
controls cover shared RS, GS, space and comma, another B run after RS, and
84-codeword capacity ending in either A or an E shift. They are all exact.
Independent decoder tests cover full capacity and overflow in modes 2 and 4
under both specification and ZD621 profiles. If the native run heuristic
exceeds capacity, the encoder also tries runs containing shared set-B characters
before rejecting the payload. Independent decoder tests cover forty alternating
pairs for all eight shared characters, in modes 2 and 4 under both profiles.

Ten complete 95-character ASCII Font 0 strikes were sampled at the label's
native dimensions. Each has twelve sampling pages and an independent composed
verification string, all pixel-exact. Extraction tests reconstruct every embedded
ZBF from those raw pages. The largest size uses one column to fit the native
canvas; the others use two. Generated fonts are in `zpl/assets`; no label-specific
glyph substitution or reference-image lookup is used.

Ten additional holdout pages exercise Wj at all ten sizes with FO/FT and all
four rotations (80 fields). Six pages retain minor rotated-glyph differences;
the manifest pins every directional count and raster hash, and every individual
field must exceed 95% foreground IoU. These residuals do not affect the exact
SurePost label. All 151 frames pin source and PNG hashes; every frame except
those rotation holdouts must be pixel-exact. Blank images are rejected.

References: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^A p. 60, ^BD pp. 106–108, ^FO p. 201 and ^FT p. 205;
[ISO/IEC 16023:2000](https://www.iso.org/standard/29835.html), character sets
in Annex A and run-based switching/padding in Annex F.

Run `cargo test --locked -p zpl --test surepost_preview` and
`cargo test --locked -p zpl-font-extract --test surepost_fonts`.

The new strikes contain ASCII. Missing Font 0 glyphs retain the enriched
32-dot fallback face and its original scaling, including Unicode, default
and legacy control glyphs. Legacy backslash also uses the fallback unless a
native replacement was sampled. Mixed text keeps native ASCII metrics and
composes fallback ink in output coordinates. Unit tests cover all ten sizes
and encoding/default-glyph variants; these approximations are not new native
accuracy claims for extended characters.

Font extraction commands and extraction-only tests in this document now run
from the separate [private font research repository](https://github.com/codyps/zpl-font-extract).
The runtime assertions and captured bytes remain in ZPL.
