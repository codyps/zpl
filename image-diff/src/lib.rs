//! Binary raster comparison and bounded PNG I/O, independent of any label format.
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
