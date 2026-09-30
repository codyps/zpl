# Retail encoding and missing glyph evidence

Captured on ZTC ZD621-203dpi ZPL D7J211001302, firmware V93.21.33Z,
at http://D7J211001302.bed.einic.org/ using HTTP Preview Label, not printing.
The temporary loopback transport delegates to system /usr/bin/curl because
other processes in the automation session could not access the local network.

font-25-14 and font-32-0 contain native samples of space, middle dot, A-grave,
A-circumflex, and box-drawing down-and-horizontal. Sampling uses CI28. Each
has a separately composed verification string that matched the printer exactly.
Original sample ZPL/PNGs and extraction metadata are retained. Embedded ZBF
files contain these glyphs only; they supplement missing characters without
replacing the established ASCII strikes.

controls compares the same C2 B7 bytes under CI0, CI27, and CI28, plus independent
rotations and a repeated UTF-8 control. It records exact submitted bytes and
PNG hashes. CP850 maps these bytes to U+252C/U+00C0; CP1252 maps them to
U+00C2/U+00B7; UTF-8 maps them to U+00B7. No encoding is guessed from the bytes.

References: Zebra Programming Guide CI (pp. 156-159), and Unicode CP850 mapping:
https://docs.zebra.com/us/en/printers/software/zpl-pg/c-zpl-zpl-commands/r-zpl-ci.html
https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/PC/CP850.TXT
