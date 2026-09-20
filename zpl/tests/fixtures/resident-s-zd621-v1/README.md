# Resident S and graphic-symbol independence

Resident S (`^AS`, 40x35 native matrix) is distinct from `^GS`. The renderer
previously used S as its internal graphic-symbol identifier and rejected AS.
Graphic-symbol strikes now use the explicit `@` tag; raw historical capture
metadata is unchanged. Native S strikes at 40x35 and 80x70 contain all 95
printable ASCII characters and are reproduced byte-for-byte from raw pages.

These 32 unmodified ZD621 203-DPI V93.21.33Z preview frames pin every input,
PNG, rendered pixel hash and underpaint/overpaint count. The 26 sampling and
independent verification frames, original font-id-S case, and mixed AS/GS/CFS
label are exact. Four atlases retain measured text-raster differences: the
8-field origin atlas, 32-field block atlas, and two 16-field multiline atlases.
All 72 individual text fields exceed 97.10% foreground IoU; tests enforce
80% in every field as well as the exact pixel residuals. The mixed label also
requires pixel-exact graphic symbols before and after selecting resident S.

The block controls measure native 40/80-dot pitches of 34/67 and block ascents
of 25/50, distinct from ordinary S baselines of 30/60. FT anchors with one and
two maximum lines determine pitch independently of glyph scaling. The
`font_s_block_metrics` option selects these metrics; it is disabled in
SPECIFICATION and enabled in ZD621_203_DPI. Its ascent adjustment also changes
the block extent and FT baseline, preserving rotated anchor placement.
Multiline controls verify zero, positive, negative and fully overlapping line
spacing. Negative pitch clamps to zero under its separate existing option.

The existing `preset_font_fo_last_dot` option selects S's rotated FO pivot,
subtracting two dots of native matrix height (scaled with magnification) and
one dot of horizontal advance. Both compatibility options have independent
profile tests. Uncaptured sizes scale the nearest measured strike/metrics;
this suite does not establish arbitrary-size or Unicode accuracy.

New samples were captured 2026-09-20 with serialized requests and five-second
pacing. All completed; no PNG was resized, padded or registered. The original
font-id-S source and preview were copied unchanged from the complete comparison
conformance reference after validating their hashes. The 80-dot sampling pages
use one column to fit the printer preview width.

References: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
Table 31 p. 1584, ^A pp. 60–61, ^CF p. 154, ^FB pp. 186–188, ^FO p. 201,
^FT p. 205 and ^GS p. 217. The guide's nominal font-height block layout remains
available through SPECIFICATION.
