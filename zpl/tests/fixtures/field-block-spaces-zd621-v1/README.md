# Field-block spaces

Raw HTTP Preview Label responses from ZD621 203 DPI, firmware V93.21.33Z.
The 48 new frames were captured 2026-09-20 with serialized requests and
five-second pacing; every request completed. The original
`block-content-spaces` source and PNG were copied unchanged from the comparison
repository's complete 2026-09-19 conformance capture, after checking both hashes
against its manifest. All canvases use PW832. No image is rescaled, registered,
or padded. Source, native PNG and rendered-pixel hashes and exact underpaint /
overpaint counts are pinned in manifest.tsv.

Of 49 complete frames, 48 match every pixel. The rotated font-0 frame retains
2 underpaint and 24 overpaint pixels, with the residual and rendered hash
pinned. Its 40 separate ink bands all exceed 98.6% foreground IoU. The region
test requires at least 80% in every band and verifies that their union covers
all ink, so blank background or unrelated text cannot hide an error. The suite covers font A and font 0,
all four alignments and rotations, leading, internal and trailing spaces,
wrapping, paragraph breaks, and terminal-line justification. The 55 trailing
space controls vary block width and retain zero to four trailing spaces.
Leading space glyphs are discarded; internal runs retain extra separators.
A delimiter that wraps discards trailing spaces on the old line. In the
printer's near-full final-line justification rule, retained trailing spaces
extend the eligibility threshold from one space to two.

Narrow-width controls vary the preceding and following word lengths, space
counts, indentation, and three-line overprinting versus twenty-line blocks.
An extra separator run wider than the next available line consumes one blank
row when the preceding line has at most one separator of free space. This
also applies after a forced character has already completed that line. The
next line's hanging indent affects the run-width threshold. An oversized
leading run consumes one blank row as well; its threshold includes every
leading separator. Long runs do not consume multiple blank rows in these
controls. Widths at or below a single separator retain the separately tested
forced-character behavior in field-block-narrow-zd621-v1.

`block_preserves_extra_spaces` selects the measured space and blank-row
behavior in ZD621_203_DPI; SPECIFICATION keeps normalized whitespace. The
independent override is tested. The 19 narrow-block differences recorded by
the initial version of this suite are now all zero underpaint and overpaint;
none are retained as diagnostic allowances.

Source: [Zebra ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FB pp. 186–187. It specifies wrapping and justification but does not prescribe
repeated-space normalization or this terminal-line eligibility threshold.
