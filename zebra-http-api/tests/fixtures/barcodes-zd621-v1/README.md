# Real-printer barcode previews

The 60 original 812-dot-width captures were replaced by 832-dot-width previews
from the ZTC ZD621-203dpi ZPL, firmware V93.21.33Z, on 2026-09-18 UTC.
Each ZPL file is the exact request that produced its PNG. The manifest records
capture provenance, the separate layout-state reset before each case, hashes,
and current renderer observations. No physical printing was requested.

Run `cargo test -p zebra-http-api --test barcode_preview` for the existing
per-format regression gate. `cargo test -p zpl --test printer_accuracy` also
covers these files, the comparison argument suite and independent controls.
See [the accuracy contract](../../../../docs/printer-accuracy.md) for baseline
review requirements and remaining gaps. Both directional differences and pixel
hashes are pinned; improvements also require deliberate baseline updates.

For render/diff artifacts:

```sh
cargo run -p zebra-http-api --example barcode-compare -- \
  zebra-http-api/tests/fixtures/barcodes-zd621-v1 \
  --artifacts /tmp/barcode-review
```

Use a new, nonexistent directory. No network access occurs without `--capture`.
Capturing a new reference requires an independently checked model/firmware:

```sh
cargo run -p zebra-http-api --example barcode-compare -- \
  --capture --host http://PRINTER/ \
  --device 'MODEL; firmware VERSION' /tmp/barcode-new-capture
```

Capture performs 60 sequential preview submissions, their image fetches, and a
separate preview-state reset before each case. It uses `^PW832`, so these tests
need no printer-width padding or content alignment. Captures are original HTTP
responses, not local renderer output. A blank preview is not barcode parity.
The ignored `all_formats_match_printer_pixels` test and `--strict` option remain
an explicit, currently failing full-parity gate.
