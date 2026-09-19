# ZD621 Code 39 wide-element quantization

16 raw HTTP Preview Label responses from ZD621 203 DPI, firmware V93.21.33Z,
collected 2026-09-19 using `zebra-render`. Inputs use PW832 and reset label
state. No reference image is cropped, aligned, or rescaled.

The twelve `barcode-module-*` inputs are unchanged copies from the sibling
`zpl-comparison/test-data/render-conformance/cases/barcode-arguments` suite.
They cover module widths 1, 2, 3 and 10 with ratios 2.0, 2.5 and 3.0. Four
additional atlases cover all eleven ratio tenths from 2.0 through 3.0 for
module widths 1, 3, 7 and 9, giving 56 symbols altogether.

Every wide bar and space uses floor(module width × selected ratio). Fractional
widths must not accumulate across elements. Before the fix, the two original
ratio-2.5 cases at module widths 1 and 3 had respectively 1120/1520 and
3600/4000 underpaint/overpaint dots. All sixteen frames now require zero
underpaint and overpaint, plus pinned source, response and local-pixel hashes.

The Zebra Programming Guide ^BY, p. 148, Table 6 describes quantized ratios,
but its worked example explicitly rounds 9 × 2.4 = 21.6 to 22 dots. The
captured printer instead uses 21. `code39_floor_wide_elements` selects this
measured departure for standalone ^B3. It is enabled in `ZD621_203_DPI` and
disabled in `SPECIFICATION`, which follows the example's nearest-dot rule.
Other Code 39-derived symbologies are outside this option's tested scope.

Reference: [Zebra ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^B3 pp. 70–72 and ^BY p. 148. The original input provenance and reference
hashes are preserved by the manifest; no comparison-repository files were edited.
