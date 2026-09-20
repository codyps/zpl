# ZD621 text-field NUL processing

Three unmodified ZD621 203-DPI V93.21.33Z frames pin exact paint counts.
The new control is pixel-exact; each original hex case has one underpaint and
one overpaint pixel in a resident glyph, with an enforced 80% foreground-IoU floor. `hex-underscore` and `hex-hash` are original complete comparison
captures, verified against that corpus's source and PNG hashes. The independent
`nul` control places AB, decoded NUL, and CD in twelve fields: CI0/27/28 with
plain Font 0, field blocks, bounded text, and bitmap font A. Plain and bounded text render AB and discard the remainder. Field blocks
remove NUL and render ABCD. An additional field starts with NUL and is blank.

The ZD621 profile's `text_nul_processing` option matches this behavior
following field-hex decoding. SPECIFICATION disables the option; it does not
invent a visible NUL glyph. Barcode bytes and subsequent fields are unaffected
by the text-only option. Other control bytes have distinct native behavior and
are not silently stripped by this change.

Native JSON records a successful single submission and input/output hashes.
No reference pixels were edited or aligned. The renderer tests also pin the
profile option independently and verify that later fields remain intact.

References: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
`^FD` p. 190 and `^FH` pp. 193–194. These commands define field data and hex
insertion but do not specify this C-string termination behavior, so it is an
explicit printer compatibility option rather than specification behavior.
