# Automatic field-block hyphenation

Sixty-one raw ZD621 203-DPI V93.21.33Z previews captured 2026-09-20.
Input/capture hashes, underpaint, overpaint and rendered-pixel hashes are pinned.
No alignment, scaling or cropping is applied. Previously, overlong words caused
an unsupported-rendering error. Fifty-nine complete frames now match exactly;
two font-0 B layouts retain three underpaint dots each and zero overpaint.
Each of the four aligned fields independently requires at least 80% foreground
IoU. Integer centering before rotation removed the previous I/B displacement;
see field-block-centering-zd621-v1. Raw references are unchanged.

Six atlases contain 72 width-boundary controls using fonts 0 and A: words alone,
words after an existing prefix, and hanging indents. Six additional atlases
exercise long words with justified overflow onto the final row. Sixteen layout
frames cover FO/FT, all rotations and L/C/R/J alignment. An encoding frame
compares CI0/27/28 using identical ASCII words.

The printer reserves soft-hyphen advance when splitting a word, uses strictly
less than the available width, and keeps reserving this space for the final
remainder of that word. It fills available space after a preceding word before
continuing on a new line. Centered hyphenated lines do not receive the extra
trailing space observed for ordinary word wrapping.

`block_hyphenation_printer_layout` selects those measured sizing choices and
soft-hyphen ink. Disabled, the renderer uses an ordinary hyphen, permits exact
fits and keeps a fitting remainder whole. Independently,
`block_hyphenation_ci27_uses_eth` reproduces the printer's automatic-break glyph
under CI27: the break paints eth (byte F0 / U+00F0), despite retaining the soft
hyphen's advance for layout. CI0/28 and literal soft hyphens are unaffected.
Both options are enabled in ZD621_203_DPI and disabled in SPECIFICATION.

Thirty-two source frames capture soft hyphen and eth at every embedded native
strike, with an independent composed-text verification for each. The C/D glyphs
are identical. The E/H glyphs are advancing blanks; their two verification
frames must remain completely white, while extraction-page sentinels contain
ink. Fifteen separate two-glyph assets preserve the existing ASCII assets.
`assets.tsv` pins their hashes and dimensions. To reproduce an asset offline,
copy its source directory and run `extract-font --offline --characters $'\u00ad\u00f0'
--verify-text $'\u00ad\u00f0\u00ad\u00f0' --batch-size 8` with the recorded font,
height, width, host and copied output path. C uses the identical D supplement.

Reference: [Zebra ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FB pp. 185–187 and ^CI pp. 155–159. The guide specifies a hyphen at an automatic
word break; the CI27 eth substitution is observed printer behavior. Soft-hyphen
escape sequences, blocks too narrow for a character plus hyphen, and arbitrary
unsampled text sizes remain outside this coverage.
