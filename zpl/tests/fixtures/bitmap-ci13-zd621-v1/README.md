# Bitmap CI13 zero and source remapping

Captured through HTTP previews on 2026-10-07 from ZD621 D7J211001302,
203 dpi, software V93.21.33Z. Two separate preview transactions produced
identical PNG bytes. The manifest pins source and capture hashes.

At native 832×256 dimensions, font A renders five zero fields at x=40:

- y=40: CI0 input zero.
- y=60: CI13 input zero (a different measured glyph).
- y=80: CI13 zero after an unrelated A-to-B source remapping.
- y=100: CI13 with explicit source 48 assigned to input 48.
- y=120: CI13 with source 26 assigned to input 48, restoring the measured
  default zero; input B is restored and the final encoding is CI0.

The repeated label verifies the restoration. The recovery dataset's measured
CI13 input-48 map and CI0 source-26 map independently identify equivalent glyphs.
Explicit source 48 must remain distinct from an unremapped CI13 input 48.

Zebra Programming Guide, ^CI pp.155–158, source-image/destination-input pairs:
https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf

The regression compares the complete native canvases at their original origins,
without alignment, rescaling, or relaxed pixel tolerances.
