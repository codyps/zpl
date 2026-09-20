# Graphic placement on ZD621

63 unmodified HTTP Preview Label responses from ZD621 203 DPI, firmware
V93.21.33Z, captured 2026-09-20. PW832 avoids preview width adjustment.
Requests were serialized with five-second pacing; only complete captures
are included. Input, response and local grayscale pixel SHA-256 hashes are
pinned, with zero underpaint and overpaint over the full canvas.

- 14 edge frames cover boxes, circles, ellipses, both diagonals, filled boxes
  and bitmaps with label shift and label top. The ZD621 profile ignores LT;
  those top frames are controls for that behavior, not vertical clipping.
- 36 layout frames cover FO/FT, every FW rotation, negative and positive shifted
  x coordinates, and FT y positions above and below nominal graphic height.
- Four size/justification frames cover odd/even circle diameters, blank trailing
  bitmap rows, recalled graphics scaled 2 by 3, and left/right FO/FT justification.
- Four boundary frames place all seven graphic types at nominal height minus
  one, equal to height, plus one and plus two.

- Four further justification frames exercise both edge-clamped and unclamped
  boxes, ellipses, diagonals and sparse bitmaps, plus a recovery control after
  the printer power cycle. All five are exact.

The printer ignores FW rotation for these commands. The
[Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FW p. 208, applies orientation to commands with a rotation parameter. Graphics
have no such parameter; both profiles preserve their orientation. FT Table 7,
p. 205, identifies the graphic area (including blank bitmap rows) as the origin
reference; right justification uses its nominal width, not visible ink width.

Empirical dot placement is independently selectable:
`graphic_ft_last_row_baseline` uses nominal height minus one;
`graphic_clamps_negative_origin` clamps the top-left origin after home/shift and
FT placement. Together, FT y at or below nominal height starts at row zero.
FO bitmap justification follows clamping, whereas FT bitmaps and primitive
shapes clamp the justified origin. Both options are enabled by ZD621_203_DPI
and disabled by SPECIFICATION. profiles.rs checks that they can be disabled
independently. Binary GF shares the nominal-size path; an attempted supplemental
binary capture timed out and is deliberately excluded from this suite.
