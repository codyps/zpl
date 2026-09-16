# Code 11, Code 49, Code 93 and Plessey previews

39 original HTTP previews captured 2026-09-15 from
`http://printer.local/`, ZTC ZD621-203dpi ZPL, V93.21.33Z,
using `zebra-render`. No physical print jobs were submitted. The first 18 images
were captured during diagnosis, before implementation changes; the remaining
21 extend coverage for numeric boundaries and ZPL field semantics.

Every `.zpl` is the exact request that produced its `.png`; `manifest.tsv`
locks both with SHA-256. PW832 and explicit BY state eliminate preview padding
and inherited barcode defaults. Offline test:

```sh
cargo test -p zebra-http-api --test linear_fixes_preview
```

All 39 images must match exactly, without registration, cropping, resampling,
or tolerance. The same test separately checks the four original PW812 captures
using the independently established 10-dot HTTP preview padding.

## Evidence and references

- [Zebra guide](../../../../docs/zpl-zbi2-pg-en.pdf), pp. 66, 74–77, 87–89,
  126: command parameters, numeric-mode selection, Code 93 shift substitutes,
  and Plessey check-display semantics.
- [USS Code 49](https://www.expresscorp.com/wp-content/uploads/2023/02/USS-49.pdf),
  §§2.1, 2.2.2, 2.3 and 3.4: row structure, numeric tail groups, starting modes,
  and quiet zones.
- [Plessey overview](https://barcodeguide.seagullscientific.com/content/Symbologies/Plessey.htm).

Code 11 probes cover all digits/dash, one/two checks, and ratios 2/3. The printer
uses `(2r−1)X` for the extra-wide element in 0, 9 and dash. Plessey probes cover
both ratios and short data: wide elements follow BY, and the termination bar is
`(r+1)X`. CRC bits are unchanged; the check-display flag does not change bars.

Code 93 captures distinguish raw ASCII from ZPL substitutes: `Hello93!` decodes
as `HELLO93`, while `)A)B)C(A` decodes as `abc!`. Uppercase, discarded input,
FH-decoded substitutes, and malformed shift sequences are also captured. The
malformed sequence fixture reproduces the printer's bars, not a readable symbol.
Independent rxing tests exercise all 128 ASCII values through valid ZPL pairs.

Code 49 probes include short text, lowercase initial shift, pure and mixed
numeric data, leading zeroes, each numeric tail form, and 80/81-digit symbols.
Outer separators span 81X; rows and internal separators span 70X at offset 10X.
Numeric compaction is checked against USS examples and printer captures.

## Decoder limitations

The existing anyd Code 11 decoder only accepts binary narrow/wide patterns;
its test adapter normalizes extra-wide bars to wide. Exact printer tests check
the physical widths independently. The anyd Code 49 decoder cannot reconstruct
numeric payloads: numeric starting-mode tests assert that specific unsupported
result, not a successful round trip. Alphanumeric/full-ASCII rows still round
trip through anyd; numeric content is covered by specification vectors and exact
printer images. Both decoder libraries remain test-only dependencies.

Human-readable typography is not tested for exact printer parity here. Plessey
check-display content and Code 93 interpretation are covered by local unit tests.
