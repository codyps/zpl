# Live proxy validation: 2026-09-30

Both printers were accessed through the named-printer proxy with separate queues,
serial pins and an isolated SQLite database. Only previews and automatic recovery
were exercised; no physical print job or firmware upload was submitted.

| Public name | Model | Serial | Firmware | DPI | Default native canvas |
| --- | --- | --- | --- | --- | --- |
| `ZD621` | ZD621 | REDACTED-ZD621 | V93.21.33Z | 203 | 832 × 240 |
| `ZQ610-Plus` | ZQ610 Plus | REDACTED-ZQ610-PLUS | V100.21.21Z | 203 | 384 × 2030 |

The [captures and manifest](../zpl-proxy-api/tests/fixtures/managed-printers-2026-09-30/manifest.json)
retain exact PNG bytes, original ZPL, SHA-256 hashes, request outcomes and identity-
stamped attempt records. This evidence is specific to these devices and firmware. Serial values in
this document are anonymized; the capture manifest retains original provenance.

## State isolation and cache

Each printer first rendered a label relying on default font, barcode, canvas and
layout settings. An intervening label changed print width, label length, origin,
shifts, orientation, mirroring, reversal, font, barcode defaults and encoding.
The original label was then rendered again with **refresh enabled**, so a cached
image could not conceal leaked state.

Both printers returned byte-identical before/after PNGs at their native dimensions,
without alignment, scaling, padding or cropping. A following non-refresh request
returned the same bytes with `X-ZPL-Cache: hit`. The ZQ610 Plus retained its
2030-dot canvas height for the intervening `LL160` request; that firmware-specific
observation is retained in the manifest, not treated as specification conformance.

All 10 accepted requests have the correct serial/firmware snapshot, including the
two cache hits. Identical source labels produced separate printer-scoped results.

## Observed real hang and autonomous recovery

The ZQ610 Plus timed out during its refreshed post-state-change preview. This was
an actual device HTTP preview timeout, **not an injected transport failure**.
No causal claim is made that the particular label sequence reliably reproduces it.
The proxy recorded and handled the sequence without a manual restart or retry:

| UTC time | Recorded phase/outcome |
| --- | --- |
| 04:28:29.264 | Label preview started |
| 04:28:59.268 | Preview timed out; `failure_kind = hang` |
| 04:28:59.270 | Automatic SGD restart started |
| 04:29:28.868 | Identity verified; exact control preview passed; recovery complete |
| 04:29:29.101 | Automatic label retry succeeded |

A separate read-only uptime query confirmed the printer had rebooted. The retried
label's PNG was byte-identical to its original successful preview. Because retry
succeeded, the label was **not** permanently cached as an error. The subsequent
request was a positive cache hit.

The ZD621 completed this sequence without a hang or restart. Its automatic restart
path was not triggered by this live test. Repeated hangs, permanent-error caching,
continued background recovery, restart throttling and disconnect independence
are tested with loopback simulation; do not describe them as live-tested behavior
on both models.
