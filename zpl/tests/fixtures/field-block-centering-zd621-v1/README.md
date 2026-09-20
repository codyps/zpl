# Field-block centering before rotation

Four unmodified HTTP Preview Label responses from ZD621 203 DPI,
firmware V93.21.33Z, captured 2026-09-20 using PW832. All requests completed
and were serialized with five-second pacing. Each frame contains 32 fields,
covering every rotation and eight odd slack widths with FO/FT and fonts 0/A.
All 128 controls are pixel-exact. Source, PNG and local pixel hashes and
zero underpaint/overpaint are pinned without registration or cropping.

[Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FB p. 187, defines centering but does not specify integer rounding. The ZD621
rounds each centered line position down before rotating it. Keeping half-dot
coordinates through rotation caused one-dot displacements in I/B text;
`block_center_rounds_down` enables the measured rounding independently of
the trailing-space option. SPECIFICATION retains fractional positioning.

The same fix reduces errors in 14 earlier overflow/hyphenation frames, with
no increase in either error count across the 62 saved centered-text frames
reviewed. Six formerly approximate hyphenation frames are now exact; two
retain only three underpaint dots each. The raw printer references are unchanged.
Negative rotated text origins and field-block escapes remain separate gaps.
