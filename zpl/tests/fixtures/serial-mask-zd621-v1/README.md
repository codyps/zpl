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

All 14 frames match every pixel. The preview-derived CI13 mapping resolves
font A's unslashed zero, removing the previous 168 overpaint pixels across
four text frames. This behavior is independently present without SF and is
also covered by the bitmap-ci13-zd621-v1 controls. Source and printer PNG
hashes remain unchanged; rendered raster hashes and zero paint counts are
pinned. The test requires exact native pixel equality for every frame and
retains foreground-IoU checks for all 18 separate text regions.

The printer controls additionally tested PQ1 and PQ3 outside this fixture
set: their native previews were identical. Those captures are evidence for
future quantity/iteration work and are not counted as supported renderer cases.

Reference: Zebra ZPL II Programming Guide, SF pp. 335–337 and PQ p. 324:
https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
Run cargo test -p zpl --test serial_mask --test serial_mask_preview.
