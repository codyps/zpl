# QR manual Kanji encoding

Ten unmodified ZD621 203 DPI HTTP preview responses, firmware V93.21.33Z,
captured 2026-09-20 with serialized requests and five-second pacing. All requests
completed. PW832 avoids preview-width adjustment. Models 1/2 and error correction
L/M/Q/H cover one-character, two-character and longer Japanese payloads.
Two additional controls encode the worked examples from both Shift JIS ranges.
`^FH` supplies the original Shift JIS pairs under CI13, avoiding Unicode/font
conversion. Source ZPL and printer PNG hashes are pinned in the manifest.

[Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^BQ pp. 129–131 defines manual `K` input. ISO/IEC 18004:2000 §8.4.5 p. 24 and
Table 3 define 13-bit compaction and character-count widths. Unit tests use the
standard's two worked Shift JIS examples and reject malformed pairs.

Every unchanged frame now matches every printer pixel with automatic mask
selection. The second check explicitly disables automatic selection and uses
the BCH-validated captured mask to retain independent encoding coverage.
Raw hashes and zero paint-error baselines are pinned in the manifest. See
[mask selection evidence](../../../../docs/qr-mask-selection.md).
