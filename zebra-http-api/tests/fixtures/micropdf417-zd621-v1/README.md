# MicroPDF417 printer probes

Captured 2026-09-15 using `zebra-render` against
`http://d7j211001302.bed.einic.org/`: ZTC ZD621-203dpi ZPL, V93.21.33Z.
Only HTTP previews were requested; nothing was physically printed.
PNGs are original printer responses, never local renders. `manifest.tsv` locks
each request and response with SHA-256. All requests specify PW832 and explicit
BY state, avoiding canvas padding and inherited printer state.

55 captures cover all 34 BF modes, pure numeric lengths 1–8 and 50, mixed text,
1/6/7-byte binary fields, a one-byte text shift, field escapes and automatic
heights at BY12/100/110 (including omitted and zero height).
Run `cargo test -p zebra-http-api --test micropdf417_preview` offline.
The test requires nonblank, exact full-image equality without alignment,
cropping, resampling or mismatch tolerance. Three older PW812 captures are
also checked with the independently established fixed 10-dot preview padding.

## References and observed choices

- [Zebra Programming Guide](../../../../docs/zpl-zbi2-pg-en.pdf), printed
  pp. 111–113: BF mode numbering and parameters. Mode 33 is 4 columns × 4 rows.
- [ISO/IEC 24728:2006](https://previewnorm.com/iec/ISO%20IEC%2024728-2006%20PDF.pdf),
  §§5.2.4, 5.4, 5.6–5.11 and Tables 10–12: initial Byte mode, compaction,
  codeword sequence, error correction and row addressing.

Decoded printer codewords establish text prefix 900, numeric prefix 902 even
for one digit, and binary prefixes 901/924. Firmware padding repeats
`900,838,779,867,865,898,868,839`: non-emitting text-state transitions. Three
columns repeat continuously; other column counts restart after 25 pad words.
These padding choices are firmware observations, not mandated ISO constants.
Automatic height is floor(BY height / rows), at least one dot, rather than the
literal height default described by the guide. Explicit positive heights are
row heights in dots. Printer state persists between previews, so a no-BY probe
was excluded in favor of explicit BY defaults.

Independent anyd tests decode all 34 rendered sizes and text/numeric/binary
payloads. Its MicroPDF417 decoder rejects codeword 913; that transition is
instead checked with rxing's shared PDF417 high-level decoder and an exact
printer fixture. Neither decoder is a runtime dependency.
