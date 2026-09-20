# Comparing the resident Font 0 TrueType file

The local `zebra-firmware/artifacts/printer-fonts-1/Z-0.TTF` is CG Triumvirate
Condensed Bold v4.01, 125,904 bytes, 571 glyphs, 566 Unicode cmap entries, and
2048 units per em. Its SHA-256 is
`4ab8a67dbf8eb9b5d006636097318069c4674e6e235228b1985a488cd077049f`.
It contains TrueType hinting programs (`fpgm`, `prep`, `cvt `). The font is an
external input to the comparison script.

Run `scripts/compare-font0-ttf.py FONT STRIKE...` using Python with
`freetype-py` installed. The script makes no printer requests. It reports
source hashes, the FreeType version, per-glyph underpaint/overpaint, advances,
foreground IoU, and glyphs below 80%. Coordinates stay relative to the native
FT baseline, with no alignment optimization. Blank/blank has no IoU score.

With FreeType 2.13.2, pixel sizes equal to the requested ZPL width/height,
`FT_LOAD_RENDER | FT_LOAD_TARGET_MONO`, and the direct Unicode cmap:

| Captured strike | Foreground IoU | Exact pixels and advance |
| --- | ---: | ---: |
| Font 0 40x24 ASCII | 97.1540% | 38/95 glyphs |
| Font 0 16x10 ASCII | 92.1348% | 64/95 glyphs |
| Font 0 40x24 Unicode supplement | 80.8162% | 17/52 glyphs |

These aggregate scores do not meet a per-glyph 80% requirement. The 40x24
ASCII exceptions are backslash, caret, lowercase w, and vertical bar. Some
are legacy mappings already handled by the renderer. The Unicode supplement
includes characters absent from this TTF: its missing-glyph box differs from
the printer's blank fallback. Raster bounding boxes can contain empty border
rows, so compare ink coordinates, not only bitmap dimensions.

This is a calibration tool, not a replacement for the exact captured strikes.
A future TrueType path must preserve printer character mapping and missing
character advances and verify hinting across sizes, orientations, and field
layouts against independent printer samples.

Sources: the local font's name/head/hhea/cmap/maxp tables and the committed
native bitmap strikes; [FreeType glyph retrieval and render flags](https://freetype.org/freetype2/docs/reference/ft2-glyph_retrieval.html).
Native capture provenance is recorded in each strike's fixture README.
