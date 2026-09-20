# ZD621 linear barcode FT boundaries

Twelve raw ZD621 203-DPI HTTP preview frames, firmware V93.21.33Z,
2026-09-19, cover 72 fields: eighteen linear barcode variants in each of
N/R/I/B orientations. PW832 avoids preview-width adjustment. No reference
is cropped, shifted, or rescaled. The manifest pins source/response SHA-256,
zero underpaint/overpaint, and the complete local pixel SHA-256.

Each orientation has three atlases, with six fields each in this order:

- 0: Code 11, Code 128, Code 93, Codabar, Industrial 2 of 5, Interleaved 2 of 5.
- 1: LOGMARS, MSI A/B/C/D, PLANET.
- 2: Plessey, ^BZ PLANET, POSTNET, Standard 2 of 5, UPC extensions 2/5.

Payloads/parameters are adapted from `zpl-comparison/test-data/render-conformance/cases/barcode-families/symbol-*.zpl`.
Only canvas/origins/orientations are changed; LOGMARS retains its mandatory
caption, and other fields have optional interpretation disabled.

The printer places the last bar row on the ^FT baseline in N/B orientations,
where the former renderer anchored the exclusive lower boundary one dot early.
R/I controls already matched and remain exact. The same measured rule also
holds for Code 39 and UPC/EAN in their separate caption fixture suites.
`linear_barcode_ft_uses_last_bar_row` selects this shared departure and replaces
the earlier Code-39-only and retail-only options. It is enabled in the printer
profile and disabled in SPECIFICATION. Matrix/stacked symbols and other ^BZ
variants have not been inferred from these samples.

Source: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FT p. 205 Table 7 and the referenced ^B command descriptions. The captures
establish the raster-boundary convention that is not explicit in the table.
