# Complete comparison conformance corpus

`conformance_preview` covers all 512 cases from the sibling comparison
repository's completed ZD621 203-DPI capture, firmware V93.21.33Z.
`original-capture.json` preserves its provenance, original source/image hashes,
preview reset and repeatability check. `corpus.json` is the source manifest
whose SHA-256 is recorded by that capture. Tests pin both manifests and the
exact reset bytes. Each candidate is rendered after the captured reset, using
ZD621_203_DPI, and compared at the original origin without alignment or resizing.

There are 453 unmodified original source/PNG pairs here. The other 59 manifest
entries reference `../barcode-aligned-zd621-v1/`: these are the independently
recaptured 832-dot barcode cases, replacing 812-dot sources that triggered
printer preview width adjustment. Old mismatched-width PNGs are not imported.

The gate checks every source hash, printer PNG hash, exact directional paint
counts, and resulting raster hash. All 490 nonblank exact frames must remain
pixel-exact. Nine blank native controls must remain blank. One invalid six-digit
UPC-E request has a blank native preview and must retain its precise renderer
error; it is not counted as a successfully rendered barcode.

The remaining twelve frames differ only in text. `text-regions.tsv` identifies
51 independently checked fields: ten labels containing one text field each,
36 numbered geometry labels, and five shipping-label text fields. The latter's
reversed lettering uses white ink inside its black box, preventing background
black from inflating its score. Every field must have nonempty foreground and
at least 80% foreground IoU (the current minimum is 93.33%). Every pixel
outside those regions must match
exactly, in addition to the exact directional-error and raster-hash gates.
Thus changes cannot hide a non-text regression within a high aggregate score.
Geometry regions are 24-dot boxes at the source FO anchors; shipping regions
also appear in the independent shipping_fonts_preview suite. Single-field
labels contain only text, so their whole frame is the field's comparison area.

The renderer's 164-case `printer_accuracy` gate and all focused native controls
remain separate. This corpus does not claim coverage of every ZPL input or
unsampled font size. Unstable empty-QR diagnostics live in empty-qr-zd621-v1
and are explicitly not deterministic accuracy targets.

References: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^A pp. 60–61, ^FB pp. 186–187, ^FO/^FT pp. 201/205, ^FR p. 203,
^TB pp. 356–357, and the command and argument lists in the preserved corpus.
