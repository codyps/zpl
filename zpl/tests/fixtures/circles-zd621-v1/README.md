# ZD621 circle scan conversion

38 unmodified HTTP Preview Label captures from ZD621 203 DPI, firmware
V93.21.33Z, collected 2026-09-19 with zebra-render. Inputs explicitly set PW832,
label length and field origins. No physical printing is needed. The manifest
pins source and response hashes, zero underpaint/overpaint and local pixel
hashes. circle_preview.rs compares the entire canvas using ZD621_203_DPI.

- Three initial atlases cover 30 diameters at borders 1, 3 and 100.
- Twenty dense atlases cover every diameter 4–99 at borders 1, 2, 3, 4 and 100:
  480 independent circle instances, with no overlap between fields.
- Fourteen holdouts cover diameters 101, 127, 128, 129, 199, 200, 201, 255, 256,
  257, 399, 400, 401 and 511, each at borders 1, 3, 10 and 1000.
- A tiny-size atlas covers diameters 1–7 at borders 1, 2, 3 and 10. Its separate
  one-dot box ensures the preview remains nonblank regardless of circle behavior.

The first three atlases established a two-region integer curve with half-dot
radius decisions for odd diameters. The other samples independently validate
it. Filled caps use exclusive right endpoints; outlined sides include the right
endpoint and inset the inner curve. The minimum diameter and border are both
two dots. The option circle_printer_curve controls these empirical departures;
SPECIFICATION disables it. The implementation computes arbitrary diameters,
without a table keyed by captured dimensions.

Zebra Programming Guide ^GC, pp. 212–213, specifies diameter, thickness and
color but not these pixel-level decisions. See docs/zpl-zbi2-pg-en.pdf and the
[Zebra command reference](https://www.zebra.com/us/en/support-downloads/knowledge-articles/ait/zpl-command-information-and-details.html).
