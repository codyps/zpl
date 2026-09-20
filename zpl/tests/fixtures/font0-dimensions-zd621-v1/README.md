# Font-0 dimension coverage

Nine native ASCII strikes cover the remaining below-80% dimension cases:
15x15, 17x17, 31x31, 33x33, 63x63, 65x65, 64x16, 16x64 and 96x96.
The square sizes were requested with width zero. All 855 glyphs have raw
sampling pages and independently verified composition; offline tests reproduce
the embedded assets byte-for-byte.

127 complete ZD621 203-DPI V93.21.33Z HTTP preview frames are pinned by source,
PNG and rendered-pixel hashes and exact underpaint/overpaint counts. Nine
original conformance inputs and PNGs were copied unchanged from the complete
2026-09-19 capture after verifying its hashes. The 117 sampling/verification
pages and one 72-field origin/rotation atlas were captured 2026-09-20 with
serialized requests and five-second pacing. Every included capture completed.

All 126 sampling, verification and original conformance frames match exactly.
Each of the 72 separate FO/FT and N/R/I/B atlas fields exceeds 97.1% foreground
IoU, with an 80% regression floor. The field rectangles partition the complete
canvas; exact residuals remain pinned independently of that floor.

The first 65-dot sampling attempt requested PW896 but returned 832 pixels.
It was rejected and is not included or scored. New single-column captures at
65 and 96 dots fit the device's width; their capture configurations record
`columns: 1`. The 96-dot verification uses the shorter `Hg0j` string to fit
one line. Other sizes retain the original two-column layout and verification
string. No renderer/reference image was resized, padded or registered.
These ASCII strikes do not establish arbitrary-size or Unicode accuracy.

References: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^A pp. 60–61, ^CF p. 154, ^FO p. 201 and ^FT p. 205. Tests use ZD621_203_DPI.
