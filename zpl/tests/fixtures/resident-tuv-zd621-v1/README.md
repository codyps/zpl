# Resident fonts T, U and V

These 43 raw ZD621 203-DPI V93.21.33Z HTTP preview frames cover resident
T (48x42), U (59x53) and V (80x71). Previously these resident IDs were rejected.
Each captured strike contains 95 printable ASCII glyphs. Thirty-nine sampling
and independent composition frames and the three original conformance cases
match exactly. The 24-field FO/FT N/R/I/B atlas has 68 underpaint and 68
overpaint pixels; every field exceeds 97.65% foreground IoU. Tests pin every
residual and require an 80% floor in each separate field, not only the page.

The forty new frames were captured on 2026-09-20, serially with five-second
spacing. Every request completed. V uses one-column sampling to remain within
the device preview width. The verification string is `Hg0j`. No image was
resized, padded or registered. The three original inputs and PNGs were copied
unchanged from the complete comparison conformance reference after validating
source and PNG hashes. Native sampling is re-extracted offline and must
reproduce all three embedded assets byte-for-byte.

Normal FO/FT pairs independently measure baselines 36, 46 and 62. These face
metrics apply in both profiles. Rotated FO uses the existing
`preset_font_fo_last_dot` option: native T/U/V height pivots subtract 3/1/2 dots
respectively, and proportional horizontal advance subtracts one dot. The
printer profile enables this departure; SPECIFICATION disables it. The native
pivot offsets scale with matrix magnification; this fixture measures the native
sizes only. Uncaptured sizes use the closest strike and remain approximations.

References: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
Table 31 p. 1584, ^A pp. 60–61, ^CF p. 154, ^FO p. 201 and ^FT p. 205.
Table 29 omits the preset baselines, so the independent native origin controls
supply them. This suite does not establish arbitrary-size or Unicode accuracy.
