# ZD621 CR, LF, and SOH text controls

Five unmodified ZD621 203-DPI V93.21.33Z previews contain 66 text fields.
Each JSON records a successful single submission and input/output SHA-256.
No pixels were edited, aligned, or cropped. All requests used width 832.

The `lf`, `cr`, and `soh` frames each cover plain, FB, and TB layouts with
CI0/27/28 Font 0 at 32 dots and CI27 bitmap A at 18x10. All 36 fields are
pixel-exact. CR/LF terminate plain and FB fields but break TB lines. SOH is
ignored in plain/TB text and becomes a space in FB.

`sequences` and independent `holdouts` cover paired CRLF, LFCR, repeated CR/LF,
leading breaks, three lines, repeated SOH, and interaction with NUL. CRLF is
one break; LFCR and repeated CR/LF retain blank lines. The third line retains
a one-dot glyph offset: sequences pins 96 underpaint/96 overpaint, holdouts
94/94, for 98.04% and 97.77% frame foreground IoU. Tests enforce exact paint
counts and raster hashes as well as the 80% text floor.

`text_control_processing` enables these empirical rules in ZD621_203_DPI;
SPECIFICATION disables it. NUL remains a separate option. Barcode bytes are
unchanged, including decoded control bytes.

References: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
FD p. 190, FH pp. 193–194, FB p. 187, TB p. 356. FB says ordinary CR/LF are
discarded; the captured hex-inserted CR/LF instead truncate the field.
