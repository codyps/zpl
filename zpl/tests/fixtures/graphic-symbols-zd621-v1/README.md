# ZD621 graphic symbols

Sixty raw ZD621 203-DPI previews, firmware V93.21.33Z, captured September
19–20, 2026. The manifest pins input/capture hashes, underpaint, overpaint and
rendered pixel hashes. No alignment, scaling or cropping is applied.

The native GS 24 by 24 strike contains 95 ASCII entries: A-E draw registered,
copyright, trademark, UL and CSA symbols. Every entry advances 26 dots;
the other 90 entries are advancing blanks. Thirteen source frames contain
12 extraction pages and an independent composed-symbol verification. The
extractor uses A as its sentinel because the ordinary vertical bar is blank.
Another thirteen source frames capture and independently verify font 0 at
24 by 24, used when a later field block selects text instead of symbols.

Thirty-four layout frames cover FO/FT, all rotations, right justification,
native/double/triple/unequal dimensions, omitted dimensions, CF/FW defaults,
all ASCII, FR/LR, and barcode/GS command ordering. GS after BC selects symbols;
BC after GS selects bars with the normal caption. FB after GS selects text
with the preceding dimensions. All sixty frames require exact pixel equality.
The ordinary text in `GS-blocks` also matches after applying the printer's upward rounding of justified word
positions, independently measured in `field-block-rounding-zd621-v1`.

`graphic_symbol_last_row_baseline` selects native row 23 rather than the
specified 3/4-height FT baseline. `graphic_symbol_ignores_justification`
selects the printer's observed treatment of FO/FT right justification.
Both are enabled in ZD621_203_DPI and disabled in SPECIFICATION. Existing
bitmap FT dot offsets independently control rotated and scaled origins.

The graphic-symbol asset uses the explicit `@` strike tag, separating it from
resident font S (`^AS`). Historical capture JSON retains the old S sampler
identifier as provenance; the raw ZPL and PNGs are unchanged. Only the packed
asset's face byte and its hash changed. The `graphic_symbols` extraction test
reproduces the asset from all twelve native pages and checks the independent
`ABCDEEDCBA` verification. New captures use `--font @ --height 24 --width 24`.

Reference: [Zebra ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^GS p. 217, Table 29 p. 1582, ^FB pp. 185–187, ^FO p. 201, ^FT p. 205,
and ^BC pp. 94–100. Unsupported inputs default to space per p. 1581.
Coverage is the captured ASCII face at 203 DPI, not all encodings or printers.
