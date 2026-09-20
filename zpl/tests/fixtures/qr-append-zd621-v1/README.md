# QR structured append and mixed manual input

Eighteen unmodified ZD621 203 DPI HTTP preview responses, firmware V93.21.33Z,
captured 2026-09-20. All requests completed sequentially with five-second pacing.
PW832 avoids preview-width adjustment; CI13/FH preserves input bytes.

[Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^BQ pp. 129–134 defines the optional `Diijjxx,` envelope and up to 200 manual
segments. ISO/IEC 18004:2000 §9 pp. 55–56 defines the structured-append header:
mode 0011, zero-based symbol index and total, followed by eight-bit parity.
The caller supplies parity for the entire message; one field cannot recompute
it from only its own portion. No automatic division into multiple symbols is
performed.

The controls cover Models 1/2, automatic input, mixed numeric/alphanumeric/byte/
Kanji segments, adjacent segments of the same mode, literal commas in automatic
input, counted byte data containing commas, sequence endpoints, and 180/200
segments. Byte counts delimit payloads before comma separators are interpreted.
The 180-segment Model 1 control reaches version 14 and matches the printer.

All eighteen frames have raw source/PNG hashes, unchanged-input
underpaint/overpaint counts and local pixel hashes pinned in `manifest.tsv`.
Every one matches every printer pixel when only the mask operand is replaced
by the BCH-validated mask decoded from the capture. Automatic mask selection
remains an open gap; the unchanged-input comparisons keep it visible.

The `m1-segments-200` frame originally exceeded the supported Model 1 limit.
The ZD621 emits a version-15 symbol for it. The independent
`qr_model1_extended_versions` option now permits captured versions 15–40;
its complete block-table controls are in `qr-model1-extended-zd621-v1`.
The specification profile retains the Annex M version-14 limit. The original
printer response is now included in the successful encoding comparisons above.
