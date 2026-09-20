# Field-block limits, spacing and overflowing indentation

Forty-three unmodified ZD621 203 DPI HTTP preview responses, firmware V93.21.33Z, captured
2026-09-20. Requests completed sequentially with five-second pacing. A repeated
zero-width bitmap-font control matched every printer pixel after the captures.
PW832 avoids the printer preview's width adjustment. Input, raw PNG and local
pixel hashes are pinned together with separate underpaint/overpaint counts.

The 629 fields cover zero and omitted widths, indentation equal to and greater
than width, an omitted width after a positive width, all four block alignments,
FO/FT and N/R/I/B rotations, negative spacing on both sides of zero line pitch,
centered overflow at the label edge, adjacent-origin cutoff sweeps, and a
2,300-character continuation extending beyond the label. All use font A or 0.
The source ZPL is authoritative for dimensions, origins and content.

[Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FB pp. 186–187 permits spacing -9999..9999 and indentation 0..9999; it specifies
no printing when width is below the selected font width. The specification
profile suppresses such fields and retains signed line pitch. Printer options:

- `block_narrow_printer_layout` also applies to zero/omitted widths.
- `block_negative_pitch_clamps_to_zero` clamps height plus spacing at zero.
- `block_indent_printer_layout` retains an intact word's fit when moving to an
  indented line, and stops wrapping when indent exceeds width.
- `block_center_overflow_clamps_to_origin` includes trailing space when centering
  overflow, drops glyphs whose rotated inline ink start is negative, and moves
  each surviving glyph to the absolute inline-axis origin.

Wholly off-canvas text rectangles do not consume the visible path budget.
The long continuation is compared against real printer output, with the path
limit retained for visible content. Culling accounts for later PW/LL changes.

Thirty frames match exactly. The other thirteen retain zero underpaint and
119 overpaint dots in total. All 211 fields in those frames are measured
individually; minimum foreground IoU is 99.7389%.

Exact frames require equality for every pixel. Nonexact font-0 frames additionally
pin each field's directional errors and enforce at least 80% foreground IoU.
Those field rectangles must cover every reference/candidate ink pixel exactly
once. The original dense font-0 alignment atlas contained overlapping fields;
it was replaced by separate rotation frames so neighboring fields cannot hide
one another's errors. Blank background is excluded from the IoU metric.
