# Data Matrix ECC200 compaction controls

Captured 2026-09-19 UTC using HTTP Preview Label on
`http://printer.local/`, ZD621 203 dpi, firmware V93.21.33Z.
No physical labels were printed. PNGs retain original response bytes, pinned
with input SHA-256 in `manifest.tsv`. The printer profile must match the whole
canvas exactly without shifting, cropping, padding or rescaling.

The 67 controls cover all six compaction modes, lowercase lengths 3–24,
C40/Text shifts, X12 and EDIFACT tails, Base256 lengths 249/250, fixed and
rectangular dimensions, FNC1 and explicit/default escape operands. Inputs use
FH hexadecimal encoding where necessary; the initial `fnc` probe demonstrates
FH decoding of `_1A`, whereas `fnc-actual` preserves `_1` for BX processing.

The captured default escape is tilde, despite the guide's modern-firmware
underscore note. An explicit g operand overrides the default. Prefix-valued
escape operands must also survive command framing. The EDIFACT controls show
checks before the fourth character of a group and retention of equal-length
tails. Both behaviors are independent printer compatibility options.

References checked: supplied ISO-IEC-16022-2006-2ed Data Matrix.pdf, §§5.2.4–5.2.9
(pp. 7–14), Annex C (tables), Annex P (pp. 116–119), plus
ISO-IEC-16022-2006-tc1-2008 Data Matrix.pdf and
ISO-IEC-16022-2006-tc2-2011 Data Matrix.pdf. TC1 changes Annex M quality grading;
TC2 replaces Clause 9 reference decoding. Neither changes the encodation rules
used here. [ISO publication and corrigenda](https://www.iso.org/standard/44230.html).
Zebra command details: bundled ZPL Programming Guide ^BX pp. 144–147.
The encoder is original code derived from those specifications; decoder
libraries are used only in tests.
