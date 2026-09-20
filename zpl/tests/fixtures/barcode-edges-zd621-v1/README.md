# ZD621 barcode ink at negative edges

Twenty-five raw HTTP preview frames from ZD621 203 DPI, V93.21.33Z,
captured 2026-09-19. Every frame must match the entire 832 × 1218 canvas
with zero underpaint and overpaint. The manifest pins the source ZPL, raw
printer PNG and rendered pixel hashes. No alignment, cropping or scaling
is applied.

The controls cover Code 11, Code 39, Code 93, Code 128 and Interleaved 2 of 5.
The printer clamps the bar group's actual ink bounds and each interpretation
glyph's actual ink bounds independently at negative edges. It does not clamp
the combined field or the nominal font cell. Dots and dashes distinguish these
rules; Code 11 offsets include both overlapping and unclipped captions.
Black ink is combined by union. With ^FR or ^LR, overlapping components invert
the existing image twice, as the independent reverse-print controls demonstrate.
Further controls cover ^FO/^FT, all four orientations, ^LH and ^LS.

`linear_barcode_clamps_negative_ink` selects this departure from nominal field
placement. It is enabled by ZD621_203_DPI and disabled by SPECIFICATION.
The implementation is limited to the five captured families. Other families
need independent controls before extending the override.

Related regression: `linear-caption-zd621-v1/above-linear-caption-B-0` formerly
had 1146 underpaint and 207 overpaint dots and now matches exactly. The separate ^FO R origin-zero bar boundary is resolved by the controls
in `barcode-boundary-zd621-v1`, which also preserve a newly measured short-glyph
caption gap. These controls do not claim full barcode or text coverage.

Reference: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FO p. 201, ^FT p. 205 Table 7, ^FR p. 203 and ^LR p. 295. Exact edge and
reverse-overlap behavior is established by the preserved hardware previews.
