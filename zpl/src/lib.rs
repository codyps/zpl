//! Parse ZPL and render local previews.
//!
//! Use [`render()`] and [`Options`] for rendering. Supporting types live in
//! [`mod@render`], output adapters in [`output`], parser types in [`parse`], and
//! bitmap font data in [`bitmap_font`]. Rasterization lives in [`output::raster`].
//!
//! ```
//! use zpl::{render, Options};
//!
//! let document = render(b"^XA^FO20,20^FDHello^FS^XZ", Options::default())?;
//! assert_eq!(document.labels.len(), 1);
//! # Ok::<(), zpl::render::RenderError>(())
//! ```

pub mod bitmap_font;
pub mod output;
pub mod parse;
pub mod render;

pub use render::{render, Options};
