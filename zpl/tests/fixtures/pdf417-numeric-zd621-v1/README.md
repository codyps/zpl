# ZD621 standalone PDF417 numeric transitions

Thirteen raw HTTP Preview Label frames from ZD621 203 DPI, firmware V93.21.33Z,
collected 2026-09-19 with `zebra-render`. All use PW832 and reset label state.
The references are unmodified; tests compare the entire canvas without cropping,
registration or tolerance. The manifest pins input/response SHA-256, zero
underpaint and overpaint, and the complete local pixel hash.

Seven atlases contain 112 symbols. Each tests digit-run lengths 1–16 with:
no surrounding text; prefixes `A`, `ABCD`, `abcd`, or `The quick brown fox `;
suffix `B`; and both prefix `A` and suffix `B`. Digit data is a prefix of
`0123456789012345`. Origins use three 260-dot columns and 65-dot rows, with
module width 1, row height 3, six data columns and security level 0.

The six `pdf417-security-*` inputs are unchanged copies from the sibling
`zpl-comparison/test-data/render-conformance/cases/barcode-arguments` suite.
They encode `The quick brown fox 0123456789` at security levels 0, 2 and 8,
with and without truncation. Before the fix, each level's full/truncated pair
had respectively 162/204, 312/318 and 11400/11364 underpaint/overpaint dots.
All 118 symbols now match exactly.

Decoded printer codewords establish these standalone selection rules:

- Entirely numeric data selects Numeric compaction at eight digits.
- Mixed data keeps digit runs through thirteen in Text, switching at fourteen.
  This also applies when the digit run starts the input but text follows it.
- Short text prefixes before a Numeric run remain in Text instead of taking
  the generic short-run Byte shift/latch path.

These are choices among valid encodings, not changes to the PDF417 data format.
Macro PDF417 uses its independently captured eight-digit rule; existing ^FM
printer tests retain that behavior. MicroPDF417 and TLC39 have separate rules.

References: [USS PDF417](https://www.expresscorp.com/wp-content/uploads/2023/02/USS-PDF-417.pdf),
§§2.2.4.4–6 and Appendix D; Zebra Programming Guide ^B7, pp. 79–82.
