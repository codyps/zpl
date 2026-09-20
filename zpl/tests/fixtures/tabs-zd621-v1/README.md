# Native tab stops and field-block tab spacing

Eleven unmodified ZD621 203-DPI V93.21.33Z previews contain 120 text fields.
Each capture completed on its first HTTP submission with `Connection: close`;
requests were separated by at least five seconds. Sources, PNGs, directional
paint counts and renderer pixel hashes are pinned. No image was resized,
aligned or edited. Tests additionally enforce 80% foreground IoU in each of
120 capture-grid regions; the final row extends to the canvas edge to include
rotated ink. Some rotated fields overlap neighboring rows, so their strict
whole-frame paint/hash gates also preserve the composed result.

`plain`, `fb` and `tb` each contain sixteen controls: Font 0 at 32 dots and
bitmap A at 18×10, leading/trailing/consecutive tabs, unequal prefix lengths,
and ordinary spaces adjacent to tabs. Plain text and TB advance to the next
80-dot stop relative to the field or line origin, independent of font size.
FB instead inserts one space-width character without treating it as a word
separator; leading tabs remain visible spacing. `fb-wrap` verifies this with
narrow blocks. `wrap` checks TB wrapping and CR/LF interaction at two widths.

Four `origins-*` frames check FO/FT, left/right justification and all rotations.
`right-leading` independently checks leading tabs/spaces and both fonts with
FO/R/right justification: the printer preserves tab indentation instead of
removing it with the first glyph's left bearing. `directions` covers explicit
FP horizontal spacing, reverse flow and vertical flow. Horizontal/reverse tab
stops account for preceding advances and gaps; TAB does not add another gap.
Vertical flow retains one empty character position.

Seven frames are pixel-exact. TB retains 10 underpaint pixels; the R and I
origin frames each retain four overpaint pixels; the TB wrap frame retains
63 underpaint and 47 overpaint pixels. All tested regions exceed 95% IoU.
These small text rasterization differences remain pinned, not silently accepted
as an adjustable tolerance.

`text_tab_stops` is enabled by ZD621_203_DPI and disabled by SPECIFICATION.
Independent option tests verify the 80-dot placement and unchanged QR data.
ESC and DEL controls are separate work; these fixtures contain TAB only,
apart from the already-supported CR/LF wrapping control.

References: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FD p. 190, ^FH pp. 193–194, ^FB pp. 186–187, ^FO/^FT pp. 201/205,
^FP p. 202 and ^TB pp. 356–357. These empirical tab rules are not specified
by those commands, so they remain selectable compatibility behavior.
