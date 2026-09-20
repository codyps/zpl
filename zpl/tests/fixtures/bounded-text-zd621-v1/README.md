# Bounded text blocks on the ZD621

Raw ZD621 203-DPI V93.21.33Z previews captured 2026-09-20, plus twelve
hash-verified `text-block-*` controls from the completed zpl-comparison
conformance capture. New captures were serialized with five-second spacing.
Every batch completed and repeated controls matched pixel-for-pixel. All
requests have preview-aligned widths; no reference is registered or resized.

Coverage includes FO/FT, all four rotations, both origin justifications, block
height truncation (including partial glyphs), narrow widths and overlong words,
spaces, angle-bracket escapes, font selection order, and replacement by FB.
Bitmap fonts A–H and sampled proportional font sizes are represented. Longer
A blocks vary requested heights and horizontal magnification to expose
accumulated line-leading errors. A later A command cancels TB on the printer;
TB after A takes the specified rotation, or the preceding A rotation by default.

The renderer keeps TB separate from FB: it truncates overflowing rows rather
than overprinting the last row, preserves spaces, and ignores soft hyphens.
Three independent compatibility options are enabled in ZD621_203_DPI and
false in SPECIFICATION:

- `bounded_text_printer_pitch`: measured line leading, including A's horizontal
  cell quantization and use of requested height before bitmap-size rounding.
- `bounded_text_printer_anchors`: measured rotated rectangle pivots and the
  proportional-font final-dot clipping in R/I rotation.
- `bounded_text_font_cancels_block`: a subsequent A command discards TB.

The exact 28×14 font-0 ASCII strike and CI0 backslash supplement are also pinned
in `assets.tsv`. Twelve atlas pages cover all 95 ASCII characters. Independent
verification strings and repeated first pages matched exactly. These captures
are rendered again in the accuracy test, not merely checked for file hashes.

The manifest pins source, raw PNG and rendered-pixel hashes, plus exact
underpaint and overpaint. Region gates require at least 80% foreground IoU;
blank reference regions must remain blank. Regions partition the canvas midway
between field anchors; the full-frame comparison covers every pixel. Residual
font raster and fractional-leading differences remain explicit in the manifest.
The results concern captured cases, not all shaping, encodings or font sizes.
Nondefault FP with TB remains explicitly unsupported.

Source: [Zebra ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
TB p. 356, FO/FT pp. 201/205, and font metrics pp. 1582–1583.

The 50-frame suite has 41 pixel-exact frames. Its 362 nonempty text regions
have a minimum foreground IoU of 88.2353%; total residual paint is 293 missing
and 768 extra pixels. The twelve independent corpus controls all meet the
text target, including blank one-dot rotated blocks. Both font verification
frames and every captured atlas page are exact.
