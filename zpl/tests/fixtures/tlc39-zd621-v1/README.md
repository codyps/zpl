# ZD621 TLC39 controls

50 raw HTTP Preview Label captures from Zebra ZD621, 203 DPI,
firmware V93.21.33Z, collected 2026-09-19 using `zebra-render`.
Inputs reset label state, use PW832 and place one TLC39 field at FO60,60.
No printer image is cropped, resized or repaired.

Controls vary alphabetic, numeric and mixed compaction, additional-field
separators, Code 39 module/ratio/height and MicroPDF417 module/row height.
Two 17-step height sweeps verify the extended link flag's integer geometry.
48 controls are pixel exact. The two `long-*` controls still differ: the
printer selects 26 MicroPDF417 rows for three 25-character fields, while
our encoder selects the smallest fitting 20-row symbol.

References: Zebra Programming Guide ^BT, pp. 140–141, and
[US20010045461A1](https://patents.google.com/patent/US20010045461A1/en),
paragraphs 0024–0029 and 0067/Figure 2. The printer uses a literal asterisk
between supplementary fields, a minimum six-row symbol, and an extended
link flag. These choices have independent compatibility options, enabled
by ZD621_203_DPI and disabled by SPECIFICATION.

`manifest.tsv` pins source/capture hashes, separate underpaint and overpaint
counts, and the complete local pixel hash. The test uses ZD621_203_DPI.
The two known differences are fixed baselines, not allowances that may grow.
