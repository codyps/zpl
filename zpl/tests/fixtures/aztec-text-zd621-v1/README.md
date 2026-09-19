# Aztec text, binary and sizing controls

Captured 2026-09-19 UTC through HTTP Preview Label on
`http://printer.local/`, ZD621 203 dpi, firmware V93.21.33Z.
No physical printing. PNG files are unmodified printer responses; `manifest.tsv`
pins their SHA-256 and their input bytes. Tests require complete-canvas exact
pixel equality with the ZD621 profile, without alignment, cropping or rescaling.

The 38 controls include upper/lower/digit/mixed/punctuation text, punctuation
pairs, shifts and latches, mixed binary/text, contiguous binary runs of 6–100
bytes (including 31/32/62/63 count boundaries), explicit percentages 10/23/50/80,
and explicitly selected compact/full layers. The two original corpus Aztec
aliases provide another independently captured payload/scale comparison.

Implementation references: ISO/IEC 24778:2008 §7.3.1.1 Table 2 (character tables,
shifts, latches, binary lengths), §§5(e), 11.2 and Annex G.2 (parity and sizing),
[ISO publication](https://www.iso.org/standard/41548.html); bundled Zebra ZPL
Programming Guide ^B0/^BO pp. 64/124.

The printer truncates fractional parity requirements for the default level,
while explicit percentages round up. It also uses one extended binary count
for 32–62 non-text bytes instead of two short shifts that save one bit. These
are separate compatibility options; specification rendering rounds up and may
split runs. Equal-cost text paths latch before shared/shifted characters when
that latch is needed later. Compact symbols take precedence when physical sizes
tie. There are no payload-specific encodings or fixture lookup tables.
