# Common font-0 sizes and paragraph layout

77 unmodified ZD621 203-DPI V93.21.33Z HTTP previews. Sixteen original cases
were copied from the complete 2026-09-19 conformance capture after verifying
source and PNG hashes. The other 61 frames were captured 2026-09-20 with
serialized requests and five-second pacing. All requests completed. Widths
are multiples of 64 (including PW640 compact labels); no images were padded,
resized, cropped or registered. The manifest pins source/native PNG/rendered
pixel hashes and exact underpaint/overpaint counts.

76 frames are exact. The dense torture-geometry label retains seven missing
and seven extra pixels, confined to rotated text. A fresh geometry-only preview
contains the identical source with only its 36 text fields removed. Tests check
that transformation, require the geometry frame to match exactly, preserve
all its black pixels in the mixed label, and require equality outside isolated
text regions. Geometry pixels are excluded from each text-IoU calculation.
All 36 text regions exceed 80%; minimum 93.3333%.

Four 95-character ASCII strikes (24x12, 20x10, 32x20 and 26x16) replace scaling
from a different hinted size. Each font-* directory preserves twelve sampling
pages, an independent composed-string verification, capture settings and
measured glyph metrics. Offline tests reproduce each embedded asset byte-for-byte
and verify the independent composition. All 52 sampling/verification frames
also run through the renderer's full native comparison test.

The accurate strikes exposed two separate FB layout choices. Explicit carriage
return/line feed resets hanging indentation, including the width available to
wrap the next paragraph. New L/C/R/J paragraph controls and leading/consecutive
breaks verify this. block_hard_break_resets_indent enables the measured behavior
in ZD621_203_DPI; SPECIFICATION retains indentation after the field's first line.

Horizontal justified spacing uses an integer quotient, then distributes leftover
dots to the earliest word gaps. The existing block_justification_rounds_up option
selects this quantization instead of nearest-dot accumulation. Four-, five- and
six-gap controls with proportional 0 and fixed A distinguish it from cumulative
ceiling; earlier one-to-three-gap controls could not. All four original compact
wrapping alignments now match exactly. Vertical/reverse flow retains its separately
measured rules. Uncaptured sizes and extended encodings remain outside this set.

References: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^A pp. 60–61, ^FB pp. 186–187, and scalable-font behavior p. 1583. The guide
specifies indentation of the second and remaining lines and word justification,
without prescribing the observed paragraph reset or integer remainder placement.
