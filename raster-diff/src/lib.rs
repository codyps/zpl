//! Pixel-exact binary raster comparison and bounded PNG I/O.
//!
//! [`Raster`] stores one grayscale byte per pixel, in row-major order.
//! [`compare`] requires binary pixels (0 or 255) and compares the reference and
//! candidate at a common top-left origin without alignment or resizing.
//! Optional white padding permits unequal dimensions, but [`Diff::matches`]
//! still requires equal dimensions.
//!
//! [`compare_stats`] returns [`DiffStats`] with the same measurements without
//! allocating a colored image. It is intended for regression tests and bulk audits.
//!
//! [`Diff`] exposes directional mismatch counts, mismatch bounds, and black-pixel
//! intersection over union via [`Diff::ink_iou`]. [`Diff::png`] encodes a colored
//! diagnostic image with optional nearest-neighbor magnification.
//!
//! Use [`Raster::decode_png_with_threshold`] with `None` for strict binary PNG
//! decoding or `Some(threshold)` for explicit binarization. [`Raster::decode_png`]
//! uses a threshold of 128. Alpha is composited on white before binarization.
//! [`Png`] provides grayscale and RGB PNG encoding, and [`compression`] exposes
//! the bounded zlib decoder.

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme {}

/// Bounded zlib decoding, also shared by the ZPL Z64 graphics decoder.
pub mod compression;
mod decode_png;
mod diff;
mod png;
pub use diff::*;
pub use png::Png;

pub const MAX_PIXELS: usize = 32 * 1024 * 1024;

/// Monochrome raster, one grayscale byte per pixel (0 black, 255 white).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageError(pub &'static str);
impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for ImageError {}
