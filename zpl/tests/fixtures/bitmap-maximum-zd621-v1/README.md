# Bitmap maximum dimensions and automatic captions

These 19 unmodified ZD621 203-DPI previews contain 63 fields. Each source and
PNG is hashed in `manifest.tsv`, together with the renderer's pixel hash and
zero underpaint/overpaint gates. All captures used a single successful HTTP
submission with `Connection: close` and at least five seconds between requests.
The printer runs V93.21.33Z; the same device supplied the existing font controls.

`abcd`, `efgh` and `symbols` pair 10× with requested 11× resident fonts A–H
and GS. The printer caps both axes at 10×. `symbols` independently checks a
narrow G glyph, avoiding the wide W's overlap between columns in `efgh`.
`axes` varies width and height independently, supplies zero for an inferred
axis, and checks CF and inherited A dimensions. `origins` checks FO/FT with
all four rotations. `explicit` compares standalone Font A and explicit-font
Code 128 captions at requested scales 9, 10, 11, 12 and 16.

`automatic` checks those same scales for implicit Code 128 captions. A QR
command sets the shared module width before Code 128 replaces it within the
same field. Beyond 10×, glyphs and inter-character advances stop growing,
but the caption's centering width and seven-row ink bottom retain the
requested scale. Eight `caption-*` frames independently check two characters
in every rotation, above and below the bars. Four `edge-*` frames check a
short dash at the top/left edge in R/I, above and below, including its blank
bottom rows. All frames are pixel-exact.

`bitmap_font_maximum_dimensions` enables the measured behavior in the ZD621
profile. SPECIFICATION rejects A–H requests whose rounded magnification
exceeds 10×, but permits larger GS symbols. The GS command documents a wider
range than the native implementation. Preset faces P–V are unaffected.

References: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^A pp. 60–61 (1–10× bitmap dimensions and rounding), ^CF pp. 154–155,
^BC p. 94 (explicit interpretation font), ^FO/^FT pp. 201/205,
^GS p. 217 (0–32000-dot operands), and font matrices pp. 1582–1584.
