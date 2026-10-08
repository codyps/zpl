# Downloaded bitmap printer controls

This fixed regression subset contains 100 original HTTP Preview Label PNGs and
exact submitted ZPL from [zpl-comparison](https://github.com/codyps/zpl-comparison)
commit `ca17c2e4fa3bfdcd9685e51c67d3cf2c8e69bd33`, under
`references/font-controlled/`. Captured October 8, 2026 UTC:

- 85 ZD621 203 dpi, firmware V93.21.33Z controls.
- 15 ZQ610 Plus 203 dpi, firmware V100.21.21Z controls (A/D/E only).

`provenance.json` retains the selected capture records, source/submission/PNG
hashes, device identities, and session repeat/restoration checks. `manifest.tsv`
pins exact submissions and unchanged PNGs. `bundle.json` is the original font
bundle manifest, whose hash matches the capture's `bundle_sha256`.
`bitmap-download.zpl` is the original shared bitmap resource preamble, verified
against that manifest. No private repository or live printer is needed to test.

The test installs those `~DB` resources, then replays the exact saved submissions
with ZD621_203_DPI or ZQ610_PLUS_203_DPI. The submissions also alias font 0, but
all rendered text in this subset explicitly selects bitmap fonts or GS. A
sentinel named face resolves that unused alias and errors on visible text,
avoiding a dependency on the unrelated Heros TrueType font. GS retains its
resident face, exactly as it did in the printer captures.

Coverage:

- Downloaded A–H at height 32 / width 24 in all four orientations; C aliases D.
- Downloaded P–V at height 32 / width 24, normal orientation.
- Original A/D “Hello 123” cases and their separate conformance captures.
- A/B/C/F FO versus FT baseline controls at height 32 / width 20.
- Three native-size A field-block pages exercising justification gap rounding.
- Twenty resident graphic-symbol cases (A–E in all four orientations).
- A/D/E at height 28 / width 16 on both devices: N/R/I/B and inverted-fit controls.

Every case requires exact equality of the native full canvas: zero underpaint
and zero overpaint. Images are never aligned, cropped, padded, or rescaled.

Measured behavior: `~DB` faces round requested dimensions to independent integer
multiples of their header matrix (minimum one); header baselines are one-based;
FO rotations use bitmap pivots; magnified FT uses the last-dot baseline rule.
These behaviors are selected by `supplied_bitmap_font_metrics`; FT also uses
`bitmap_font_ft_dot_origin`. API registrations and resolvers represent already
installed downloads and use the same profile-dependent metrics. SPECIFICATION
keeps its previous custom-font behavior.

References: Zebra ZPL Programming Guide ~DB pp. 169–170, ^A@ pp. 31–32,
^FO p. 201, ^FT p. 205 and resident matrix tables pp. 1582–1584:
<https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf>.

This subset does not establish downloaded P–V rotated parity, rotated downloaded
FT parity, arbitrary download-header semantics, or other device/firmware parity.
Mixed TrueType/bitmap pages are outside this bitmap-only regression corpus.
Growing capture/extraction tooling remains outside this repository.
