//! Compact bitmap glyphs and encoding mappings in one shared dataset.
//! Keys are printer input/source positions, not an inferred Unicode character map.
//! Generate modules with `zpl-font-extract compile`; see the workspace font guide.
#![no_std]
#![forbid(unsafe_code)]

extern crate self as zpl_bitmap_fonts;

/// Versioned glyphs and encoding observations from the ZD621 at 203 dpi.
#[cfg(feature = "zd621")]
#[path = "zd621/fonts.rs"]
pub mod zd621;

/// Resident bitmap face aliases for a 203-dpi printer. C and D share a matrix;
/// E and H select their 203-dpi variants. `@` denotes the separate GS face.
#[cfg(feature = "zd621")]
pub fn resident(id: char) -> Option<&'static Font> {
    let name = match id {
        'A' => "Z:A.FNT",
        'B' => "Z:B.FNT",
        'C' | 'D' => "Z:D.FNT",
        'E' => "Z:E8.FNT",
        'F' => "Z:F.FNT",
        'G' => "Z:G.FNT",
        'H' => "Z:H8.FNT",
        '@' => "Z:GS.FNT",
        _ => return None,
    };
    zd621::font_by_name(name)
}

pub mod captures;
pub mod collection;
pub use collection::{Font, Glyph, Metrics};
