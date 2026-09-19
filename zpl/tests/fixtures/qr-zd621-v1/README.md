# ZD621 QR mask-selection controls

32 unmodified HTTP Preview Label responses from ZD621 203 DPI, firmware
V93.21.33Z, collected 2026-09-19 with zebra-render. Sources set PW832, LL400,
label/origin state, magnification two and requested mask zero.

The cross product covers both QR models, all four error-correction levels,
and four payload classes: one letter, fifteen digits, eight uppercase letters,
and mixed-case text containing digits. The printer chooses several different
masks despite the same requested mask. The encoded modules match the renderer
when the printer's mask is forced during diagnosis, but the regression test
renders the original input unchanged. Mask selection remains an accuracy gap.
Do not replace the general selection algorithm with a payload-specific lookup.

A separate encoding test reads the mask from the printer image's format bits,
checks their BCH remainder, and changes only the request's mask operand. It
requires an exact match across the entire raw printer image for all 32 cases.
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

References: Zebra Programming Guide ^BQ, pp. 128–131, and ^BY, p. 148;
ISO/IEC 18004:2000, section 8.8.2 and Annex M.8 for QR mask evaluation;
section 8.9 and Annexes C.1/M.9 for format-bit placement and decoding.
See docs/zpl-zbi2-pg-en.pdf and the
[Zebra command reference](https://www.zebra.com/us/en/support-downloads/knowledge-articles/ait/zpl-command-information-and-details.html).
