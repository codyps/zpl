# ZD621 Code 39 interpretation controls

These 23 raw ZPL/PNG pairs were captured from a Zebra ZD621, 203 dpi,
firmware V93.21.33Z, through its HTTP preview interface on 2026-09-19.
The captures are unmodified. `manifest.tsv` pins input and capture SHA-256,
zero underpaint/overpaint, and the complete local raster pixel SHA-256.
`code39_caption_preview.rs` requires full-canvas equality using
`profiles::ZD621_203_DPI`; text is checked pixel-for-pixel too.

Coverage includes above/below interpretation in all four orientations,
Mod-43 checksum, empty data, explicit fonts A/0, module widths 1/2/3,
inherited ^FW orientation, and ^FT anchors including a one-dot bar height.
The printer includes start/stop asterisks and the enabled checksum in its
caption, ignores preceding explicit font selections for that caption, and
anchors normal/bottom-up ^FT symbols at the last bar row. Rotated/inverted
^FT controls retain the opposite bar boundary. Independent compatibility
options select those three observed behaviors; SPECIFICATION disables them.

Sources: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^B3 pp. 70–72, ^BY p. 148, and ^FT p. 205 Table 7. The raw captures establish
the firmware-specific caption and boundary behavior where the description
alone does not specify identical raster output.
