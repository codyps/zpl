# Font-0 28x14 automatic-hyphenation glyphs

15 unmodified ZD621 203-DPI V93.21.33Z previews, all pixel exact. Thirteen
original field-block cases were copied from the complete 2026-09-19 conformance
capture after verifying its source and PNG hashes. The two fresh sampling and
independent verification frames were captured 2026-09-20 with serialized requests
and five-second pacing. Both requests completed. All widths are multiples of 64;
images were not padded, resized, cropped or registered.

The existing 28x14 ASCII strike lacked U+00AD (soft hyphen) and U+00F0 (eth).
Automatic FB hyphenation therefore failed during measurement or painting. The
new supplement supplies both measured glyphs: the existing printer profile
measures the soft hyphen but paints eth under CI27, matching the independently
established encoding departure. No layout or compatibility rule changes here.

The native cases cover widths 1, 20 and 120 with L/C/R/J alignment, overflow
and a long unbroken word. All full-frame comparisons require zero underpaint
and overpaint. The manifest pins source/native PNG/rendered-pixel hashes.
An offline extraction test reproduces the two-glyph asset byte-for-byte from
the saved sampling page and verifies composition against the independent image.

References: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FB pp. 186–187 and ^CI pp. 156–159. See field-block-hyphenation-zd621-v1 for
the native CI27 eth behavior and its profile-controlled specification alternative.
