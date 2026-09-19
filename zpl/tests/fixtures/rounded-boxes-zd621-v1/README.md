# ZD621 rounded-box scan conversion

25 raw HTTP Preview Label responses from ZD621 203 DPI, firmware V93.21.33Z,
collected 2026-09-19. Inputs use PW832 and explicit label/origin state. All
captures remain unmodified; manifest.tsv pins input, response and local pixel
hashes and zero underpaint/overpaint. rounded_box_preview.rs requires exact
full-canvas pixels with ZD621_203_DPI.

- Two atlases cover radii 1–48 at requested border widths 1 and 3.
- Two small-box atlases cover 48 combinations each of width, height and rounding
  at those border widths, including inner radii that truncate to zero.
- Twenty-one independent larger-radius controls cover 49–257, including
  boundaries around 64, 128, 200 and 256. They were captured after deriving the
  recurrence from the first radius atlas and validate both outer and inner curves.

The integer curve has a conventional midpoint first octant and a different
shallow-octant error recurrence. The implementation computes it for arbitrary
radii rather than using a captured-radius lookup. The profile option
`rounded_box_printer_curve` selects it independently of
`rounded_box_printer_geometry`, which controls the two-dot minimum border,
dimensions no smaller than the border, independent inner rounding percentage,
and two-dot minimum integer corner radius (including a zero inner-radius result).
The two compatibility options are disabled in SPECIFICATION.

Zebra Programming Guide ^GB, pp. 210–211, specifies the rounding percentage;
the exact integer recurrence and minimum-radius observations above are empirical
firmware behavior, not claims made by the specification. See the bundled
`docs/zpl-zbi2-pg-en.pdf` and
[Zebra command reference](https://www.zebra.com/us/en/support-downloads/knowledge-articles/ait/zpl-command-information-and-details.html).
