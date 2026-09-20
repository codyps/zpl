# Initial serialization-mask controls

14 complete unmodified HTTP previews from a ZD621, 203 DPI, V93.21.33Z,
captured 2026-09-20 with five-second pacing. Every source uses PW832;
no reference is padded, cropped, resized or registered. All requests completed.
The original serial-mask comparison case was recaptured without modification.

The five primary frames cover that original, decimal/hex/octal/alphabetic/
alphanumeric/skip masks, mixed-case inputs, Code 128 and EAN-13, FH decoding,
and a reset control. Nine companion frames compare ordinary FD with FD+SF
under CI0/13/27/28 and repeat the reset control. Every same-encoding FD/SF
pair and both resets are pixel-identical, establishing that SF leaves initial
field data unchanged. Mask/increment metadata is validated; subsequent-label
iteration using PQ remains an explicit unsupported-command error.

Ten frames match every pixel. Four text frames retain 168 overpaint pixels
and zero underpaint: the captured CI13 font-A zero is unslashed, whereas the
embedded strike has a slash. This is independently present without SF and
is an existing character-encoding/font gap, not a mask transformation.
All 18 separate text regions meet the 80% foreground-IoU requirement; the
minimum is 88.6792453%. Source, native PNG, local raster hashes and exact
paint counts are pinned. The barcode frame has a separate zero-difference
requirement, so a future baseline cannot relax its non-text accuracy.

The printer controls additionally tested PQ1 and PQ3 outside this fixture
set: their native previews were identical. Those captures are evidence for
future quantity/iteration work and are not counted as supported renderer cases.

Reference: Zebra ZPL II Programming Guide, SF pp. 335–337 and PQ p. 324:
https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
Run cargo test -p zpl --test serial_mask --test serial_mask_preview.
