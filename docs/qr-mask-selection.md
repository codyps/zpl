# Printer QR mask selection

The ZD621 profile enables `qr_printer_mask_selection`. It ignores the requested
mask, evaluates all eight candidate matrices including their format bits, and:

1. Stably ranks candidates by the penalty for equal 2×2 blocks.
2. Keeps only the three lowest-scoring candidates.
3. Adds run, finder-pattern, and dark-balance penalties to those three and
   selects the lowest total, retaining shortlist order on a tie.

The other penalties have observable arithmetic details: finder patterns accept
scaled 1:1:3:1:1 runs with four units of white on either side, counting once;
exterior white runs become 127. Run lengths use signed-byte arithmetic, which
matters for extended Model 1 symbols. The balance counter omits black runs
reaching the right edge, divides by `floor(area / 100)`, then subtracts 50,
divides by 5 toward zero, takes the absolute value, and multiplies by 10.

SPECIFICATION leaves this option disabled and honors the documented mask
operand. Existing explicit-mask tests independently verify encoding, BCH format
bits, and placement with automatic selection disabled.

## Evidence

The general rule was identified by offline inspection of the public legacy
[GX430t V56.17.17Z firmware](https://www.zebra.com/content/dam/support-dam/en/firmware/unrestricted/0001/v56-17-17z.exe),
then checked against current ZD621 203-DPI V93.21.33Z captures. The legacy image
was never sent to the printer. No vendor binary or decompiled source is included.

The decompressed image is 6,488,064 bytes with SHA-256
`a512e2b37f3e615337823ffb16f3e2ffb5c568a38af6abe710772841d6dd0380`.
In the SuperH image loaded at `0x8c000000`, the useful routines
are `0x8c1a7c74` (three-candidate selection), `0x8c1a7a90` (2×2 blocks), and
`0x8c1a7de8` (remaining penalties). The latter includes signed-byte run loads
and integer balance arithmetic at `0x8c1a81b4` onward. These are evidence for
an algorithm, not an assumption that all legacy and current behavior agrees.

The initial offline rule matched all 170 diagnostic mask choices. Eight new
native holdout frames contain 32 symbols spanning both models, all four error
correction levels, and new numeric, alphanumeric, mixed-case, and longer byte
payloads. Whole-frame tests use unchanged printer PNGs and ZPL with explicit
source/output hashes. Extended Model 1 and structured-append/Kanji tests also
require exact whole-frame automatic selection; their explicit-mask controls
remain separate.

Normative background: ISO/IEC 18004:2000 §8.8.2 defines the penalty categories
([ISO catalog](https://www.iso.org/standard/30789.html)); the
[Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf)
`^BQ`, pp. 129–134, documents the mask operand. The staged selection and integer
departures above are printer compatibility behavior, not specification defaults.
