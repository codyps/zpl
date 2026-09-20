# Page mirroring in ZD621 previews

Thirteen unmodified ZD621 203 DPI HTTP preview responses, firmware V93.21.33Z.
Nine controls were captured 2026-09-20 with serialized requests and five-second
pacing. The final PMN/PON control restored those settings and matched the initial
normal control exactly. Four original page-transform controls come from the
completed zpl-comparison conformance capture; its corpus/input/PNG hashes and
identical repeated control were verified before using these references.
All inputs use PW832, avoiding preview-width adjustment.

[Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^PM p. 319 defines horizontal mirroring of the entire printable area. The setting
persists until PMN or power-off; missing and invalid parameters are ignored.
The specification profile implements that behavior, including settings after
fields, persistence across labels and composition with PO inversion.

The ZD621 HTTP preview ignores PM. `preview_ignores_print_mirror` selects that
captured departure independently from `preview_ignores_print_orientation`.
Both are enabled in the printer profile and disabled in SPECIFICATION. This
observation concerns the HTTP preview, not a scanned physical print.

The fresh controls contain asymmetric boxes, white compositing, an ellipse,
a bitmap, Code 128 bars and native font-A text. They cover PM/PO combinations,
late enable/disable, and missing/invalid PM operands. All thirteen frames are
pixel-exact, with zero underpaint and zero overpaint. Source, raw printer PNG
and rendered pixel hashes are pinned; no registration or padding is applied.
