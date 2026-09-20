# Font 0 defaults and overrides

31 unmodified ZD621 203-DPI V93.21.33Z HTTP previews. The original
`field-defaults` case is from the complete 2026-09-19 conformance capture;
both input and PNG hashes were verified before copying. The 26 sampling /
verification pages and four FO/FT rotation holdouts were captured 2026-09-20,
serialized with five-second pacing. Every request completed. Preview widths
are multiples of 64; no images were scaled, cropped, padded or registered.

The renderer previously scaled the 32-dot strike for the default 24x16 text
and its 48x32 field override. Native captures at these two sizes make the
original case pixel-exact (previously 76.5% foreground IoU). All 26 sampling
and independent verification frames are exact as well. The four rotation
holdouts retain 18 underpaint and 18 overpaint pixels in total. Each of their
48 separate text fields exceeds 98.5% IoU. Tests enforce an 80% floor per field
and pin exact residual counts and source / native / rendered-pixel hashes.
The fixed field regions partition each complete canvas.

Both font-source directories contain 95 printable ASCII glyphs, capture
settings, metrics, twelve sampling pages and a separate verification string.
The extraction test regenerates each embedded asset byte-for-byte and checks
independent composition. These strikes cover the captured sizes and ASCII
characters, not arbitrary font-0 sizes or unmeasured Unicode glyphs.

References: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^A pp. 60–61 (field-local override and restoration of ^CF), ^CF p. 154,
^FO p. 201 and ^FT p. 205. Tests are offline and use ZD621_203_DPI explicitly.
