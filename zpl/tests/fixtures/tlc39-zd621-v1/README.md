# ZD621 TLC39 controls

119 raw HTTP Preview Label captures from Zebra ZD621, 203 DPI,
firmware V93.21.33Z, collected 2026-09-19 using `zebra-render`.
Inputs reset label state, use PW832 and place one TLC39 field at FO60,60.
No printer image is cropped, resized or repaired.

Controls vary alphabetic, numeric and mixed compaction, additional-field
separators, Code 39 module/ratio/height and MicroPDF417 module/row height.
Two 17-step height sweeps verify the extended link flag's integer geometry.
All 119 controls are pixel exact. Seven numeric-length controls verify Numeric
compaction even for one digit. Eighteen field-count/length controls and 44
additional boundary controls vary field lengths, uppercase/lowercase letters,
and numeric runs independently.

For additional fields the printer reserves Byte-compaction capacity (five words
per six bytes, one per leftover byte, plus its mode latch), excluding the TLC
linkage word from that estimate. It still emits the actual Text/Numeric words.
The `tlc39_additional_data_byte_capacity` option selects this conservative sizing;
the specification profile fits the actual codewords. The 13/14-byte boundary
controls distinguish whether the estimate includes the linkage word.
Mixed-data numeric runs switch at fourteen digits; isolated numeric data uses
Numeric mode at every tested length. The 12/13/14-digit controls distinguish
this threshold from standalone MicroPDF417 and full PDF417.

References: Zebra Programming Guide ^BT, pp. 140–141, and
[US20010045461A1](https://patents.google.com/patent/US20010045461A1/en),
paragraphs 0024–0029 and 0067/Figure 2. The printer uses a literal asterisk
between supplementary fields, a minimum six-row symbol, and an extended
link flag. These choices have independent compatibility options, enabled
by ZD621_203_DPI and disabled by SPECIFICATION.

`manifest.tsv` pins source/capture hashes, separate underpaint and overpaint
counts, and the complete local pixel hash. The test uses ZD621_203_DPI.
Every control also requires exact full-canvas equality.
