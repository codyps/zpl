# Font 0 at 40x22: anchor controls

37 unmodified ZD621 203-DPI V93.21.33Z HTTP previews, all pixel exact.
Twenty-four anchor cases come from the complete 2026-09-19 conformance
capture, with both source and PNG hashes verified before copying. Thirteen
fresh extraction/verification pages were captured 2026-09-20 using serialized
requests and five-second pacing. All requests completed. Preview widths are
multiples of 64; no images were resized, cropped, padded or registered.

The renderer previously scaled a different font-0 strike. That changed both
hinted glyph ink and advances, leaving most of these anchor cases below 80%
foreground IoU. Embedding the actual 40x22 strike fixes every FO/FT origin,
N/R/I/B rotation and left/right/automatic justification in this set without
changing placement rules. Entire frames, including the non-text anchor
crosses, now require zero underpaint and overpaint. manifest.tsv also pins
source, native PNG and rendered pixel hashes.

The font-source directory contains all twelve sampling pages (95 printable
ASCII characters), capture settings, measured glyph metrics and a separate
verification string. zpl-font-extract's font0_40_22 integration test reproduces
the embedded asset byte-for-byte from those pages and independently verifies
composition. All thirteen source frames also pass through the renderer.
Tests are offline. This covers the sampled size, not arbitrary font-0 sizes.

References: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^A pp. 60–61, ^FO p. 201, ^FT p. 205 and scalable-font behavior p. 1583.
