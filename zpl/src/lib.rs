//! Parse ZPL command streams and render output-independent label scenes.
//!
//! [`render()`] converts ZPL bytes into a [`render::Document`] containing label
//! scenes and rendering warnings. [`Options`] controls dimensions, resolution,
//! and compatibility behavior; its default selects the ZD621 203-DPI profile.
//! See [`render::profiles`] for other printer and specification profiles.
//!
//! Output adapters in [`output`] encode scenes as PNG or SVG. For direct pixel
//! access, use [`output::raster::rasterize`]. Bitmap font types and data live in
//! [`bitmap_font`].
//!
//! For command framing without rendering, use [`parse::ParseContext`]. The
//! parser preserves unknown commands and binary payloads; successful parsing
//! does not imply that the renderer supports the input.
//!
//! ```
//! use zpl::{render, Options};
//!
//! let document = render(b"^XA^FO20,20^FDHello^FS^XZ", Options::default())?;
//! assert_eq!(document.labels.len(), 1);
//! # Ok::<(), zpl::render::RenderError>(())
//! ```

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme {}

pub mod bitmap_font;
pub mod output;
pub mod parse;
pub mod render;

pub use render::{render, Options};
