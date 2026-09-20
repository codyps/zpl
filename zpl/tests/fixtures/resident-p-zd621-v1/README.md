# ZD621 preset font P

59 unmodified ZD621 203-DPI V93.21.33Z HTTP previews. The original font-id-P
case comes from the 2026-09-19 conformance reference; the other 58 frames were
captured 2026-09-20 with serialized requests and five-second pacing. All
requests completed. No images were cropped, padded, resized or registered.
Every preview width is a multiple of 64. manifest.tsv pins source/native PNG
hashes, exact underpaint/overpaint counts and rendered pixel hashes.

57 frames are pixel exact. The two scaled rotation frames retain a total of
10 underpaint and 16 overpaint dots. Each of their separate FO/FT N/R/I/B
text fields is checked against the 80% foreground-IoU floor; the minimum
across all 24 rotation-control regions is 99.6610%. Original font-id-P,
default-family alphabet, omitted/zero dimensions and every sampled normal
orientation compose exactly. Native clipping at the label edge is retained.

P is a proportional preset with a native 20x18 matrix. Requested 32x24
quantizes to 40x18. Independently hinted 20x18, 40x18 and 40x36 strikes are
embedded, each with 95 printable ASCII characters. Other dimensions use the
closest strike and remain approximate. The mixed-font control also exposed a
font-0 20x18 gap (69.0% foreground IoU); a new complete strike makes it exact.
S–V remain unsupported, and extended encodings still need investigation.

The preset_font_fo_last_dot option selects the measured rotated FO pivot:
subtract the height magnification from the vertical matrix boundary and one
dot from the proportional advance. FT retains its unadjusted baseline origin.
The option is enabled in ZD621_203_DPI and disabled in SPECIFICATION.

Each font-* directory contains twelve extraction pages, an independent
verification preview, capture settings and the measured glyph metrics.
The zpl-font-extract preset integration test re-extracts every glyph offline,
reproduces the embedded binary byte-for-byte, and verifies the separately
captured composition. All 52 extraction/verification frames also pass through
the renderer's native-comparison test. No printer is needed by the tests.

References: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
Table 31 p. 1584, ^A pp. 60–61, ^CF p. 154, ^FO p. 201, ^FT p. 205.
