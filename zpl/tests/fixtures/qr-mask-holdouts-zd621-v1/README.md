# Automatic QR mask holdouts

Eight unmodified ZD621 203-DPI V93.21.33Z previews contain 32 new symbols:
both models, L/M/Q/H correction, numeric, alphanumeric, mixed-case and 180-byte
payloads. Captured after deriving the staged mask rule from offline firmware;
none were used to choose weights or construct payload-specific rules.

All eight full canvases are pixel-exact. Sources and PNGs are hash-pinned;
JSON records a single successful submission. No cropping, alignment, pixel
editing or preview-width compensation was used. Requests use PW832 and
Connection: close, paced five seconds apart.

See [algorithm, references and evidence](../../../../docs/qr-mask-selection.md).
