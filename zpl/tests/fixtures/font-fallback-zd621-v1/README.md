# Unavailable resident font controls

33 unmodified ZD621 203 DPI V93.21.33Z previews captured 2026-09-19/20,
with serialized requests and five-second pacing. Every request completed.
PW832 avoids preview width rounding; no images were resized or registered.
Source and native PNG hashes and exact underpaint/overpaint counts are pinned.
32 frames are pixel exact; rotated font 0 has two missing and two extra dots.

Unavailable field fonts inherit CF, independently of a preceding field's font.
Unavailable CF selects A. Controls cover CF 0/A/B, explicit/omitted/zero
sizes, rotations, font-only CF, repeatability and reset. P–V are separate
preset fonts and must not use this fallback. The profile option is enabled
only in ZD621_203_DPI; SPECIFICATION reports unsupported font assets.

Reference: Zebra ZPL II Programming Guide, ^A pp. 60–61 and ^CF p. 154:
https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
The guide lists legal font identifiers and invalid CF fallback, but does not
specify how unavailable field font identifiers resolve on this printer.

The 20 original font-id cases are copied unchanged from zpl-comparison
conformance-reference, after checking its complete capture manifest hashes
(captured_utc 2026-09-19T03:55:35Z). The other 13 frames are fresh controls.
