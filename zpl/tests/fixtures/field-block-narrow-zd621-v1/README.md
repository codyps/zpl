# Narrow field blocks

Eight unmodified HTTP Preview Label responses from ZD621 203 DPI,
firmware V93.21.33Z, captured 2026-09-20 using PW832. Requests were serialized
with five-second pacing. Only completed captures are included.

Four atlases cover 160 font 0/A blocks at widths 1–40, with single words and
space-separated letters. Four more cover 56 space-width boundaries using
14 settings of font 0, A, B, D, E, F, G and H. Every frame is pixel-exact;
the manifest pins source/PNG/pixel hashes and zero underpaint/overpaint.

[Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FB p. 186, says text narrower than the font does not print. SPECIFICATION
honors this rule for the supported positive block widths. ZD621 instead emits
one character when it cannot fit a character plus hyphen, even if that character
exceeds the block width. It paints a hyphen at an exact character-plus-hyphen
fit, including the final character. A separator after a forced line consumes
a blank row when its advance is at least the available width. Negative
alignment slack is clamped for forced characters.

`block_narrow_printer_layout` selects this measured departure independently
of automatic hyphen metrics and CI27 glyph substitution. It is enabled by
ZD621_203_DPI and disabled by SPECIFICATION. profiles.rs checks the override.
Rotated fractional centering and field-block escape handling have separate
outstanding diagnostic captures and are not claimed as fixed here.
