# ZD621 initial serial fields

Eight unmodified previews from the 203-dpi ZD621, firmware V93.21.33Z,
captured 2026-09-20 in three serialized batches with five-second pacing.
Every request completed; the repeated final controls were pixel-identical.
PW832 avoids preview width adjustment. PNGs are not cropped, resized or aligned.

The controls cover SN Y/N/defaults, decrement and zero increments, default
starting value, embedded numeric runs, no numeric run, all-zero data, leading
spaces, Code 128 and EAN-13, plus independent values and FH-escaped digits.
The holdout records that overlong (>12-digit) numeric runs retain their original
zeros on this firmware; serial_overlong_keeps_value selects that behavior.

See Zebra ZPL II Programming Guide, SN pp. 341–342:
https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
This tests the initial preview only; PQ iteration is still an explicit error.

manifest.tsv pins source, raw PNG and rendered pixel SHA-256, with zero
underpaint and overpaint across every full canvas, including text.
Run cargo test -p zpl --test serial --test serial_preview.

The CI13 serial fields retain the source-position slashed zero, unlike ordinary
FD/SF text. `serial_ci13_zero_uses_source` selects this measured departure.
On 2026-10-07, two independent previews of the unchanged text-serial request
reproduced the original PNG SHA-256 exactly:
`97d738b631358df5c73ffe988ab96e2a73f9d381b33141fc47418ee78bdde167`.
The specification profile leaves this override disabled.
