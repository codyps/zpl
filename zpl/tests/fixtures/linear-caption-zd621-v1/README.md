# ZD621 linear barcode interpretation

Thirty raw HTTP preview frames from ZD621 203 DPI, V93.21.33Z, captured
2026-09-19. Twenty-nine require exact full-canvas equality, including text.
One deliberate left-edge clipping probe retains a known gap of 1146
underpaint and 207 overpaint dots. The manifest pins input/capture hashes,
those residual counts and the entire local raster hash. No image is aligned,
cropped or rescaled; all inputs request PW832.

Six initial atlases cover eighteen variants in normal/bottom-up orientations:
Code 11/128/93, Codabar, Industrial/Interleaved 2 of 5, LOGMARS, MSI A–D,
PLANET, Plessey, ^BZ PLANET, POSTNET, Standard 2 of 5, and UPC extensions 2/5.
Nineteen single-field holdouts vary payload, module width 1/3, Code 11 one/two
checks, Codabar C/D delimiters, and explicit font 0. Five above-bar atlases
cover all orientations at module width 3, including an inset counterpart to
the clipping probe. Inputs derive from the comparison repository's
`barcode-families/symbol-*.zpl` corpus plus focused independent holdouts.

Captured behavior, each selectable through a separate compatibility option:

- Code 11 interpretation includes checksum digits and triangular delimiters.
  The start and one-check stop use four native rows; the two-check stop uses
  a taller seven-row triangle.
- Code 93 uses hollow-box delimiters. Its optional checksum interpretation
  remains unsupported and is not part of these passing controls.
- Codabar includes its selected start/stop letters.
- POSTNET/PLANET captions center over full bar pitches including the last gap.
- ^B1/^B2/^B5/^BA/^BK ignore an explicit font command and use module-scaled A.
  Code 128 continues to honor its explicit font selection.

The original caption-only IoUs were 12.5% (Code 11), 71.63% (Code 93),
70.83% (Codabar), and about 24–25% (postal); the corrected unclipped controls
are 100%. These figures compare foreground caption ink, not blank canvas.
The remaining clipped-caption case is explicitly not an accuracy-goal pass:
the printer repositions text that crosses the left edge, while the renderer
currently clips it. Its inset counterpart matches exactly.

Sources: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^B1 p. 66, ^B2 pp. 68–69, ^B5 pp. 78–79, ^BA pp. 87–89,
^BC p. 94 (explicit font), ^BK pp. 118–119, ^BZ pp. 150–153,
^FT p. 205 Table 7. The preserved previews establish the exact glyph masks
and printer-specific layout details.
