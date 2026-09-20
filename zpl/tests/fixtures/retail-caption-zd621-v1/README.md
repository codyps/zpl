# ZD621 UPC/EAN captions and baseline anchors

Raw HTTP Preview Label captures from Zebra ZD621, 203 DPI, V93.21.33Z,
2026-09-19. Every barcode input explicitly requests PW832; references are not
cropped, aligned, rescaled, or otherwise edited. `manifest.tsv` pins source
and response SHA-256, zero underpaint/overpaint, and the complete local raster
hash. The test uses `profiles::ZD621_203_DPI` and requires every pixel to match,
including text. It does not dilute caption errors with white background or bars.

The 74 barcode frames cover EAN-8, UPC-E, EAN-13 and UPC-A, module widths
1–7/9, all orientations, independent payloads, ignored explicit font 0,
optional UPC check-digit captions, and ^FT origins with above/below/hidden
captions. Widths 1/2 use font A; wider captions use native OCR-B at integer
multiples floor(module/3). Digit groups sit around the guards, with the
outer number-system/check digits outside the bars. The vertical gap remains
four dots. Outer digits do not change the bar-width rotation pivot.

`retail_interpretation_printer_layout` selects this measured 203-DPI below-bar
layout. The separate `retail_ft_uses_last_bar_row` selects the inclusive
normal/bottom-up ^FT anchor independently of caption visibility. Both are
enabled in ZD621_203_DPI and disabled in SPECIFICATION. Other resolutions
and the above-bar font selection are outside this caption option's scope.

`font-source/` preserves the raw independent OCR-B digit sampling pages,
a separate composed-text verification, and extractor metadata. The renderer
asset is `zpl/assets/fontE-28-15-digits.zbf`; it contains only decimal digits,
not a complete general-purpose resident E font. Reproduce extraction with:

```sh
cargo run -p zebra-http-api --example extract-font -- \
  --host http://printer.local/ --font E --height 28 --width 15 \
  --characters 0123456789 --batch-size 5 --delay 5 \
  --verify-text 90785634120123456789 /tmp/font-e-capture
```

The independent verification had zero differing pixels. The glyph metrics
use a 20-dot advance and baseline row 22 (the guide's one-based baseline 23).
For offline reproduction, copy `font-source` to a temporary directory and
use that output path with `--offline` and the same other arguments.

Sources: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^B8/^B9 pp. 83–86, ^BE pp. 104–105, ^BU pp. 142–143 (font A/OCR-B
selection), ^FT p. 205 Table 7, resident-font Table 29 p. 1582; retail
structure follows ISO/IEC 15420:2009. Exact dot placement comes from the
preserved printer controls rather than an inferred outline font.
