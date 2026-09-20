# Long field rasterization and font 0 at 16 x 10 dots

The original 3072-byte field conformance case was rejected by the rasterizer's
all-edges-times-height work estimate. Scanning only vertically active edges
allows this valid field to render under the unchanged 100 million edge/row
work limit. A native 16x10 strike then replaces the scaled fallback; the
requested width 8 clamps to 10 under the existing ZD621 minimum-size option.
The original conformance source and printer PNG now match exactly.

Fourteen raw frames are hash-pinned and required to match every pixel: that
original frame plus twelve ASCII sampling pages and an independent composition
check. The 95 glyphs are reproduced byte-for-byte from these native pages in
the extraction test. Sampling used ZD621 203-DPI firmware V93.21.33Z on
2026-09-20, serialized requests, five-second pacing, no failed requests and no
image scaling or registration. The original frame was copied unchanged from
the complete 2026-09-19 comparison conformance reference after validating both
source and PNG hashes. Other exact strikes and fallback sizes are unchanged.

See Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
font dimensions in ^A pp. 60–61 and field blocks in ^FB pp. 186–188.
Tests explicitly select ZD621_203_DPI. The short-contour and excessive-active-
work tests in `raster_output.rs` separately protect the resource-limit behavior.
