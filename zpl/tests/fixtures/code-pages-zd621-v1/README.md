# ZD621 single-byte code pages

Seven raw ZD621 203-DPI previews (V93.21.33Z) pin zero underpaint and zero
overpaint. The six `encoding-*` cases are unchanged complete captures from the
comparison corpus, with source/PNG hashes checked against its manifest. They
exercise ASCII under CI27, CI31, CI33, CI34, CI35, and CI36.

`non-ascii` is a new 832x640 native control with six rows and paired columns.
Each left field uses the selected single-byte code page; the right field uses
the same characters encoded as UTF-8. All use Font 0 at 40x24 and PA0,0,0,0.
Rows cover Windows-1252 (é, pound, euro), 1250 (é, euro), 1251 (Cyrillic),
1253 (Greek), 1254 (é, ö, pound, euro), and 1255 (Hebrew). Both columns and
all original ASCII frames are pixel-exact in the renderer. The JSON records
the successful single submission and input/output hashes.

The renderer now decodes these code pages before glyph lookup. Character
availability still depends on the captured resident-font repertoire. Barcode
bytes bypass text decoding, and changing CI between fields selects the new
mapping. These are documented encodings, so both SPECIFICATION and
ZD621_203_DPI support them without a compatibility departure.

Reference: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
`^CI`, pp. 156–159, and code-page tables beginning p. 1568. The guide restricts
CI31 support by model/font; the native control here confirms scalable Font 0
on this ZD621. This does not establish code-page behavior for every resident
bitmap font or undefined byte, or implement CI source/destination remapping.
