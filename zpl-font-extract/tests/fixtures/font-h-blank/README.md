# Advancing blank OCR-A glyphs

Raw ZD621 203-DPI preview, firmware V93.21.33Z, captured 2026-09-19.
The native resident-H extraction page requests ASCII 96–103. The grave accent
has ink; lowercase a–g have no ink but each advances 19 dots. Independent
sentinel probes measure every advance and reject an entirely blank preview.
The unit test checks that extraction and ZBF packing preserve these glyphs.

Zebra's ZPL Programming Guide Tables 29/31 (pp. 1582–1583) describe native H
as OCR-A, 21 × 13 with six gap dots. Blank lowercase behavior is measured from
this preview, not inferred from the character names.
Source: https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
