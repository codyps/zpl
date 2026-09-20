# ZD621 resident F and explicit bitmap Code 128 captions

Forty-one raw ZD621 203-DPI previews, firmware V93.21.33Z, captured 2026-09-19.
Every frame requires exact full-canvas equality, including text. The manifest
pins input/capture hashes, zero underpaint/overpaint, and rendered pixel hashes.
Comparisons do not align, scale or crop either image.

Resident F was previously rejected. Twelve extraction pages cover all 95
printable ASCII characters; a separate composed-text verification supplies the
thirteenth source frame. Each glyph advances 16 dots (13 matrix plus three gap
dots), with a zero-based native baseline of 20. The packed asset hash and raw
extraction configuration, glyph metadata and verification report are preserved.

The 28 independent frames cover full ASCII at doubled dimensions, all four
orientations, FO/FT, left/right justification, unequal-axis and integer size
quantization, wrapping, CF defaults, and native/double/triple FT placement.
The existing bitmap compatibility options apply to F as well as A/B/C/D.

Code 128 controls cover explicit A/B/C/D/F captions above/below bars, FO/FT,
and all orientations. Additional A/F controls have captions wider than the
bars. Two measured departures are independently selectable:

- `barcode_reverse_interpretation_shift` moves explicit bitmap captions by one
  dot along the reversed I/B reading direction, as it does automatic captions.
  It leaves N/R and explicitly selected proportional font 0 unchanged.
- `code128_fo_uses_bar_width` fixes the I/B rotation pivot at the bar width,
  even when a caption extends beyond either bar edge. Without it, the complete
  field extent determines that pivot.

Both options are enabled in ZD621_203_DPI and disabled in SPECIFICATION.
These captures establish printable ASCII F coverage and these caption layouts;
they do not establish arbitrary encodings or other resident fonts. Separate
proportional-font exploratory captures still show rotated-glyph differences.

To reproduce the packed asset offline, copy `font-source` to a temporary
directory and run `extract-font` with `--font F --height 26 --width 13
--batch-size 8 --offline --verify-text 'Abc XYZ 0123 !?'`, the saved source host
and copied output path. Compare `font.zbf` to `zpl/assets/fontF-26-13.zbf`.

Reference: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
Table 29 (gap/baseline, p. 1582), Table 31 (203-DPI matrices, p. 1583),
^A pp. 60–61, ^BC p. 94, ^CF p. 154, ^FO p. 201, and ^FT p. 205.
Exact raster placement comes from the preserved printer controls.
