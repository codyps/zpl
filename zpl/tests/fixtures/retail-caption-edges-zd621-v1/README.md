# ZD621 retail captions at the label edge

Five unmodified native ZD621 203-DPI V93.21.33Z frames pin zero underpaint and
zero overpaint across 32 barcode fields. `discovery` preserves the original
UPC caption failure found during character-remapping work: an off-label OCR-B
number-system digit was clipped by the renderer but moved inward by the printer.
It previously had 82 underpaint and 63 overpaint pixels and is now exact.

`small` and `large` cover UPC-A module widths 1–6 at horizontal origins 0, 5,
and 20, including resident A and two OCR-B scales. `rotations` places captions
at the edge in all four orientations. `holdouts` adds FT baseline placement,
EAN-13, UPC-E, label shift, reverse printing, and an EAN-8 interior control.
The original discovery frame also includes character-image remapping.

The printer clamps caption groups along their reading axis. Normal and rotated
fields use the group's nominal origin; inverted and bottom-up fields use the
visible edge. The perpendicular coordinate and barcode-bar placement remain
unchanged. After moving groups, normal black ink is unioned before rotation so
caption/guard overlap stays black. Reverse printing retains component parity.

`retail_caption_clamps_negative_inline_origin` selects this departure from
nominal clipped placement. It refines `retail_interpretation_printer_layout`,
is disabled in SPECIFICATION, and enabled in ZD621_203_DPI. The independent
option test checks that bars do not move and interior captions do not change.
Existing retail-caption and barcode-edge suites remain regression controls.

References: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
`^FO` p. 201, `^FT` p. 205, and the UPC/EAN interpretation-line commands. The
edge rule is measured printer behavior. Per-case JSON and the manifest pin
successful submissions, source bytes, native PNGs, and renderer pixel hashes.
No reference image was resized, shifted, or edited.
