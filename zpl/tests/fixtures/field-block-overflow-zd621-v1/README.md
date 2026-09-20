# ZD621 field-block overflow and explicit breaks

Thirty-six raw ZD621 203-DPI previews, firmware V93.21.33Z, captured 2026-09-19.
All comparisons use the full canvas with no alignment, scaling or cropping.
The manifest pins input/capture hashes, exact underpaint/overpaint counts and
rendered pixel hashes. Every frame must also exceed 80% foreground-ink IoU.

The 24 resident A/F frames are pixel-exact. The 12 proportional font-0 frames
retain small rotated-glyph residuals, which are pinned,
not ignored or normalized. All ink in this suite is text, so whitespace cannot
inflate the text score.

Overflow was previously rejected. The guide's FB maximum-line parameter says
excess text overprints the final row. The renderer now unions overlapping glyph
ink on that row. Justification stops at the final block row; explicit paragraph
breaks also end justification. An explicit break does not contribute the
trailing space used by the printer's automatic-line centering behavior. The
existing `block_center_includes_trailing_space` option controls that departure;
SPECIFICATION aligns the actual text without it. The overflow rule itself is
specified behavior and applies to both profiles.

Controls cover FO/FT, all rotations, L/C/R/J, one- and two-row overflow,
automatic word wrapping, explicit breaks, and three-row non-overflow holdouts.
The A/F/0 fonts use different matrix sizes and advances. Separate specification
unit tests compare overprinting to independent fields on the same row.

Reference: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FB pp. 185–187 (maximum-line and justification parameters on p. 186),
^FO p. 201, ^FT p. 205. Exact printer placement is defined by these raw controls.

Integer centering before rotation (field-block-centering-zd621-v1) reduces
errors in six font-0 frames without changing their printer references.
