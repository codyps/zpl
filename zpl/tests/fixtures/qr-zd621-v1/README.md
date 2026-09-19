# ZD621 QR mask-selection controls

40 unmodified HTTP Preview Label responses from ZD621 203 DPI, firmware
V93.21.33Z, collected 2026-09-19 with zebra-render. Sources use PW832,
magnification two and requested mask zero.

The 32 original single-symbol frames cover both QR models, all four error-correction levels,
and four payload classes: one letter, fifteen digits, eight uppercase letters,
and mixed-case text containing digits. The printer chooses several different
masks despite the same requested mask. The encoded modules match the renderer
when the printer's mask is forced during diagnosis, but the regression test
renders the original input unchanged. Mask selection remains an accuracy gap.
Do not replace the general selection algorithm with a payload-specific lookup.

Eight byte-mode atlases add 96 symbols: both models and all correction levels,
with six payloads each encoded automatically and explicitly as bytes. Payloads
are `a`, `abcdefgh`, `hello`, `hello world`, `Hello QR`, and `Hello QR 123`.
The last payload changes segmentation in automatic mode. The other five have
the same encoding and mask in both input modes. These controls distinguish
selection effects of the encoded data from the input-mode operand itself.
Atlases use LL200, explicit BY2,3,10, eight 104-dot columns and 100-dot rows.

A separate encoding test reads the mask from the printer image's format bits,
checks their BCH remainder, and changes only the request's mask operand. It
requires an exact match across the entire raw printer image for all 128 symbols
in the 40 frames. For atlases it locates each symbol within its cell, changes
only the mask operands, and compares the full canvas without aligning images.
This isolates bitstream generation, error correction, module placement and
geometry from mask selection. It does not replace the unchanged-input test or
count diagnostic renders as end-to-end accuracy improvements.

The captured QR top edge is nine dots below FO60, matching the initial BY height
of ten dots. Recapture with that printer state; BY operands can persist between
HTTP requests. The renderer's specification and ZD621 profiles both start with
the documented height ten. No reference is translated or padded to compensate.

manifest.tsv pins source/response SHA-256, exact underpaint/overpaint, and the
complete local pixel hash. qr_preview.rs renders the unchanged source using
ZD621_203_DPI. Zero-error cases and known gaps are both pinned; passing the suite
does not claim that every QR symbol matches the printer.

For reproducible offline mask research, build the `zpl-to-svg` example, then run
`python3 scripts/analyze-qr-masks.py /tmp/qr-masks.json` from the repository root
with Pillow installed. The exporter checks capture hashes and format BCH bits,
verifies exact full-canvas rendering with the captured masks, and exports all
eight candidate module matrices per symbol. It does not contact the printer or
update fixtures, baselines, or renderer behavior. The JSON is diagnostic evidence,
not an accuracy result for unchanged requests.

References: Zebra Programming Guide ^BQ, pp. 128–131, and ^BY, p. 148;
ISO/IEC 18004:2000, section 8.8.2 and Annex M.8 for QR mask evaluation;
section 8.9 and Annexes C.1/M.9 for format-bit placement and decoding.
See docs/zpl-zbi2-pg-en.pdf and the
[Zebra command reference](https://www.zebra.com/us/en/support-downloads/knowledge-articles/ait/zpl-command-information-and-details.html).
