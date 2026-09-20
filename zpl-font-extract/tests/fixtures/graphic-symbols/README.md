# Graphic-symbol extraction control

ZD621 203 DPI V93.21.33Z, captured 2026-09-19. This native 24 by 24
`^GS` extraction page measures ASCII 64 through 71. Only A-E paint symbols;
all eight inputs advance 26 dots. Registered symbol A replaces the normal
vertical-bar sentinel, which is blank in this face. The test compares generated
ZPL to the captured input and extracts the raw PNG, including blank glyphs.

Reference: [Zebra ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^GS p. 217 and Table 29 p. 1582. Complete source and verification captures
are in `zpl/tests/fixtures/graphic-symbols-zd621-v1` in the workspace.
