# Resident-font accuracy controls

New captures: 2026-09-19 UTC, HTTP Preview Label at
`http://printer.local/`, ZD621 203 dpi, V93.21.33Z. No printing.
The 20/64-dot font-0 strikes reuse the original September 14 font-study captures
in `zebra-http-api/tests/fixtures/font-study`, including their separate held-out
verification strings. Other strikes were captured with the repository's
`extract-font` example. Each directory includes capture settings. Source and
original PNG hashes are pinned by the manifests; embedded ZBF hashes are in
`assets.json`. Captures are not normalized, cropped, or replaced by local renders.

The suite checks 118 exact full-canvas comparisons: all printable ASCII sample
pages for font 0 at 16/20/32/64-dot configurations, resident A/D native strikes,
a 32-dot `é` supplement and held-out verification strings, plus twelve Code 128
caption controls. The 32-dot font-0 configurations include widths 16, 24 and 64;
the existing natural-width 32-dot strike retains its original regression tests.

Caption controls vary module widths 1–3, above/below placement, and default versus
explicit font selection. The default caption uses resident A enlarged by the
module width, centered by its full advance. The six/eight-dot interpretation gaps
are independently selectable printer compatibility behavior.

Another 34 origin controls cover right/automatic justification, ^FO/^FT,
all rotations, descenders, scalable/bitmap fonts and native/enlarged strikes.
`justification.tsv` pins separate under/overpaint counts and raster hashes.
The maximum residual is four dots: font-0 outlines rasterized in a rotated
orientation differ slightly from rotating the captured normal strike. Every
control exceeds 99.3% ink IoU; tests require at least 80% in addition to the
exact residual counts. Inverted right-justified ^FO ink margins are a separate
compatibility option. No registration or whole-white-canvas score is used.

References: Zebra Programming Guide ^A pp. 27–28, ^BC p. 94, ^FO p. 201,
^FT p. 205, and resident font matrices/baselines p. 1582. Fonts A/D use integer
matrix magnification; their one-based baselines become zero-based rows 6/13.
^CI27 supplies Windows-1252 Latin-1 bytes; ^CI28 supplies UTF-8. The supplement
contains é only and does not claim complete Latin-1 or Unicode coverage.

These are discrete raster strikes, not recovered outlines or printer font files.
Unsampled scalable sizes still use the original 32-dot strike as a fallback.
