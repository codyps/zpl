# ZD621 barcode interpretation origins

Forty unmodified HTTP Preview Label responses from ZD621 203 DPI, firmware
V93.21.33Z, collected 2026-09-19 with `zebra-render`. Every input uses PW832,
explicit label state and ^BY. The manifest pins source/response SHA-256,
zero underpaint/overpaint, and the complete local pixel hash. Tests require
full-canvas equality, so matching bars cannot hide errors in the text.

- Sixteen `above-readable-*` inputs are unchanged copies of the B2/BC/BE/BU
  readable controls in the sibling comparison repository's conformance suite.
  They cover all four orientations with interpretation above the bars.
- Eight `below-readable-*` controls change only the above-line operand to N
  for Code 128 and Interleaved 2 of 5, again in all four orientations.
- Sixteen further controls exercise I/B rotations, above/below interpretation,
  both symbologies, and module widths 1 and 3. The `offset` case uses x=351;
  the other controls use x=350. All use y=500 and bar height 70.

Measured printer choices are independently selectable:

- `barcode_fo_uses_bar_height` anchors rotated ^FO fields by nominal bar height,
  excluding interpretation and retail guard extensions from that anchor.
- `barcode_above_text_keeps_bar_origin` puts interpretation above the bar
  origin instead of moving the bars down. Code 128 retains its existing
  independent `code128_above_text_keeps_bar_origin` option.
- `barcode_reverse_interpretation_shift` moves automatically centered I/B
  interpretation one dot along the reversed reading direction. It applies
  before field rotation, independently of bar placement. Explicit fonts do
  not use this auto-centering adjustment.

All are enabled by `ZD621_203_DPI` and disabled by `SPECIFICATION`. Profile
unit tests exercise independent overrides. Macro ^FM components retain their
separate origin handling.

Before these fixes, some fields had less than 45% whole-symbol foreground IoU.
After correcting bar origins, the remaining I/B interpretation text alone had
about 52–54% IoU due to its one-dot displacement. All forty frames are now exact.
Code 39 interpretation and below-bar UPC/EAN layout are separate outstanding
cases and are not included in this gate.

References: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FO p. 201, ^B2 pp. 66–69, ^BC pp. 94–99, ^BE pp. 109–110 and ^BU pp. 142–143.
