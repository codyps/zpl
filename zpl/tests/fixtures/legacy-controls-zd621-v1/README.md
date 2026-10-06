# Encoding-specific ESC and DEL rendering

Eight unmodified ZD621 203-DPI V93.21.33Z previews contain 176 source fields,
including isolated glyphs and advance probes. Every full frame is pixel-exact.
Two isolated Font 0 ESC fields intentionally contain no ink. Each capture
completed successfully on its first HTTP submission, with `Connection: close`
and at least five seconds between requests. Input, PNG and output raster hashes
are pinned alongside zero underpaint/overpaint counts.

- `original` is the earlier 28-field NUL/SOH/TAB/LF/CR/ESC/DEL diagnostic. It
  previously failed to render; it now matches all printer pixels.
- `esc` and `del` each contain 24 controls across CI0/13/27/28, fonts 0/A, and
  plain/FB/TB layouts.
- `glyphs` and `glyphs-pa` isolate the two legacy characters, measure their
  advances with pipe sentinels, and independently verify CI13 mixed text.
  They cover Font 0 at 16/32 dots and bitmap A at native and doubled sizes,
  with the PA default-glyph setting off/on.
- `origins` checks both fonts, FO/FT, all rotations, CI0/13 and PA0/1.
- `modern` and `modern-a` each check eighteen leading/interior/trailing control
  cases under CI28/31/33/34/35/36 in plain/FB/TB layouts.

The native rules are:

| Encoding | Plain and TB ESC | Plain and TB DEL | FB ESC/DEL |
| --- | --- | --- | --- |
| CI0/13 | Native legacy glyph | Native house glyph | Native legacy glyphs |
| CI27/28/31 | Omit | Omit | Space-width characters inside words |
| CI33–36 | Space-width character | Omit | Space-width characters inside words |

Font 0 lacks the legacy ESC glyph: PA0 leaves blank spacing, while PA1 uses
its default glyph and different advance. Bitmap A has an ESC graphic, so PA
does not change it. DEL is available as a house glyph in both sampled fonts.
Leading FB controls preserve their spacing instead of becoming separators
that the word wrapper would discard.

Five packed assets are independently reconstructed by zpl-font-extract's
`legacy_controls` test. Glyph rows come from the isolated raw samples;
advances come from the translated final pipe, and vertical metrics use the
native baselines. The doubled-A and CI13 composed fields verify them through
the renderer. The arrow/house scalar keys inside the packed files identify
legacy-only entries; they do not enable Unicode glyphs in normal CI28 faces.
The control faces retain the independently selected backslash and PA behavior.

`text_esc_del_processing` selects these rules in ZD621_203_DPI. SPECIFICATION
disables them. Independent tests verify selection and unchanged barcode bytes.
The additional legacy glyph assets cover the sampled fonts/sizes; this does
not claim complete Unicode or every unsampled resident-glyph combination.

References: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^CI pp. 156–159, ^FD p. 190, ^FH pp. 193–194, ^FB pp. 186–187,
^FO/^FT pp. 201/205, ^PA p. 315, ^TB pp. 356–357 and font matrices/baselines
pp. 1582–1584. The encoding/layout-specific control behavior is empirical and
remains a selectable compatibility departure.

Font extraction commands and extraction-only tests in this document now run
from the separate [private font research repository](https://github.com/codyps/zpl-font-extract).
The runtime assertions and captured bytes remain in ZPL.
