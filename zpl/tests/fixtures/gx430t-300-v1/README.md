# GX430t 300-DPI native preview evidence

Printer: Zebra GX430t, serial `32J193600198`, firmware `V56.17.17Z`,
12 dots/mm (marketed 300 DPI). Captured 2026-10-18 through its legacy HTTP
**Preview Label** form at `192.168.6.142`, whose MAC matched the USB identity.
Networking was already enabled and reachable; no static address was imposed.
These are original firmware PNGs, not photographs of physical output.

Use `zpl::render::profiles::GX430T_300_DPI` or CLI `--profile gx430t`.
The native canvas has 2558 rows, a 1280-dot maximum width, 64-dot width
rounding and centered content. PW latches at the first draw; LL is ignored by
this preview service. Logical-width clipping retains an extra right dot when
PW modulo 64 is 1. Capture controls verify these independently.

## Fixed regression corpus

| Campaign | Native frames | Purpose |
| --- | ---: | --- |
| main | 166 | Same 164 main cases as ZD621, plus repeated controls |
| conformance | 514 | Complete 512-case corpus plus repeated controls |
| dpi-controls | 159 | Width boundaries; OCR E/H sizes, FO/FT and four rotations |
| boundary-controls | 24 | Additional widths and safe-origin OCR holdouts |
| bitmap-edge-holdouts | 50 | Independent zero/near-origin OCR and font A controls |
| ocr-justification | 34 | OCR right justification at native and double sizes |
| retail-captions | 50 | Four retail families, six module widths, above/below |
| canvas-holdouts | 11 | Late PW and varied LL controls |
| font-probes | 10 | Initial font-0 calibration pages |
| extra-font-probes | 36 | Independent anisotropic sizes and Unicode pages |
| preset-font-probes | 11 | P through V and remap-independent holdouts |
| latin-holdouts | 14 | Legacy cent/backslash, PA1, MaxiCode FO/FT and FW settings |
| maxicode-a-runs | 16 | Short A runs, spaces and shared punctuation |

The 524 main/density/edge/caption/canvas/Latin/MaxiCode frames are full-image pixel-exact.
Conformance has 475 exact nonblank frames (including two controls), 24 frames
with text-only residuals, nine blank diagnostics, one invalid UPC-E input and
five unsupported FE cases. Every non-text pixel matches. The minimum of 28
text-field foreground IoUs is **1462/1470 = 99.4558%**, exceeding the existing
ZD621 minimum of 93.33%. White text is measured as white foreground within
its explicit field region. Full native canvases retain their original origin:
no registration, padding, cropping or rescaling enters the comparison.

`baseline.tsv` pins exact underpaint/overpaint/intersection counts, raster
hashes and error offsets. `text-regions.tsv` pins each foreground intersection
and union, with exact equality outside these regions. Blank images, finder-only mode-5 MaxiCode previews, native barcode error
labels, and literal FE fallbacks are diagnostic observations, not proof of
successfully encoded symbols.
GX430t rejects FE explicitly; modern specification/ZD621 concatenation remains
available. See Zebra's [FE supported-printer list](https://docs.zebra.com/us/en/printers/software/zpl-pg/zpl-commands/%5Efe.html).

Every campaign retains submitted ZPL, original PNG and acquisition metadata with
times, dimensions, image URLs and SHA-256 digests. Repeated preview-response HTML
is omitted; `omitted_html_sha256` preserves its original acquisition hashes.
`main/config-start.html` and `main/status-start.html` retain one original printer
state pair; all other campaigns' state pages were byte-identical to these. The
`files` map hashes retained files only. Start/end
controls are byte-identical within every campaign. Capture scripts ran serially
against the shared preview object and followed the dynamic returned IMG link.
No physical print was requested. Early head-open/paused attempts filled the
legacy queue and stalled HTTP; these failed/blank observations were discarded
as positive evidence. After authorized reset, closing the head, canceling the
queue and resuming, all retained campaigns completed. No media load was needed.

An intentional CI0 remap case contaminated implicit barcode captions later in
the initial conformance run. The 190 affected cases were reacquired with a full
CI0/CI13 identity-map reset before each request. The canonical manifest records
which campaign supplied each case and retains both original campaign receipts.
GX430t short MaxiCode set-A runs stop at spaces while shifted from B;
14 separate payload controls verify this choice, including the 3/4-character
latch boundary. Commas, colons and periods remain inside those short runs. An independent
decoder also checks the payloads of both the native and local images for all
14 sweep cases and the original mixed numeric holdout.
No locally rendered image replaces a printer reference. Fixed font calibration
and the rendering-required normalized strikes are in `zpl-bitmap-fonts`; growing
sampling/extraction research belongs in the separate font-extract repository.

Run offline:

```sh
cargo test --locked -p zpl --test gx430t_preview --test printer_accuracy --test conformance_preview
```

This evidence covers this firmware, configuration and fixed repertoire. It does
not establish exactness for every unsampled font size, glyph or printer model.
