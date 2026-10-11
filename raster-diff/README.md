# raster-diff

Compare black-and-white images pixel by pixel and generate colored PNG diffs.
Includes PNG decoding, encoding, and a `png-diff` CLI, with no dependencies.

## Quick start

Add the crate to your `Cargo.toml` (import it as `raster_diff` in Rust):

```toml
[dependencies]
raster-diff = "0.1"
```

Compare two tiny images and encode a magnified diagnostic PNG:

```rust
use raster_diff::{compare, Raster};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Row-major pixels: 0 is black, 255 is white.
    let reference = Raster {
        width: 2,
        height: 2,
        pixels: vec![0, 0, 255, 255],
    };
    let candidate = Raster {
        width: 2,
        height: 2,
        pixels: vec![0, 255, 0, 255],
    };

    let diff = compare(&reference, &candidate, false)?;
    assert_eq!(diff.reference_only, 1); // Missing ink in the candidate.
    assert_eq!(diff.candidate_only, 1); // Extra ink in the candidate.
    assert_eq!(diff.different_pixels(), 2);
    assert!(!diff.matches());

    let png = diff.png(64)?; // Each input pixel becomes a 64 × 64 block.
    // Save with std::fs::write("diff.png", &png)?;
    assert_eq!(diff.both_black, 1);
    assert_eq!(diff.both_white, 1);
    assert_eq!(diff.ink_iou(), 1.0 / 3.0);
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    Ok(())
}
```

[![Example diff: shared black at top left, missing ink at top right, extra ink at bottom left, shared white at bottom right](https://raw.githubusercontent.com/codyps/zpl/main/docs/examples/raster-diff-quick-start.png)](https://raw.githubusercontent.com/codyps/zpl/main/docs/examples/raster-diff-quick-start.png)

| Color | Meaning |
| --- | --- |
| Magenta | Black only in the reference: missing ink |
| Cyan | Black only in the candidate: extra ink |
| Dark gray | Black in both images |
| White | White in both images |

## Compare without an image

Use `compare_stats(&reference, &candidate, false)` when only counts, bounds,
and foreground IoU are needed. It returns `DiffStats`, validates the same
binary raster and dimension contracts, and allocates no RGB image. Identical
canvases take an equality shortcut after validation. Call `compare` separately
when a diagnostic PNG is actually requested; its existing API is unchanged.

## Compare PNG files

Read each file with `std::fs::read`, then use
`Raster::decode_png_with_threshold(&bytes, None)?` to require strictly
black-and-white pixels. Pass the decoded rasters to `compare` as above.

For grayscale or color inputs, pass `Some(128)` to explicitly turn luminance
below 128 black and the rest white. Alpha is composited on white before this
check. The shorter `Raster::decode_png(&bytes)` uses a threshold of 128.

The first raster is the reference. Comparison uses the same top-left origin,
without alignment or resizing. The final argument to `compare` enables white
padding for unequal dimensions; with `false`, unequal dimensions are an error.
Even with padding, `diff.matches()` returns false when dimensions differ.

`diff.bounds` gives the smallest rectangle containing differing pixels.
`diff.ink_iou()` reports shared black pixels divided by all pixels that are black
in either image; two blank images have IoU 1. Counts and bounds refer to input
pixels, regardless of the output magnification passed to `diff.png(scale)`.

## Command line

```sh
cargo install raster-diff
png-diff --check --scale 4 reference.png candidate.png diff.png
```

The output path must not already exist. `--check` exits with **1** if pixels or
dimensions differ, after writing the diff; **0** means success, and **2** means
an error. Without `--check`, differences still exit successfully.
Use `--pad` for white padding, `--threshold 128` for explicit binarization,
and `--help` for all options.

See the [API documentation](https://docs.rs/raster-diff) and
[PNG support and resource limits](https://github.com/codyps/zpl/blob/main/docs/raster-diff.md#library-and-limits).

## License

Licensed under the [Open Software License 3.0](https://github.com/codyps/zpl/blob/main/LICENSE).
