# ZD621 curved-shape controls

34 raw HTTP Preview Label captures from Zebra ZD621, 203 DPI,
firmware V93.21.33Z, collected 2026-09-18 using `zebra-render`.
Inputs reset label state, use PW832 and place one primitive at FO80,80.
No printer image is cropped, resized or repaired.

- Twelve circles: diameters 15, 16, 31, 32, 60 and 81; borders 1 and 3.
- Ten ellipses: 30×20, 31×21, 60×40, 80×120 and 121×61; borders 1 and 3.
- Twelve rounded boxes: 60×40/r4, 61×41/r4, 100×60/r2 and 100×60/r8;
  borders 1, 3 and 6.

Command reference: Zebra Programming Guide ^GB/^GC/^GE, pp. 210–214.
The firmware's integer curve rasterization remains unreproduced. This suite
preserves the observations for that work; it does not claim pixel parity.
For example, the circle and ellipse one-dot outlines are substantially thicker
in the preview than nominal geometry, and even/odd dimensions quantize differently.

`manifest.tsv` pins each source and capture SHA-256, separate underpaint and
overpaint counts, and the complete local pixel SHA-256. The test uses
ZD621_203_DPI and requires those exact counts and positions. Improve the
renderer and deliberately tighten the manifest when the next fix is validated;
do not regenerate printer references from local output.
