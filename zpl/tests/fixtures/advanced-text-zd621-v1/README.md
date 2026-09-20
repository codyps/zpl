# Advanced text controls

49 unmodified HTTP previews from the ZD621, 203 DPI, V93.21.33Z, captured
2026-09-20. Requests were serialized with five-second pacing; all completed.
Widths are 832 dots or another multiple of 64. Images are not cropped,
registered, padded or resized. Font-page repeats, matrix/bracket/feature
repeats, FB and TB repeats and reset controls matched their originals.

The set contains all 16 PA flag combinations, fresh unchanged copies of the
six original advanced-text cases from zpl-comparison, independent Hebrew/
Latin/digit/punctuation strings, embeddings/overrides/isolates, explicit and
automatic TB justification, FB/TB wrapping, PA operand/field/label persistence,
and four rotations at font 0 sizes 32x0 and 40x24. Three sampled fonts each
have a separate verification string and repeated first page. assets.tsv pins
the two PA1 default-glyph strikes and PA0's 32-dot missing-character advances.
They cover U+0378 and the five Arabic characters in the original cases; ASCII
A is a verification anchor. This does not extend the resident font repertoire
to arbitrary Arabic or other uncaptured characters.

45 frames are pixel-exact, including all six original cases and every
independent 160-dot TB holdout. Two rotated frames and two 100/120/180-dot TB
frames retain a total of 870 underpaint and 862 overpaint pixels. All 58
separate rotated/wrapped text regions meet the 80% foreground-IoU requirement;
the minimum is 93.2298137%. Counts and raw rendered pixel hashes are pinned,
including acceptable residuals, so the threshold cannot hide a regression.

state-next-label.zpl was submitted immediately after state-state.zpl. The
fixture test renders their concatenation and checks label 1, preserving the
actual PA state without modifying either raw source. Its expected pixel hash
is the native image's pixel hash; the test independently establishes exactness.
All other expected render hashes were generated from local renderer outputs.

Printer departures are separate compatibility options: omitted PA operands
retain previous values, paired-bracket resolution (N0) is omitted, and Unicode
isolates behave as class-L missing characters. SPECIFICATION disables these;
ZD621_203_DPI enables them. FB ignores bidi as documented, while TB wraps the
logical text, resolves paragraph direction and then reorders each visual line.
Shaping/OpenType toggles do not change the captured embedded glyph repertoire;
unmeasured glyphs still produce explicit errors.

References: Zebra ZPL II Programming Guide, PA p. 315, FB p. 188, TB p. 356,
FW p. 208, CI pp. 156–159:
https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
Unicode Bidirectional Algorithm (paragraph resolution, L1/L2 and mirroring):
https://www.unicode.org/reports/tr9/

Run cargo test -p zpl --test advanced_text --test advanced_text_preview.
