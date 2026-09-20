# QR magnification changes the shared barcode module width

Four unmodified ZD621 203-DPI V93.21.33Z frames are pixel-exact. The original
`compact-state-qr-code128` conformance capture is verified against its source
manifest. Three new captures record single successful submissions and hashes.
No image alignment, cropping, or pixel editing is used.

`same-field` covers QR magnifications 1–4 followed by Code 128 before FD.
`next-field` renders each QR first, then a Code 128 field using the inherited
width. `holdouts-basic` covers omitted/empty magnification (203-DPI default 2),
Model 1, and an explicit BY reset. There are 17 rendered barcode fields across
these frames, including the four QR fields in `next-field`.

The independent `qr_updates_barcode_module_width` option is enabled by
ZD621_203_DPI and disabled by SPECIFICATION. It updates the shared module
width when BQ is processed, even when a subsequent barcode command replaces
that QR in the same field. BY can reset it. Ratio and height remain separate.

Reference: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
BQ pp. 129–134 and BY p. 148. BQ describes its magnification; the shared BY
state departure is established by these printer controls.
