# Automatic QR segmentation and Labelixa label

These are unmodified HTTP Preview Label responses from a ZTC ZD621-203dpi ZPL,
firmware V93.21.33Z, at 203 DPI. They are native previews, not physical scans.
`provenance.json` records the original comparison source, capture timestamps,
request/response hashes, diagnostic field coordinates, and font asset hash.
`manifest.tsv` pins native dimensions, zero underpaint/overpaint, and pixel hashes
for all thirteen regression frames. Tests compare entire canvases at their
original origin without alignment, cropping, padding, or rescaling.

## Reported case

`labelixa-original` preserves the exact ZPL and PNG from the
[comparison case](https://codyps.github.io/zpl-comparison/cases/public-zpl--labelixa-qr-url-2x2.html#codyps-zpl).
The source's MIT notice is retained in `LICENSE-labelixa.txt`. The two fresh
`labelixa-*-repeat` controls explicitly establish barcode/layout state; the
last also establishes CI27. Both have the original preview's pixels.

The input is `^BQN,2,8` with `QA,https://example.com/product/48213`.
The printer's decoded segments are Byte(`https://example.com/product`) and
Alphanumeric(`/48213`): 274 data bits, requiring Model 2 Version 4 at level Q,
33×33 modules. Its mask is 7; the ink bounds are (90,69) through (353,332).
The previous minimum-bit encoder instead emitted Byte(`https://example.com/product/`)
and Numeric(`48213`): 267 bits, fitting Version 3/Q, 29×29 modules.
Both have the correct payload. Magnification remains eight dots per module;
the encoding choice changes the symbol from 232×232 to 264×264 dots.

The `qr_printer_segmentation` compatibility option selects the observed run
merging. It is enabled in ZD621 and disabled in SPECIFICATION and ZQ610, where
this change has not been measured. The specification profile retains minimum-bit
encoding. Manual input is unaffected, and mask selection remains independently
selectable. The new integration test independently decodes both encodings.

## Segmentation evidence

The printer merges adjacent numeric, alphanumeric, and byte runs with a forward
scan. A merge must reduce bit cost; ties retain the separate runs. Matching
outer runs can absorb the intervening run. Before absorbing a compact run into
bytes, the following compact run gets a chance to join it. Previously accepted
boundaries are not revisited. This last detail matters: `abcABCDEF123xyz` keeps
an alphanumeric middle segment although encoding everything as bytes is shorter.
The implementation is original, inferred from the captured module bitstreams.

The nine QR atlases contain 296 symbols:

- `tails`, `interior`, and `alpha-numeric` vary run lengths across the numeric,
  alphanumeric, and byte transition thresholds.
- `capacity` varies byte-prefix lengths around symbol-version boundaries.
- `mixed-runs` and `merge-order` distinguish staged merging from global optimal
  segmentation and from a blanket whole-field byte-mode workaround.
- `holdouts-m1` and `holdouts-m2` cover both QR models and all four correction
  levels with URLs and interleaved runs. Two of these payloads informed the
  final merge-order refinement; they are retained as diagnostic controls.
- `final-holdouts` was captured after that refinement, with twelve new payloads.

These captures use explicit PW/LL/LH/LS/LT/PO/LR/BY, requested mask zero, and
two-dot modules. Requests were serialized. No physical printing, firmware
updates, or automatic POST retries were performed. They establish this tested
firmware's behavior, not universal agreement for arbitrary payloads or printers.

## Text evidence

The remaining caption difference was an unsampled natural-width 28-dot font-0
strike. `font-0-28/` contains twelve eight-glyph sampling pages (95 printable
ASCII glyphs total), extractor metadata, and a separate composition verification.
`font0-28-0.zbf` is reproduced byte-for-byte by `zpl-font-extract/tests/qr_label_font.rs`.
The complete strike is 3,584 bytes; it is not fitted to the reported caption.

Capture command:

```sh
cargo run --locked -p zebra-http-api --example extract-font -- \
  --host http://D7J211001302.bed.einic.org/ --font 0 --height 28 --width 0 \
  --dpi 203 --verify-text 'AVATAR Agj Wavy 123 _^~|!' /tmp/qr-font-0-28
```

`font-origins` independently checks FO/FT placement in all four orientations
with different text. The original label and both repeated captures are also
independent composition controls. The renderer selects the new strike only for
the exact 28×28 dimensions; other sizes retain their existing selection/fallback.
The font unit tests also verify enriched-glyph fallback at the new size.

Normative references: [Zebra ^BQ](https://docs.zebra.com/us/en/printers/software/zpl-pg/zpl-commands/%5Ebq.html)
documents automatic/manual switches, models and magnification; it does not
prescribe a unique automatic segment partition. [ISO/IEC 18004:2000](https://www.iso.org/standard/30789.html)
§§8.3–8.4 defines segment modes, character counts and bitstreams; §8.9 and
Annex C specify format bits. The font sampling follows the Zebra Programming
Guide's `^A`, `^FO`, and `^FT` commands. Printer segmentation is a compatibility
choice among valid encodings, not a claim that minimum-bit encoding violates QR.
