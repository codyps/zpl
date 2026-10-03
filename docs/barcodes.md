# Local barcode encoders

The new encoders are original Rust implementations, not ports or vendored encoder
libraries. Each symbology has its own module under
[`zpl/src/render/barcode`](../zpl/src/render/barcode/). Small helpers share bit
packing, Reed–Solomon arithmetic, retail digit patterns, and 2-of-5 widths. The
existing Code 39 and Code 128 implementations were moved into their own modules.
No runtime dependency was added. `rxing` and `anyd` are **dev-dependencies only**;
their encoder features are disabled.

This is a preview renderer, **not complete coverage of every ZPL barcode mode**.
The explicit gaps below must not be treated as successful barcode generation.
Decoder round trips are interoperability checks, not printer certification or
verification of print tolerances, quiet zones, postal acceptance, or GS1 rules.

## Command and specification references

The page numbers below are the **printed page numbers** in the
[bundled Zebra ZPL II Programming Guide](zpl-zbi2-pg-en.pdf), not PDF viewer page
indices. That guide specifies the command defaults and field-data syntax; the
linked symbology documents describe encoding. ISO catalogue pages identify the
standard, but do not imply that a full purchased standard was available. Legacy
editions are named deliberately; this is not a claim of conformity to subsequent
revisions. For older proprietary codes, the Zebra guide is the available command
specification; no separate complete normative specification was obtained.

| Code / command | Module | Zebra pages | Symbology specification / documentation |
| --- | --- | --- | --- |
| Aztec / `B0`, `BO` | `aztec.rs` | 64, 124 | [ISO/IEC 24778](https://www.iso.org/standard/82441.html) |
| Code 11 / `B1` | `code11.rs` | 66 | [Zebra guide, Code 11](zpl-zbi2-pg-en.pdf); weighted C/K checks |
| Interleaved 2 of 5 / `B2` | `interleaved2of5.rs` | 68 | [ISO/IEC 16390:2007](https://www.iso.org/standard/43898.html) |
| Code 39 / `B3` | `code39.rs` | 70 | [ISO/IEC 16388:2007 preview](https://cdn.standards.iteh.ai/samples/43897/358ed85e97e14c3e81d30f621f57ec34/ISO-IEC-16388-2007.pdf) |
| Code 49 / `B4` | `code49.rs` | 74 | [USS Code 49](https://www.expresscorp.com/wp-content/uploads/2023/02/USS-49.pdf), including Appendix F character patterns |
| PLANET / `B5`, `BZ` type 1 | `planet.rs` | 78, 150 | [Zebra guide, PLANET and POSTAL](zpl-zbi2-pg-en.pdf); legacy two-height postal encoding |
| PDF417 / `B7` | `pdf417.rs` | 79 | [USS PDF417](https://www.expresscorp.com/wp-content/uploads/2023/02/USS-PDF-417.pdf); [ISO/IEC 15438](https://www.iso.org/standard/65502.html) |
| EAN-8 / `B8` | `ean8.rs` | 83 | [ISO/IEC 15420:2009](https://www.iso.org/standard/46143.html), EAN/UPC |
| UPC-E / `B9` | `upce.rs` | 85 | [ISO/IEC 15420:2009](https://www.iso.org/standard/46143.html), zero suppression and parity |
| Code 93 / `BA` | `code93.rs` | 87 | [Zebra guide, Code 93](zpl-zbi2-pg-en.pdf); full ASCII shifts and C/K checks |
| CODABLOCK / `BB` | `codablock.rs`, `codablock_a.rs` | 90 | [AIM ISS CODABLOCK F](https://web.aimglobal.org/external/wcpages/wcecommerce/eComItemDetailsPage.aspx?Category=9&ItemID=236); [BarTender documentation](https://barcodeguide.seagullscientific.com/Content/Symbologies/Codablock_F.htm) |
| Code 128 / `BC` | `code128.rs` | 94 | [Zebra guide, Code 128](zpl-zbi2-pg-en.pdf); ISO/IEC 15417 |
| MaxiCode / `BD` | `maxicode.rs` | 106 | [ISO/IEC 16023:2000](https://www.iso.org/standard/29835.html); [preview including Figure 5](https://cdn.standards.iteh.ai/samples/29835/8a88c580bf31467788aeb63128a1e9f7/ISO-IEC-16023-2000.pdf); [Toshiba character-set tables](https://www.toshibatec.co.jp/products/manual/BV410T31_interface-barcode.pdf), printed pages 478–479 |
| EAN-13 / `BE` | `ean13.rs` | 109 | [ISO/IEC 15420:2009](https://www.iso.org/standard/46143.html) |
| MicroPDF417 / `BF` | `micropdf417.rs` | 111 | [ISO/IEC 24728:2006](https://previewnorm.com/iec/ISO%20IEC%2024728-2006%20PDF.pdf), Tables 2 and 10–12 |
| Industrial 2 of 5 / `BI` | `industrial2of5.rs` | 114 | [Zebra guide, Industrial 2 of 5](zpl-zbi2-pg-en.pdf) |
| Standard 2 of 5 / `BJ` | `standard2of5.rs` | 116 | [Zebra guide, Standard 2 of 5](zpl-zbi2-pg-en.pdf) |
| ANSI Codabar / `BK` | `codabar.rs` | 118 | [BarTender Codabar documentation](https://barcodeguide.seagullscientific.com/Content/Symbologies/Codabar.htm); [Zebra guide](zpl-zbi2-pg-en.pdf) |
| LOGMARS / `BL` | `logmars.rs` | 120 | [Zebra guide, LOGMARS](zpl-zbi2-pg-en.pdf); Code 39 with mandatory modulo-43 check |
| MSI / `BM` | `msi.rs` | 122 | [Zebra guide, MSI](zpl-zbi2-pg-en.pdf), including the four check selections |
| Plessey / `BP` | `plessey.rs` | 126 | [BarTender Plessey documentation](https://barcodeguide.seagullscientific.com/content/Symbologies/Plessey.htm); [Zebra guide](zpl-zbi2-pg-en.pdf) |
| QR / `BQ` | `qr.rs` | 128 | [ISO/IEC 18004:2015](https://www.iso.org/standard/62021.html); Thonky [error-correction tables](https://www.thonky.com/qr-code-tutorial/error-correction-table) and [alignment positions](https://www.thonky.com/qr-code-tutorial/alignment-pattern-locations) |
| GS1 DataBar / `BR` | `databar.rs`, `databar_stacked.rs`, `databar_limited.rs`, `databar_expanded.rs` | 135 | [ISO/IEC 24724:2011](https://www.iso.org/standard/51426.html), sections 5–7 and Annex C; [GS1 overview](https://www.gs1.org/standards/barcodes/databar) |
| GS1-128 Composite / `BR11`, `BR12` | `composite.rs`, `composite_a.rs`, `composite_b.rs`, `composite_c.rs`, `gs1_128.rs` | 135–136 | [ISO/IEC 24723:2010](https://www.iso.org/standard/51425.html), sections 5–12, Tables 8–12; [Zebra command documentation](https://docs.zebra.com/us/en/printers/software/zpl-pg/zpl-commands/%5Ebr.html); the bundled guide's illustrated page 136 shows `linear|2D` field syntax |
| UPC/EAN extensions / `BS` | `upc_extension.rs` | 137 | [ISO/IEC 15420:2009](https://www.iso.org/standard/46143.html), two- and five-digit supplements |
| TLC39 / `BT` | `tlc39.rs` | 140 | [Zebra guide, TLC39](zpl-zbi2-pg-en.pdf); [US20010045461A1](https://patents.google.com/patent/US20010045461A1/en), paragraphs 0024–0029 (T flag, placement, linkage codeword 918, GS separators) |
| UPC-A / `BU` | `upca.rs` | 142 | [ISO/IEC 15420:2009](https://www.iso.org/standard/46143.html) |
| Data Matrix / `BX` | `data_matrix.rs`, `data_matrix_legacy.rs` | 144 | [ISO/IEC 16022:2000](https://previewnorm.com/iec/ISO%20IEC%2016022-2000%20PDF.pdf), legacy §§5.2–5.7 and Annexes B–F; ECC200 §6 |
| POSTNET / `BZ` type 0 | `postnet.rs` | 150 | [Zebra guide, POSTAL](zpl-zbi2-pg-en.pdf); legacy two-height postal encoding |
| Intelligent Mail / `BZ` type 3 | `intelligent_mail.rs` | 150 | [USPS-B-3200 Rev H](https://postalpro.usps.com/storages/2017-08/2190_USPSB3200IntelligentMailBarcode4State_0.pdf), sections 2.2.1–2.2.6 and Appendix E Table 22; [published USPS vector](https://about.usps.com/kits/kit600/kit600_v04-2026_020.htm) |

The normative numeric lookup data in `code49_patterns.rs`,
`pdf417_patterns.rs`, and `maxicode_modules.rs` was transcribed from the linked
specification tables/figures, not extracted from another encoder. Code 49's
scanned tables were checked for widths, parity, and uniqueness. QR block sizes
were transcribed from the linked tutorial tables. Algorithm implementations are
local and original; published character assignments are interoperability data.

The user supplied the complete BS ISO/IEC 24724:2011, BS ISO/IEC 16023:2000,
and BS ISO/IEC 24723:2010 editions for the DataBar, MaxiCode, and GS1 Composite
follow-ups. DataBar Limited's 89 check-pattern
sequence numbers are normative Annex C data; its bounded-width enumeration is
original, not a port of the standard's Annex B reference C implementation.
MaxiCode dimensions use section 4.11, Tables 6–8 of the full 2000 edition.
CC-A uses the four-column assignments from Tables 9 and 11 of ISO/IEC 24723;
its base-928 conversion uses local `u128` arithmetic, not Annex E's sample code.
`gs1_compaction.rs` shares the general-purpose numeric/alphanumeric/ISO-646
algorithm between Expanded and Composite. CC-C error correction meets the
minimum recommendations in [USS PDF417](https://www.expresscorp.com/wp-content/uploads/2023/02/USS-PDF-417.pdf), Table 7.
The supplied PDFs remain outside the repository and are not redistributed here.

## Implemented scope and remaining gaps

- Linear: Code 11 C/K, three 2-of-5 forms, LOGMARS, Codabar delimiters, full-ASCII
  Code 93, MSI check modes A–D, Plessey CRC, EAN/UPC checks and supplements.
  Invalid supplied retail checks are rejected, not silently replaced. Optional
  Codabar check modes are not implemented. Code 93 supports optional C/K
  interpretation, with printer-specific extended-check formatting controlled
  by `code93_extended_checksum_preview`. Plessey always encodes its CRC; `BP`'s `e` flag
  only controls displaying its two hexadecimal check digits. Plessey honors the
  `BY` ratio and uses a wide-plus-narrow termination bar. Code 11 uses
  nominal extra-wide `(2r−1)X` elements in the specification profile; the
  printer profile independently truncates wide `rX` and extra-wide `5rX/3`
  elements in 0, 9 and dash. Code 93 interprets Zebra's
  `&`, `'`, `(` and `)` shift substitutes after `FH`. With the specification profile, unsupported raw bytes are errors. The ZD621 profile
  uppercases raw lowercase and skips unsupported bytes. Literal full
  ASCII must be expressed through the documented substitute pairs.
  Code 39 supports its optional Mod-43 checksum and above/below interpretation.
  Code 128 supports ZPL subset A/B/C starts and switches, SHIFT/FNC1–3, automatic
  ASCII compaction, UCC Mod-10 and case mode U, and parenthesized GS1 mode D
  with AI 00/01/02 key checks. Extended-byte FNC4 remains unsupported.
  Default interpretation uses captured resident A; explicit ^A selection is supported.
- QR: Model 1 versions 1–14 and Model 2 versions 1–40; all four error levels and eight requested masks;
  automatic optimization across numeric/alphanumeric/byte segments and manual
  `N`, `A`, `Bdddd` inputs. No Kanji, mixed manual segments, ECI, or
  structured append. The field's error-level switch selects error correction.
- Data Matrix: ECC200 ASCII, C40, Text, X12, EDIFACT and Base256 encodation,
  digit pairs, upper shift, square sizes 10–144 and six rectangular sizes.
  FNC1, doubled escape and escaped ASCII control characters are supported;
  other ECC200 function escapes remain rejected. Explicit `BX` escape operands
  take precedence; the specification profile defaults to underscore, while the
  ZD621 profile uses the captured tilde default. Its EDIFACT transition choices
  are a separate compatibility option. Encoding follows ISO/IEC 16022:2006
  §§5.2.4–5.2.9 and Annex P; both supplied technical corrigenda were checked.
  Legacy ECC 000/050/080/100/140 supports all six input formats, CRC, convolutional
  correction and odd square sizes 9–49; omitted quality selects ECC 000.
  Legacy payload length is bounded at 511 characters by its nine-bit length field.
  The legacy format selector is unused with ECC200, as documented by Zebra.
- Aztec: compact/full symbols, fixed layer counts, percentage selection, and
  runes; original shortest-path Upper/Lower/Mixed/Punctuation/Digit and binary
  encodation, including punctuation pairs. Default parity reserves 23% of symbol
  capacity plus three words. The printer profile truncates the default fractional
  requirement and preserves contiguous non-text binary runs; explicit percentage
  requests round up. No ECI, structured append, or reader
  initialization. The captured controls are exact; arbitrary firmware/payload parity is not guaranteed.
- PDF417: text, numeric and byte compaction (including one-byte shifts), ECC
  levels 0–8, requested/automatic dimensions and truncated layout. `\&` and
  doubled-backslash field escapes are decoded after `FH`. Omitted row height
  divides the `BY` overall height by the row count, rounding down to whole dots
  with a one-dot minimum. Explicit row height is dots on the tested ZD621, despite
  contradictory multiplier wording in the guide. Automatic dimensions target
  nominal 2:1 width:height at Y=3X, before applying the requested row height.
  The ZD621 profile compares integer grids instead of rounding the continuous
  column estimate, and latches Text into Punctuation for runs of four or more
  punctuation characters. These choices have independent compatibility options;
  [60 native controls](../zpl/tests/fixtures/pdf417-layout-zd621-v1/README.md)
  pin layout transitions and punctuation handling. The Labelixa carrier symbol
  matches the captured 308×132-dot PDF417 exactly (previously 274×162 dots).
  Numeric runs of at least eight digits follow the sampled ZD621 threshold.
  Compaction is deterministic, not globally optimal or guaranteed identical to
  every firmware for every payload. Macro PDF417 is supported through `^FM`; ECI remains unsupported.
- MicroPDF417: all 34 `BF` modes in Zebra order (mode 33 is 4×4), text/numeric/
  byte compaction and byte shifts. The initial mode is Byte, so text requires
  codeword 900 (ISO/IEC 24728 §§5.2.4, 5.4). `\&` and doubled backslashes are
  interpreted after `FH`. Omitted/zero row height divides `BY` overall height
  by the symbol's rows, rounding down with a one-dot minimum on the ZD621;
  this differs from the guide's stated height default. Pure numeric fields use
  numeric compaction even for one digit. Padding uses the observed no-output
  text-latch sequence, including its column-dependent restart. See the
  [real-printer probes](../zebra-http-api/tests/fixtures/micropdf417-zd621-v1/README.md).
  These are sampled firmware choices, not a guarantee of identical compaction
  for all payloads/firmwares. `^FM` structured append is supported; ECI remains unsupported.
  TLC39 and composite CC-B retain their separate compaction/padding paths.
- Multiple origins (`^FM`): up to 60 coordinate pairs for PDF417/MicroPDF417,
  independent per-segment compaction, file ID, segment index/count and final
  terminator (USS PDF417 Appendix G). Excluded pairs consume a segment without
  painting it; too few origins produce no symbols, as specified by Zebra p. 199.
  Other field types ignore `^FM`. The printer profile selects the captured
  file ID [0,0,36], MicroPDF417 reverse-origin adjustment;
  specification options derive a stable file ID from the payload and use the
  complete symbol extent. File IDs are grouping aids, not globally unique IDs.
- Code validation (`^CV`): valid fields render normally; recognized character,
  check-digit, length and parameter errors produce reverse INVALID fields.
  State persists across labels within a render call. The printer profile enables
  captured retail/legacy Data Matrix error-code departures. Unimplemented encoder
  semantics remain renderer errors. This does not emulate firmware internal-error
  panels or every firmware-specific data normalization path.
- Code 49: full ASCII, 2–8 rows, row and symbol checks. Automatic mode compacts
  digit runs of five or more with the USS base-48 rules, including special tail
  groups, and selects initial numeric/shift modes. Capacity reaches 81 digits.
  Manual internal modes 0–5 remain accepted without automatic compaction.
  Rows include a 10X leading and 1X trailing quiet zone; outer separator bars
  extend across them, while internal separators span only the 70X row.
  Default interpretation uses captured resident A; explicit ^A selection is supported. See the
  [four-code printer regressions](../zebra-http-api/tests/fixtures/linear-fixes-zd621-v1/README.md).
- CODABLOCK A: Code 39 alphabet, 1–22 rows, automatic/explicit row and column
  sizing, row indicators, padding and optional modulo-43 block checks. The
  ZD621 profile selects its unscaled row-height operand and 16-bit checksum
  accumulation; specification settings use module-scaled height and mathematical
  sums. F/E: ASCII A/B switching, B/C padding, 2–44 rows, subset-dependent row/symbol checks including the implicit mode-E FNC1; internal and outer separators. ZD621 options select unscaled row heights and data-fitted rows. Special ZPL
  function escapes are rejected. No optimized set C.
- MaxiCode: modes 2–6, structured carrier headers, byte character sets, nine-digit
  compression, A/B latches, two/three-character shifts, C/D/E lock-in, and
  primary/secondary Reed–Solomon checks. Compaction follows ISO 16023 Annex F.
  Specification geometry uses nominal physical dimensions; the ZD621 profile
  selects calibrated dot geometry, a terminal latch, NUL termination, mode 4/6
  minimum length of six bytes, and mode-5 finder-only previews. That last option
  reproduces a firmware defect and deliberately omits barcode data; disable it
  to obtain a complete symbol. No claim of UPS label compliance.
- DataBar: Omnidirectional/Truncated (1/2), Stacked/Stacked Omnidirectional (3/4),
  Limited (5), Expanded/Expanded Stacked (6), and EAN/UPC aliases (7–10).
  The printer profile uses captured fixed UPC/EAN proportions and requires eleven
  uncompressed UPC-A digits for `BR8`; all four zero-suppression forms are supported.
  The specification profile also accepts compressed UPC-E input. Retail aliases
  ignore the height operand, which applies only to composite GS1-128 components.
  Expanded supports compressed GTIN, weight, price and date methods (ISO/IEC
  24724:2011 §7.2.5.4) plus general-purpose method 00, numeric/alphanumeric/ISO-646 switching,
  and 2–22 even-numbered segments per row. Some printer separator rows and
  malformed long-weight/no-date outputs still differ from the standard encoding.
  Supply Expanded data as a raw GS1 AI element string, not parenthesized display
  text; use `^FH` with `_1D` for internal FNC1 separators. Application-level GS1
  AI validation is caller-owned. Linked composite forms of DataBar/retail variants
  1–10 are not implemented; a `|` composite payload on those variants is rejected.
- GS1-128 Composite: `BR11` selects four-column CC-A, falling back to four-column
  CC-B when the payload exceeds CC-A capacity; `BR12` selects CC-C. Each has
  its own module. GS1-128 uses B/C switching, leading FNC1, and the type-specific
  linkage character. Separator bars and horizontal alignment follow sections 11–12.
  The 2D payload uses general-purpose method 0, not the optional date/lot/AI-90
  specialized compression, so capacity and size may differ from a printer.
  CC-C chooses columns to fit the linear component's width and quiet zones;
  it does not widen beyond that bound to accommodate oversized payloads.
  Supply `linear|2D` raw AI element strings; `^FH` `_1D` supplies FNC1 within
  either component. AI validation, quiet zones, and human-readable text are
  caller-owned. No ECI, symbol-separator transmission mode, or component escape
  mechanism is supported. Primary input is bounded at 200 bytes; secondary
  input is bounded at 2361 bytes and additionally by the selected symbol capacity.
- TLC39: six-digit Code 39 alone, or linked four-column MicroPDF417 above it,
  with the isolated T flag and linkage codeword 918. Comma-separated serial and
  additional fields become GS-separated bytes in SPECIFICATION, or asterisk-separated
  data in the ZD621 profile. Text, numeric and byte compaction share the PDF417
  encoder. Printer layout and the extended link flag have independent options.
  Long supplementary payloads can still select a different row count from firmware.
- Postal: POSTNET 5/9/11 digits, PLANET 11/13 digits, and Intelligent Mail with
  20 tracking plus 0/5/9/11 routing digits. POSTNET/PLANET are legacy codes;
  rendering them does not establish current postal acceptance. Dimensions follow
  preview field settings, not mailpiece qualification rules.

Except for Code 49's built-in row quiet zones, quiet zones are caller-owned.
Interpretation text uses captured resident A at the module magnification unless an explicit ^A command selects another supported font. Printer-specific retail typography remains incomplete. Standalone
UPC/EAN guard-bar extensions use five modules in the specification profile; the ZD621 profile uses
13 dots to match the captured 203-DPI printer controls. See
[printer accuracy](printer-accuracy.md). Retail generated checks and MSI's check-display flag are handled;
other interpretation lines generally display the submitted field data. Binary
payloads should disable interpretation text. Rotation/origins use the common
scene path machinery. There are no placeholder rectangles for unsupported codes.

## Independent decoder tests

For example, `^BRN,11,3,1,60^FD0103212345678906|10ABC^FS` encodes a GS1-128
GTIN linked to a CC-A lot field. Change type 11 to 12 to select CC-C.

Run `cargo test -p zpl`. [`barcode_decode.rs`](../zpl/tests/barcode_decode.rs)
renders ZPL through the real scene/raster path. `rxing` detects common linear and
2D symbols in grayscale pixels; `anyd` decodes uncommon linear/stacked/postal
codes from module samples of the raster. Module-level tests additionally cover
Code 49 row counts, CODABLOCK controls, MaxiCode modes, and the published USPS
Intelligent Mail vector. All MicroPDF417 sizes, all QR masks/error levels, large
matrix payloads, rectangular Data Matrix sizes, and PDF417 ECC levels are covered.
DataBar tests cover Limited width-group boundaries and Expanded padding/stacking.
Expanded has 748 numeric length/row-width cases using anyd's module decoder;
selected layouts additionally use rxing image detection. Two raster fixtures are
checked with anyd instead, including an internal FNC1. One exceeds rxing's legacy
30-character AI 91 validation limit; the other hits its missing Numeric-state
reset following an Alphanumeric FNC1.
TLC39 tests sample both rendered components, decode Code 39, check the T flag and
918 linkage marker, and use rxing's PDF417 pattern, Reed–Solomon, and payload
decoders. This is a component-level adapter, not a full TLC39 scanner test: anyd's
generic MicroPDF417 decoder rejects the TLC-specific 918 marker. MaxiCode tests
also assert nominal hexagon and finder dimensions at three printer resolutions.
`composite_tests.rs` covers every four-column CC-A/CC-B size, CC-C capacities,
GS1-128 B/C linkage, corrupted-codeword correction, and raster-level separation
and alignment at multiple module sizes. The test adapters use rxing for PDF417
pattern recognition, Reed–Solomon correction, byte compaction, general-purpose
GS1 bit decoding, and Code 128. A small inverse base-928 adapter handles CC-A.
anyd additionally checks CC-B's MicroPDF417 framing before its expected rejection
of the composite-specific 920 marker. These are component-level decoder tests,
not an end-to-end composite scanner certification. Raw bitstream tests bypass
rxing's legacy AI-length validator, not its compaction decoder. The adapter
restarts that decoder in Numeric mode after each FNC1, as required by Tables 6/7,
because rxing 0.9.2 leaves the preceding Alphanumeric/ISO-646 mode active.
These tests do not establish support for variants explicitly listed as missing.

Decoder libraries are used only as independent test oracles:
[rxing](https://docs.rs/rxing/0.9.2/rxing/) and
[anyd](https://docs.rs/anyd/0.1.4/anyd/). Production builds can be audited with
`cargo tree -p zpl --edges normal`.

## Real-printer preview comparisons

The [cross-corpus barcode audit](barcode-accuracy.md) inventories saved previews
with model-specific profiles and separates positive coverage from diagnostic
controls. Run `cargo test --locked -p zpl --test barcode_accuracy`.

The [ZD621 fixture corpus](../zebra-http-api/tests/fixtures/barcodes-zd621-v1/README.md)
contains 60 real HTTP previews covering all 29 barcode commands. Run
`cargo test -p zebra-http-api --test barcode_preview` for offline regression
comparisons. The `barcode-compare` example reports each difference and optionally
writes local/diff PNGs; live preview capture is explicit and never prints labels.

These tests preserve known differences rather than asserting that our renderer
already matches the printer. An ignored strict parity test requires nonblank,
pixel-identical output for every case. See the fixture README for commands,
capture provenance and the initial mismatch inventory.

PDF417 also has [30 focused printer probes](../zebra-http-api/tests/fixtures/pdf417-zd621-v1/README.md):
29 require exact full-image equality and one requires explicit rejection of a
too-small symbol whose printer preview is blank. These use `PW832`, so no
preview-padding adjustment is necessary. Run
`cargo test -p zebra-http-api --test pdf417_preview`. Independent decoder tests
cover mixed-mode transitions, every byte value and numeric groups across the
44-digit boundary. The original two PDF417 barcode-corpus images are unchanged;
their local/diff baselines were reviewed and updated after this fix. They now
match the symbol content, but still differ by the preview's 10-dot centering
offset and 832-vs-812 canvas width.

### Retail input normalization

EAN-8, EAN-13 and UPC-A field data is padded on the left with zeros or truncated
to 7, 12 and 11 data digits, as specified on Zebra guide pages 83, 109 and 142.
The ZD621 profile additionally selects measured handling of supplied check
digits, nondigit bytes and overlong EAN fields through separate compatibility
options. CVY checks original input before those normalizations. See the
[raw retail-data controls](../zpl/tests/fixtures/retail-data-zd621-v1/README.md)
for exact full-frame comparisons and the observed validation distinctions.
