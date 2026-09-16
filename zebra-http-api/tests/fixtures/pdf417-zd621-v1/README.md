# PDF417 printer probes

These 30 ZPL requests and original PNG responses were captured on 2026-09-15
from `http://printer.local/`, ZTC ZD621-203dpi ZPL, firmware
V93.21.33Z. The 24 requests in `manifest.tsv` were captured during the PDF417
investigation **before** the encoder fixes. Six additional sizing probes in
`sizing.tsv` were captured while validating the automatic-layout fix.
They use the HTTP Preview Label operation, not physical printing.
Both manifests lock the request and response SHA-256 hashes.

`cargo test -p zebra-http-api --test pdf417_preview` requires exact nonblank
full-image parity for 29 probes. The remaining `tight` case deliberately exceeds
its fixed capacity: the printer is blank and our renderer must explicitly reject
it. The `tighttext` case fits only after text compaction and must match exactly.
The requested width is 832, avoiding the preview's centered canvas padding.
No alignment, cropping, scaling or mismatch tolerance is used by these tests.

Coverage: text; numeric lengths 4–10, 12 and 13; six/seven binary bytes;
mixed text/binary; automatic/zero/empty dimensions; omitted/explicit row height;
truncation; CR/LF escapes; fixed-size success/failure. The two-dot row-height
probe checks rendering only: rxing could not scan that undersized symbol.
The additional probes cover 30-, 100- and 260-character automatic layouts and
show that dimension selection uses nominal Y=3X, independently of the requested
row height. The 260-character case uses five columns and 28 rows even with
explicit heights of three/nine dots or an omitted height.

References: [ZPL guide](../../../../docs/zpl-zbi2-pg-en.pdf), printed pp. 79–82;
[USS PDF417](https://www.expresscorp.com/wp-content/uploads/2023/02/USS-PDF-417.pdf),
§§2.2.4, 2.5–2.6 and Appendix D;
[ISO/IEC 15438](https://www.iso.org/standard/65502.html).
Observed ZD621 compaction/sizing choices are compatibility targets, not general
PDF417 requirements. Decoder tests independently exercise additional payloads.
