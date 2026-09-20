# Field-block spaces

Raw HTTP Preview Label responses from ZD621 203 DPI, firmware V93.21.33Z.
The 35 new frames were captured 2026-09-20 with serialized requests and
five-second pacing; every request completed. The original
`block-content-spaces` source and PNG were copied unchanged from the comparison
repository's complete 2026-09-19 conformance capture, after checking both hashes
against its manifest. All canvases use PW832. No image is rescaled, registered,
or padded. Source, native PNG and rendered-pixel hashes and exact underpaint /
overpaint counts are pinned in manifest.tsv.

The 16 `exact` frames cover font A and font 0, all four alignments, leading,
internal and trailing spaces, wrapping, paragraph breaks, and terminal-line
justification. The 55 trailing-space controls vary block width and retain zero
to four trailing spaces. Leading spaces are discarded; internal runs retain
extra separators. A delimiter that wraps discards trailing spaces on the old
line. In the printer's near-full final-line justification rule, retained
trailing spaces extend the eligibility threshold from one space to two.
`block_preserves_extra_spaces` selects this behavior in ZD621_203_DPI;
SPECIFICATION keeps normalized whitespace. The independent override is tested.

The 20 `diagnostic` frames additionally exercise widths 1–40, up to 12 repeated
spaces, and both three-line overprinting and twenty-line blocks. These retain
known differences: the printer sometimes consumes an additional empty line
between words in narrow blocks. Their exact current underpaint / overpaint and
render hashes guard against regressions; they are **not** evidence that those
text regions meet the 80% goal. The ordinary-space fix does not resolve this
blank-line rule. Keeping these raw controls makes that remaining work explicit.

Source: [Zebra ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FB pp. 186–187. It specifies wrapping and justification but does not prescribe
repeated-space normalization or this terminal-line eligibility threshold.
