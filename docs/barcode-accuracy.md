# Barcode preview inventory

`cargo test --locked -p zpl --test barcode_accuracy` audits saved barcode
previews under `zpl/tests/fixtures` and `zebra-http-api/tests/fixtures`.
It parses commands rather than searching payload text, and requires positive
coverage of all 29 barcode commands separately for each printer model. No
network requests occur in this test.

The audit uses `ZD621_203_DPI` for ZD621 V93.21.33Z captures and
`ZQ610_PLUS_203_DPI` for ZQ610 Plus V100.21.21Z captures. The fixture READMEs,
JSON capture records and manifests retain device identity and acquisition
provenance. The conformance and original barcode campaigns' separate reset
is replayed before their candidate label. Other captured inputs carry their
own setup. Native dimensions and origin are preserved, without padding,
cropping, alignment or rescaling.

The aggregate foreground error is `1 - intersection / union`, measured at
threshold 128. The positive-frame inventory requires error strictly below
20% (foreground IoU strictly above 80%). Unlisted positive frames must also
remain pixel-exact; individually reviewed text residuals and known gaps are
pinned separately. This is a whole-frame audit, not proof of individual symbol geometry or decoded payloads. Mixed text, multiple
symbols, clipping and reversed fields need their dedicated tests. Existing
pixel-exact, per-field, hash and decoder gates remain authoritative; this
inventory does not replace or relax them. See [printer accuracy](printer-accuracy.md)
and [barcode coverage](barcodes.md).

Blank previews, rejected requests, undecodable finder-only symbols and unstable
native output are diagnostic observations, not successful barcode evidence. Individual exceptions in
[`barcode-accuracy-gaps.tsv`](../zpl/tests/fixtures/barcode-accuracy-gaps.tsv)
pin classifications with reasons, source and printer hashes, canvas dimensions,
local pixel hashes, exact directional errors and renderer diagnostics.
A changed observation or a stale exception fails the audit. Never add an
exception merely to make a failure pass: first inspect its capture context and
its dedicated regression. In particular, the empty-QR campaign demonstrates
changing native output for identical requests, which a deterministic renderer
cannot reproduce. MaxiCode mode 5 previews contain only finder/orientation
marks, so they are also diagnostic even when their pixels match exactly.

The reviewed inventory has 1,647 frames: 1,585 exact positive frames, thirteen
positive text-residual frames, three unmet accuracy targets, and 46 diagnostic
controls. The original three shipping-label residuals still have 6 underpaint
and 9 overpaint pixels each. The original diagnostics comprise 26 blank previews
(eight rejected inputs and eighteen rendered blanks), seven unstable empty-QR
captures, and nine finder-only MaxiCode mode 5 captures. The 22 added public
documents contribute five exact frames, ten positive text-residual frames,
three known gaps, and four malformed-barcode diagnostics. Counts describe saved previews, not fresh device measurements or
physical printing. The source inventory count is pinned to prevent silent loss
of cases; extensions require deliberate review.

The [public-document campaign](../zpl/tests/fixtures/public-zpl-zd621-v1/README.md)
retains existing QR-version and PDF417-dimension differences in two Labelixa
labels, plus the Shopify label's text error above 20%. These are explicitly
unmet targets, including under `ZPL_BARCODE_STRICT=1`; importing the captures
does not claim to fix their encoding or fonts. Four BinaryKits documents contain
a truncated QR payload, literal Code 39 template placeholders, or clipped BY12
bars that did not decode in the archived audit. Their acceptance is tested without
counting them as positive barcode evidence. Every new residual has its own
reason and immutable observation in the gaps manifest, and the dedicated
`public_zpl_preview` test pins all 22 full canvases and the affected non-text regions.

The shipping-label residuals are checked against per-field foreground masks and
exact non-text pixels in `conformance_preview` and `shipping_fonts_preview`.
The other relevant dedicated gates include `barcode_modes_preview`,
`databar_retail_preview`, `maxicode_preview`, `empty_qr_preview`,
`zq610_preview`, and the `zebra-http-api` crate's `pdf417_preview`.

For a tab-separated report and optional rendered/difference PNGs:

```sh
ZPL_BARCODE_REPORT=/tmp/barcodes.tsv \
ZPL_BARCODE_ARTIFACTS=/tmp/barcode-images \
  cargo test --locked -p zpl --test barcode_accuracy -- --nocapture
```

These outputs are diagnostics; they never update the saved references or
exceptions. `ZPL_BARCODE_STRICT=1` additionally rejects pinned accuracy gaps;
expected diagnostic controls do not become positive evidence in strict mode.
Run the stronger main and conformance gates alongside this inventory:

```sh
cargo test --locked -p zpl --test barcode_accuracy \
  --test printer_accuracy --test conformance_preview
```
