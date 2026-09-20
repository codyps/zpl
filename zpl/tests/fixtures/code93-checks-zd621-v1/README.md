# ZD621 Code 93 checksum interpretation

23 raw HTTP Preview Label frames from Zebra ZD621 203 DPI, V93.21.33Z,
captured 2026-09-19, contain 336 symbols. Every frame requires full-canvas
pixel equality: zero underpaint and overpaint, with pinned source, PNG and
local raster SHA-256. PW832 avoids preview-width adjustment; no reference
is aligned, cropped or rescaled.

Coverage comprises 94 controls exercising every C and every K checksum value
0–46, all 188 combinations of extended C values 43–46 with K values 0–46,
48 independent payload/size/orientation controls, and six original probes.
`checks.tsv` records the two exhaustive groups' field indexes, payload bytes
and expected C/K values. Holdouts use module widths 1/3, all orientations,
above/below captions, explicit font 0, and full-ASCII shift-pair input.

^BA's e=Y no longer rejects the symbol. C and K are calculated by the same
routine for bars and interpretation. Hidden interpretation never changes bar
encoding. The specification profile displays extended checksum values using
the documented ZPL substitutes (`&`, `'`, `(`, `)`).

`code93_extended_checksum_preview` selects the observed ZD621 departure:
when C is extended, the printer applies shift formatting using K as lookahead
and then displays K again. Control/error branches instead display a solid
cell and three repeated tail cells, omitting the stop delimiter. The mapping
is defined by checksum symbol classes, not by payload lookup. Native control
cells 19–25, the cent-like cell at 92, and the quote at 96 are taken from the
captured resident interpretation glyphs. The familiar Code 11 triangles and
Code 93 box occupy cells 22–24. This option is enabled in ZD621_203_DPI and
disabled in SPECIFICATION; it does not modify encoded bars or checksums.

The raw references establish every extended C/K combination independently
of the implementation, and the holdouts verify new payloads, integer glyph
scaling and rotation. These results do not resolve unrelated QR mask or
left-edge caption positioning gaps.

Sources: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^BA pp. 87–89, especially parameter e on p. 88 and the full-ASCII substitute
tables on pp. 88–89. Exact malformed control/error output and glyph bitmaps
are empirical printer-profile behavior, not claims about the specification.
