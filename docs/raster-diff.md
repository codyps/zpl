# Binary PNG image diff

The Rust `png-diff` binary in the `raster-diff` crate compares printer and local
renderer output in printer dots and produces a colored PNG. It uses the in-tree PNG codec; no imaging library
or external executable is needed.

```sh
direnv exec . cargo run -p raster-diff --bin png-diff -- \
  printer.png local.png diff.png
```

Input order matters: the **first image is the reference**, the second is the
candidate being evaluated. The output filename must not already exist.

| Output color | Meaning |
| --- | --- |
| Magenta (`#D81B60`) | Black only in the reference: candidate is missing ink |
| Cyan (`#00A6D6`) | Black only in the candidate: candidate has extra ink |
| Dark gray (`#404040`) | Both images have black ink |
| White (`#FFFFFF`) | Both images are white |

The CLI reports both input dimensions, directional pixel counts, the total
mismatch percentage, and the smallest rectangle containing all differing pixels.
It also reports *ink IoU*: shared black pixels divided by pixels that are black
in either image. IoU is 1 for identical images, including two blank images.
Counts and bounds always refer to the original resolution, before magnification.

## Options

```sh
# Magnify each diff pixel to a 4-by-4 block for inspection.
direnv exec . cargo run -p raster-diff --bin png-diff -- \
  --scale 4 printer.png local.png diff-large.png

# Fail a CI check when the pixels or image dimensions differ.
direnv exec . cargo run -p raster-diff --bin png-diff -- \
  --check printer.png local.png diff-check.png

# Explicitly compare different canvas sizes with white padding on right/bottom.
direnv exec . cargo run -p raster-diff --bin png-diff -- \
  --pad printer.png local.png diff-padded.png
```

By default dimensions must match and decoded pixels must be exactly black or
white. This accepts 1-bit grayscale/indexed PNGs and binary images stored in
8-bit containers. Grayscale or colored pixels cause an error rather than silently
hiding antialiasing differences. `--threshold 128` explicitly converts luminance
below 128 to black; other pixels become white. Alpha is composited on white.

The comparison never aligns, crops, rescales, or shifts inputs automatically.
`--pad` retains a common top-left origin. Different dimensions still fail
`--check`, even if the padded pixels match. `--scale` affects only the output
and uses nearest-neighbor magnification.

Exit codes: **0** successful output; **1** differences with `--check`; **2**
invalid input, unsupported PNG, mismatched dimensions without `--pad`, existing
output, or another operational error. The diff is written before exit code 1.

## Library and limits

`raster_diff::compare(&reference, &candidate, pad)` operates on binary
`raster_diff::Raster` values. `Diff` exposes counts, bounds, RGB pixels, `matches()`,
`ink_iou()`, and `png(scale)`. `Raster::decode_png_with_threshold(bytes, None)`
provides strict binary decoding. Pass `Some(threshold)` to opt into binarization.

PNG input supports noninterlaced grayscale/indexed 1/2/4/8-bit and RGB/alpha
8-bit data, all five row filters, and stored/fixed/dynamic DEFLATE. CRCs and
lengths are checked. Each input is limited to 16 MiB and 16 Mi pixels; the diff
canvas and magnified output are limited to 32 Mi pixels. PNG output uses stored
DEFLATE blocks, so diagnostic files can be large.

The [font extractor](font-extraction.md) uses this same comparison implementation
for its optional text verification and saves `verification-diff.png`.

## Validation

```sh
direnv exec . cargo test -p raster-diff
```

Tests cover directional colors, swapped inputs, blank/equal images, unequal
canvases, padding, bounds, scale limits, PNG filters/bit depths/transparency,
strict binary checks, corruption and truncation. A generated 1-bit fixture pair
was also checked through the CLI: 256 missing pixels and 400 extra pixels,
with the resulting RGB PNG independently decoded and visually inspected.

## Crate boundary

`raster-diff` (in the `raster-diff/` directory, imported as `raster_diff`) has no
dependencies and builds without ZPL. It owns binary rasters,
PNG decoding/encoding, bounded zlib decoding, and comparison. `zpl` depends on it
and keeps scene/path rasterization in `zpl::output::rasterize(&scene)`.
`zpl::output::Raster` re-exports the shared raster type; no pixel copies are needed.
The printer font extractor also calls `raster_diff::compare` directly.
