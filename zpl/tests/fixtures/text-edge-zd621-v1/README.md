# Text origins at printer preview edges

106 unmodified HTTP Preview Label responses from ZD621 203 DPI, firmware
V93.21.33Z, captured 2026-09-20 using PW832. All requests completed and were
serialized with five-second pacing. No printer reference was generated locally.

The suite has 912 individual text fields. Every field exceeds 99.84% foreground
IoU; 85 complete frames match exactly. The other 21 retain pinned font-0
residuals of 1–16 overpaint dots per frame, with no underpaint. The formerly
79.21% narrow FT-B field is now pixel-exact. These results do not claim that
QR mask selection or field-block escapes are fixed.

- Eight initial frames compare plain and block text at negative/positive origins.
- 32 rotation frames cover FO/FT, all rotations, fonts 0/A and label shift.
- 32 holdouts cover the top edge and left/right origin justification.
- 16 narrow-block layouts cover L/C/R/J, every rotation and FO/FT.
- 18 further right-origin atlases cover fonts 0/A/B/D/E/F/G/H, two A scales,
  one/two/four maximum rows and nonzero line spacing.

The manifest pins source and PNG SHA-256, rendered pixel SHA-256 and exact
underpaint/overpaint. fields.tsv partitions all reference/candidate ink into
individual field regions. Tests check each region's exact error counts, enforce
80% foreground IoU independently, and prove that no ink is omitted or counted
twice by those regions. Comparison remains at the original canvas origin with
no registration, padding or rescaling.

[Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FO p. 201, ^FT p. 205, ^LS p. 296 and ^FB pp. 186–187 describe the positioning
commands. The captured departures are independently selectable:

- `text_clamps_negative_origins` clamps the field origin after home/shift, then
  independently clamps each rotated glyph's ink origin. Whole-line translation
  cannot reproduce inverted text, whose glyphs may overlap at the edge. The
  renderer unions that overlapping ink before drawing, including block overflow.
- `block_fo_right_justification_printer_layout` selects the right-justified FO
  block anchors: B excludes the final line height from its shift, and I shifts
  by one dot minus block width. Ordinary text and FT anchors are unaffected.

Both options are enabled by ZD621_203_DPI and disabled by SPECIFICATION.
Profile tests cover independent overrides and unchanged positive text origins.
