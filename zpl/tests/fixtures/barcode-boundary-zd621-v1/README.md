# ZD621 rotated bar boundary

Twenty-four raw ZD621 203-DPI HTTP previews, firmware V93.21.33Z, captured
2026-09-19. Every input requests an 832 × 1218 canvas. The manifest pins
source and raw capture hashes, underpaint/overpaint counts, and rendered
pixel hashes. No alignment, scaling, or cropping is applied.

Twenty frames are pixel-exact. Controls cover Code 11/39/93/128 and
Interleaved 2 of 5, all four orientations, FO/FT, home/shift, origins zero
and one, heights 1/2/3/30/31/80, module widths 1/3, and negative shifts
of 1/2/10 dots. R bars whose final starting X is nonpositive and I bars
whose starting Y is nonpositive lose their farthest height dot. A one-dot
bar disappears. Negative offsets are independently moved onto the canvas;
they do not remove additional height dots. Positive starting edges and N/B
orientations retain their full height.

`linear_barcode_rotated_edge_loses_dot` selects the boundary departure.
It is independent of `linear_barcode_clamps_negative_ink`; both are enabled
by ZD621_203_DPI and disabled by SPECIFICATION. These options currently cover
only the five families verified by the edge controls.

Four module-width-2 caption holdouts preserve a separate measured text gap:

| Control | Underpaint | Overpaint |
| --- | ---: | ---: |
| zero-caption-I-black | 12 | 12 |
| zero-caption-I-reverse | 20 | 20 |
| zero-caption-R-black | 50 | 50 |
| zero-caption-R-reverse | 112 | 112 |

The residuals are in Code 11 short-glyph placement: the printer retains
six dots of padding for short rotated/inverted glyphs at this size, while
the renderer clamps their visible ink directly to zero. Code 128 and Code 93
fields in these controls match. The I inputs additionally exercise right-edge
clipping. Passing the pinned residuals is not an accuracy-goal claim, and
blank canvas/background ink is not used to claim a text IoU score.

Reference: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FO p. 201 and ^FT p. 205 Table 7. The raw printer controls establish the
undocumented boundary and caption behavior.
