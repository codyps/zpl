# Native font sizes for shipping and typography labels

Six complete 95-character ASCII Font 0 strikes were sampled from ZD621
203-DPI firmware V93.21.33Z at (height,width) 20×12, 28×15, 36×22, 28×16, 52×30,
and 42×24. Each has twelve unmodified sampling frames and an independent
`Hgypqj 0123 ABCxyz` verification frame. All 78 frames are pixel-exact when
rendered with the embedded strikes; extraction tests reconstruct each ZBF
from its original sampling pages and validate the verification pixels.

`origins` independently checks Wj at the five shipping-label sizes, FO and FT, and all four
orientations: 40 fields, with 16 underpaint and 16 overpaint pixels in total.
Every field must exceed 80% foreground IoU. No fixture pixels are edited,
cropped, or aligned; all raw sources, PNGs and resulting raster hashes are
pinned by the manifest.

`original` is the comparison corpus shipping label, verified against its
source manifest. `shipping-repeat` is a fresh single successful submission
and has identical printer pixels. The renderer has 6 underpaint and 9
overpaint pixels in each, entirely within rotated text. The title, justified
block, reversed lettering, and every non-text pixel are exact. Inverted and
vertical lettering exceed 98.9% IoU.

Tests measure each text field separately. Reversed text is measured as white
ink inside its black box; the background does not inflate its text score.
All pixels outside the five text regions must match exactly. The manifest
contains 82 complete frames, including both shipping-label captures and
the original typography label. Its nine small 20×12 headings previously
scored only 44–55% IoU; the entire typography frame is now pixel-exact.

References: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
A p. 60, FO p. 201, FT p. 205, and FB pp. 186–187. Font 0 is hinted at each
requested size; measured strikes replace scaled approximations without
changing the specification or printer compatibility options.
