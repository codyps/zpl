# ZQ610 Plus native preview controls

120 unmodified PNGs from ZTC ZQ610 Plus-203dpi ZPL, serial XXZMJ230802993,
firmware V100.21.21Z. These are HTTP Preview Label results, not physical prints.
UTC timestamps, submitted-byte hashes, returned-image hashes, printer identity,
and acquisition status are preserved in `paired-capture.json` and
`smoke-capture.json`. The `.zpl` files contain the exact submitted bytes.

116 frames came from the sibling comparison repository's
`references/zq610-plus-v1` paired campaign: 60 barcodes, 37 layout/font/canvas
controls, three unclipped text replacements, 12 width boundary controls and
four visible clipping-control companions.
113 native ZD621 V93.21.33Z pairs match in the common coordinate region;
three width-cap diagnostics differ. This is not full-image equality. Each
local-renderer regression compares the entire native ZQ610 canvas at its original
origin, with no alignment, padding, cropping or rescaling.

The wide CODABLOCK A case moves FO60 to FO20 and retains its module size and
payload. Native ink ends at x=381, inside the 384-dot canvas. Other captured
barcode fields already fit. The four `smoke-*` frames retain `^LL1218`; all
native images are 2030 rows high. Thus `preview_ignores_label_length` preserves
the caller's configured preview height and `ZQ610_PLUS_203_DPI` defaults to the
observed height of 2030. The option is independent of the ZD621 overrides and
disabled for SPECIFICATION and ZD621. A device with another configured height
requires a caller override; 2030 is not a universal physical media dimension.

The profile also caps width at 384, rounds to multiples of 64 with a centered
origin, and fixes canvas width at the first draw. Each option is independent.
116 frames are nonblank and pixel-exact with ZQ610_PLUS_203_DPI. Three rotated
Font 0 frames retain residuals shared by both printers: B under/over 1/3,
I 5/3, R 5/0 (foreground IoU at least 0.985). One width-late-grow control is
deliberately blank and is not positive rendering-accuracy evidence. The TSV pins
source/PNG hashes, directional paint differences, and local pixel hashes.
The four smoke frames have byte-identical start/end control captures recorded
in their original provenance.

All 116 paired cases completed. Earlier HTTP stalls, explicit retries and
authorized resets remain in provenance. Later bounded batches include controls;
earlier sessions without end controls remain incomplete repeatability checks.
This is a focused subset, not the full earlier 5,106-record inventory. No
coverage of uncaptured fonts or arbitrary ZQ610 firmware behavior is implied.

Reproduction: sibling `zpl-comparison/benchmarks/zq610.py` prepares/captures the
campaign and exports exact frames plus the explicit diagnostics above. Existing
native sources and PNGs cannot be replaced. See `docs/printer-recapture.md` and the comparison campaign
README for commands. Preserve original sources and inspect new baselines before
extending the tests; no tolerance is used to hide differences.
