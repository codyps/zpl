# ZD621 extended QR Model 1 versions

Raw ZD621 203 DPI HTTP previews, firmware V93.21.33Z, captured 2026-09-20.
Requests completed sequentially with five-second pacing. PW832 avoids preview
width adjustment; LL640 accommodates the largest symbol without clipping.
All symbols use three-dot modules and explicit reset state in their source.
The largest Kanji holdout uses raw Shift JIS bytes to stay within the documented
3,072-byte FD limit. Its byte sequence is also valid UTF-8 for HTTP transport.

ISO/IEC 18004:2000 Annex M pp. 97–112 defines Model 1 through version 14.
The printer also renders versions 15–40 with the same Model 1 placement,
format-information XOR and data-word ordering. The independent
`qr_model1_extended_versions` compatibility option enables these versions.
It is enabled in the ZD621 profile and disabled in SPECIFICATION. The original
standard tables and the behavior of smaller symbols remain unchanged.

The 112 captures contain:

- 104 byte-mode controls: each version 15–40 at each L/M/Q/H correction level.
  Each payload exceeds the preceding version's capacity, so it independently
  exercises the next capacity boundary.
- Six held-out numeric, alphanumeric and Kanji controls at versions 27 and 40,
  checking non-byte data and the version-27 character-count-width transition.
- A repeated version-15-L control with identical printer pixels.
- A version-14-Q boundary control using the standard block layout.

For the byte controls, inverse Model 1 placement recovers the submitted data
prefix exactly. The terminator and alternating pad words locate the data/RS
boundary. Every candidate block layout is checked by recomputing every
Reed–Solomon parity block over GF(256), polynomial 0x11D, first root zero.
Only one layout matches each capture. `parameters.tsv` records the recovered
block counts, data/parity words and total placement words. These encoding
parameters are independent of the payload; the additional modes test that.

The regression test requires exact whole-frame equality for every unchanged
request under automatic mask selection. A separate explicit-mask comparison
disables automatic selection and changes only the mask operand to the
BCH-validated value recovered from the printer. Raw ZPL/PNG hashes and zero
paint-error baselines are pinned in `manifest.tsv`.

The formerly unsupported structured-append version-15 capture remains in
`qr-append-zd621-v1` and also matches exactly. See
[mask selection evidence](../../../../docs/qr-mask-selection.md).
