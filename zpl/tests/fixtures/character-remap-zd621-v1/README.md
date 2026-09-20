# ZD621 legacy character remapping

Four unmodified native ZD621 203-DPI V93.21.33Z frames pin zero underpaint and
overpaint. `encoding-remap` is the original comparison-corpus case, checked
against the complete reference manifest. Three new controls cover persistent
per-encoding tables, duplicate destinations, nonrecursive mapping, space and
euro images, Code 128 captions, and UPC captions with resident A and OCR-B.
All new controls restore the character mappings they touch before ending.

CI pairs are output-image source followed by input-character destination.
CI0 and CI13 retain separate tables across fields and encoding switches.
Later pairs replace earlier entries for the same destination. CI27/28 accept
but ignore remapping pairs. Glyph selection applies mappings after field text
processing; barcode data stays unchanged while captions select mapped glyphs.
This preserves the distinction between changing character images and changing
field-block delimiters or barcode payloads.

The guide says space cannot be remapped, but this ZD621 Font 0 maps destination
32 to the requested image. `remap_space` controls that departure: false in
SPECIFICATION, true in ZD621_203_DPI. The documented legacy image 21 selects
the euro symbol. Higher legacy source images without an established mapping
are rejected, rather than silently interpreted as Latin-1. Other legacy CI
encodings and unsampled resident-font glyphs remain outside current support.

Initial exploratory captures revealed that mappings survive HTTP preview
requests. The committed controls explicitly initialize the entries they use;
exploratory outputs without that initial state are not independent references.
The positive-origin retail control separates remapping from an additional
observed off-label UPC-caption placement gap, which remains to be fixed.

Reference: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
`^CI` pp. 155–159, especially the legacy-only remapping restriction on p. 155,
source/destination definitions on p. 157, euro example on p. 158, and the space
restriction on p. 159. Per-case JSON and the manifest pin submitted sources
and successful native PNG captures; no reference image was aligned or edited.
