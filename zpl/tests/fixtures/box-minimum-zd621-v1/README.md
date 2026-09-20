# Graphic box minimum dimensions

Three unmodified ZD621 203 DPI HTTP Preview Label responses, firmware
V93.21.33Z, captured 2026-09-20. Each contains nine boxes: dimensions smaller
than thickness, zero dimensions, omitted dimensions, and an ordinary control.
Rounding values 0, 1 and 8 cover square and rounded corners. PW832 avoids
preview width rounding. Captures were serialized with five-second pacing.

The manifest pins source and response SHA-256, local grayscale pixel SHA-256,
and zero underpaint/overpaint. box_minimum_preview.rs requires exact full-canvas
comparison using ZD621_203_DPI. profiles.rs also checks normalization under
SPECIFICATION: minimum/default dimensions are documented behavior, not a
compatibility override.

[Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^GB p. 210: width and height default to thickness and are adjusted to their
minimum before calculating rounding. The zero-width example is a vertical line.
