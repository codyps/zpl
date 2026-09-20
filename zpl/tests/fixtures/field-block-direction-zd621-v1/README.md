# Field direction inside field blocks

Forty-one unmodified ZD621 203-DPI V93.21.33Z HTTP previews, captured
2026-09-20 in six serialized batches with five-second request spacing. Every
request completed, and every final reset control matches the first control's
pixels exactly. Sources use PW832; references are neither aligned nor resized.

The matrix covers H/V/R, zero and nonzero character gaps, FO/FT, all four glyph
rotations, both origin justifications, L/C/R/J block alignment, hanging indents,
line spacing, overflow overprinting, and bitmap A/proportional 0 fonts. Separate
width sweeps and word-order controls distinguish reverse justification rules.
The final independent controls use both fonts, four word gaps and FP gaps 0/3/8.
The two original command-order controls live in `field-direction-zd621-v1`.

Two independent options describe the measured printer departures:

- `block_field_direction_printer_layout`: vertical text overprints the glyphs
  within each wrapped line. Its justified words move only by distributed extra
  space. Reverse text predecrements before the first glyph, negates alignment
  offsets, and retains positive hanging indents. Reverse justification moves
  the integer spacing quotient backwards, then puts each remainder dot forwards
  in the earliest word gaps. The 26-width sweep and four-gap holdouts distinguish
  that behavior from rounding a signed fractional word position.
- `block_spaces_ignore_character_gap`: horizontal and reverse blocks omit the
  FP gap after ASCII spaces; vertical block measurement retains that gap.

Both are enabled in ZD621_203_DPI and disabled in SPECIFICATION. The specification
profile applies ordinary directed glyph flow and character gaps inside blocks.
Unit tests cover that profile and independent option disablement.

Thirty-nine frames are pixel-exact. Two inverted font-0 frames retain ten
underpaint and ten overpaint pixels in total. All 230 nonempty text regions
exceed 99.80% foreground IoU. Tests pin source, raw PNG and rendered-pixel hashes,
exact under/overpaint counts, and an 80% minimum per region. Regions partition
canvases midway between anchors; full-frame comparisons cover every pixel.
These results concern sampled fonts and sizes, not all text rendering.

Source: [Zebra ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FB pp. 186–188, ^FP p. 202, Field Interactions pp. 1606–1611.
