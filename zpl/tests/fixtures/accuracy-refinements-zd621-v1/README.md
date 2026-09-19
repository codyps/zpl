# Accuracy refinement controls

Captured 2026-09-19 UTC through HTTP Preview Label on
`http://printer.local/`, ZD621 203 dpi, firmware V93.21.33Z.
These are original printer response bytes, not physical scans. No labels were
printed. Sources explicitly reset layout state and use PW832 / LL400.
`manifest.tsv` pins input and response SHA-256; `accuracy_refinements.rs`
requires full-canvas pixel equality using the ZD621 profile, without alignment,
padding, cropping, or rescaling.

Six CODABLOCK F/E controls exercise lowercase, different column/row counts,
module widths 1–3, row heights 8–14, padding and FNC1 checksum contributions.
The printer ignores excess requested rows when columns are explicit. Two-row
requests whose data fits one row produced blank responses; these invalid inputs
are tested as renderer errors rather than accepted as correct barcodes.

Six field-block controls cover center, right and full justification, two block
widths and a hanging indent. Centering includes a trailing space only when it
fits with room remaining. When the trailing space cannot fit, even the final
justified line is stretched. These observations are selectable compatibility
behavior, not assumptions about every firmware.

Three shape/text controls verify that HTTP previews ignore positive/negative
label-top adjustments and inverted print orientation. These independent
compatibility options do not claim that physical printing ignores the commands.

References: bundled Zebra ZPL Programming Guide, ^BB pp. 90–93, ^FB pp. 185–187,
^LT and ^PO; AIM USS CODABLOCK F (row padding and separators).
