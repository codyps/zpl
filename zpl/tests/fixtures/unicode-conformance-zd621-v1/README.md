# ZD621 Unicode and formatting controls

Unmodified previews from ZD621 203 DPI, V93.21.33Z. The manifest pins each
submitted source, native PNG, and local raster hash, with zero underpaint and
zero overpaint across 18 frames. Seventeen contain ink. `unicode-cjk` is blank
on the printer and is retained as missing-glyph behavior, not successful CJK
text coverage. Seven `unicode-*` root cases come from the comparison corpus;
sampling pages and `formatting` are independent native controls.

The two sampling directories reproduce the supplemental 40x24 Font 0 assets.
Their capture JSON records the exact character lists. Zero-advance soft hyphen
and zero-width space are valid measured glyphs; blank or missing sentinel
captures still fail extraction. The zero-advance verification JSON preserves
the original 859-pixel mismatch from composing literal, isolated combining
marks. NFC composition now reproduces that same unchanged native verification
PNG exactly, while the submitted ZPL remains decomposed.

`formatting` compares ordinary text, narrow and wide `^FB`, and `^TB` with
soft hyphen and zero-width space. Plain text and `^TB` suppress both. Native
`^FB` displays soft hyphen and treats zero-width space as an ordinary space.
The independent `block_utf8_formatting_visible` compatibility option selects
this behavior; it is enabled by ZD621_203_DPI and disabled by SPECIFICATION.
Existing visible soft-hyphen glyphs from the legacy encoding remain available
for this behavior and automatic field-block hyphens.

References: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
`^CI` pp. 156–159, `^FB` p. 186, `^PA` p. 315;
[Unicode UAX #15](https://www.unicode.org/reports/tr15/), sections 1.1–1.2,
canonical equivalence and NFC. Normalization applies to decoded CI28 text;
barcode payload bytes are preserved. This does not implement compatibility
normalization or claim full Unicode line-breaking support.
