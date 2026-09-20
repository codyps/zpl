# ZD621 resident H (OCR-A)

Forty-one raw ZD621 203-DPI previews, firmware V93.21.33Z, captured 2026-09-19.
Every frame requires exact full-canvas equality. The manifest pins input and
capture hashes, zero underpaint/overpaint and rendered pixel hashes. Comparisons
do not align, scale or crop either image.

The native 21 × 13 strike contains all 95 printable ASCII inputs. Space and
lowercase a–z have no ink but each advances 19 dots; the remaining 68 characters
have visible glyphs. Every character has the same advance, comprising 13 matrix
dots and six gap dots, with a zero-based native baseline of 20. The renderer
previously rejected H. The extractor also rejected non-space blank glyphs,
even when their nonempty sentinel probes verified a positive advance. It now
preserves such measurements while still rejecting invalid or blank previews.

Thirteen source frames preserve twelve extraction pages and an independent
composed-text verification. Twenty-four layout frames cover all ASCII,
CF/A omitted/zero dimensions, unequal scaling, integer size rounding, wrapping,
FO/FT, every rotation and justification, native/double/triple FT placement,
right-margin holdouts and explicit Code 128 captions above/below the bars.
The wide captions exercise the existing bar-width rotation-pivot option.
Two of these frames contain only lowercase letters and must remain entirely
white; every other frame must contain ink.

Four additional `blank-study` frames compare the complete ASCII sequence to
one with only lowercase letters replaced by spaces, with sentinel glyphs
bounding each row. Each printer image pair is identical at native and doubled
sizes. Lowercase is preserved as advancing blank data, not mapped to uppercase.

The existing bitmap-font sizing, FT, inverted-margin and caption compatibility
options match H; SPECIFICATION disables those departures. Blank glyphs belong
to the captured resident face and retain their measured advances in both
profiles. Coverage is ASCII at 203 DPI, not arbitrary encodings or resolutions.

To reproduce the packed asset offline, copy `font-source` to a temporary
directory and run `extract-font` with `--font H --height 21 --width 13
--batch-size 8 --offline --verify-text 'Abc XYZ 0123 !?'`, the saved source host
and copied output path. Compare `font.zbf` to `zpl/assets/fontH-21-13.zbf`.

Reference: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
Tables 29/31 (pp. 1582–1583), ^A pp. 60–61, ^BC p. 94, ^CF p. 154,
^FB pp. 185–187, ^FO p. 201, and ^FT p. 205. Blank lowercase behavior is measured
from these previews rather than inferred from the OCR-A name.
