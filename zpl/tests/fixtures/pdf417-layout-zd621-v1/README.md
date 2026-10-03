# PDF417 integer layout and punctuation controls

60 original HTTP Preview Label responses captured on 2026-10-03 UTC from
ZTC ZD621-203dpi ZPL, firmware V93.21.33Z, at 203 DPI. No physical printing.
`manifest.json` records the device, endpoint, acquisition times, dimensions,
and SHA-256 hashes of the exact submitted ZPL and original PNG responses.
Every request includes its own format-state reset and native 832×1218 canvas.
The first `carrier` and final `repeat` controls in the initial 13-case sequence
were byte-identical PNG responses. Two subsequent sequences added 26 layout
and 21 punctuation controls. No images were aligned, cropped or rescaled.

The carrier payload is extracted unchanged from the MIT-licensed Labelixa
example in [this comparison](https://codyps.github.io/zpl-comparison/cases/public-zpl--labelixa-carrier-style-shipping-4x6.html).
See `LICENSE-labelixa.txt`. Its literal `_1E` and `_1D` sequences are not hex
escapes: the source has no `^FH`. The complete comparison's original ZPL SHA-256
is `862c054ae82e3d439e8e21503920f8105c9170265cc2677af843996b865316c0`;
its original printer PNG SHA-256 is
`ddec21cecbb88e211ac71117efd5f7df194ce9c23fde24715d07884ccf63635f`.
The isolated symbol retains `^BY2^B7N,6,5`; its test origin is (40,40).

The automatic-layout controls distinguish rounding a continuous column estimate
from minimizing `abs(17*columns + 69 - 6*rows)` over integer grids, with
`rows = max(3, ceil(required_codewords / columns))`. Required codewords include
the length descriptor and error correction. Nominal row height is three modules;
explicit dot height is applied after selection. For 105 required words the
printer uses 4×27, and for 106 it uses 5×22. Tests cover several other transitions,
error-correction levels 0/2/5, and fixed-column punctuation cases.

The carrier has 41 compacted data words plus a descriptor and 64 ECC words.
The printer chooses 5×22, pads with four 900 words, and draws 308×132 dots.
The old encoder emitted 42 data words, selected 4×27, and drew 274×162 dots.
It used separate punctuation shifts; the printer instead latches for four
consecutive punctuation characters. The controls bracket that threshold from
Alpha, Lower and Mixed and include spaces and characters shared by Mixed and
Punctuation. These are firmware choices, not universal encoding requirements.

`pdf417_layout_preview` checks every full canvas exactly and independently
scans the carrier and punctuation holdouts. The broader `barcode_accuracy`
inventory also includes all 60 captures. The two compatibility options are
independently selectable and enabled only in the ZD621 profile.

References: [Zebra ZPL Programming Guide](../../../../docs/zpl-zbi2-pg-en.pdf),
`^B7` pp. 79–82; [USS PDF417](https://www.expresscorp.com/wp-content/uploads/2023/02/USS-PDF-417.pdf),
§2.2.4.4, Table 3 (Text submodes), §§2.5–2.6 and Appendix D.
