# Public ZPL preview acceptance regressions

These 22 inputs and native PNGs were copied byte-for-byte from
[codyps/zpl-comparison](https://github.com/codyps/zpl-comparison),
`test-data/public-zpl/cases` and `references/public-zd621-20261002`.
They are the cases in the [public document comparison](https://codyps.github.io/zpl-comparison/categories/public-zpl.html).
Input and PNG SHA-256 values were checked against the published category evidence.

The printer is **ZTC ZD621-203dpi ZPL, V93.21.33Z**, at 203 DPI. These are HTTP
Preview Label responses from October 2, 2026, not physical prints. `capture.json`
preserves capture times, submitted hashes, native dimensions, reset ZPL and the
matching repeat-end control. The endpoint hostname is anonymized. The repeat is byte-identical to the carrier-style
input/capture already included here. No printer was contacted for this regression.

`sources.json` and `manifest.json` preserve the comparison repository's source
provenance and adaptations; their paths refer to that repository. This directory
flattens its `cases/` and capture paths into `<name>.zpl` and `<name>.png`.
Original immutable source URLs are recorded there. Both upstream MIT licenses
are retained in `licenses/`. Source payloads are unchanged, including template
placeholders. Canvas widths were rounded to native 64-dot multiples by the
original comparison campaign; three batch formats were separated there.

`public_zpl_preview.rs` uses `ZD621_203_DPI`, checks every source/capture hash,
requires one nonblank native-sized output per case, and pins **full-canvas**
underpaint, overpaint and local pixel hashes in `baseline.tsv`. Counts use the
original origin without padding, alignment, cropping or scaling. IoU is foreground
intersection over union; higher is better. Seven complete labels are exact.
The QR segmentation compatibility option and captured 28-dot font strike resolve
the Labelixa QR URL label; the font also resolves the EAN-13 product label.
The same strike reduces both directional errors in Example3-54x86, the carrier,
GS1-128, pallet, and shipping labels. Their stronger baselines retain the original
captures; remaining differences are text. See the
[QR/font capture evidence](../qr-segmentation-zd621-v1/README.md).
The PDF417 layout/punctuation fix merged from main makes the carrier symbol
pixel-exact; its whole-label residual is now text only.

The five formerly rejected documents now render:

| Document | Tolerance | Whole-canvas foreground IoU |
| --- | --- | --- |
| Example2-102x170 | End inline GF before FT/BY without FS | 84.5068% |
| Example4-102x152 | Malformed QR header; GB thickness zero | 84.7111% |
| Example5-75x202 | Code 39 `%s` becomes `%S` | 87.9309% |
| Example6-75x254 | BY module width 12 | 92.6249% |
| Example8-64x152 | Code 39 `{0}` becomes `0` | 83.7939% |

The affected graphic/barcode regions are additionally pixel-exact at their native
coordinates, as are Example4's QR and zero-thickness table line. Tests specify
those bounds explicitly. Example6's wide bars remain clipped at the canvas bottom;
this does not establish decoder success. Example4's QR format bits identify level
M and mask 0, and its encoded payload is **`kage 4 in the Storage Box 2B`**, losing
the initial `Pac`. `%S` is the Code 39 Extended representation of `~`, not literal
`%s`. The original template placeholders have not been substituted or repaired.

The five compatibility settings are independent, enabled in `ZD621_203_DPI`, and
disabled in `SPECIFICATION` and `ZQ610_PLUS_203_DPI`. Original errors and byte
offsets are tested with each setting disabled. The scope is these observed ZD621
tolerances, not arbitrary malformed ZPL or other firmware. In particular, manual
and structured-append QR parsing, other drawing separators, shape dimensions,
resource budgets and proxy admission remain validated.
