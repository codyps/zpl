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
    resident_at_dpi(id, 203)
}

/// Select native OCR-A/OCR-B matrices for the printhead density.
/// Zebra Programming Guide, Font Matrices (Tables 29/31):
/// <https://docs.zebra.com/us/en/printers/software/zpl-pg/fonts-and-barcodes/font-matrices.html>.
/// Other resident bitmap matrices are independent of density.
#[cfg(feature = "zd621")]
pub fn resident_at_dpi(id: char, dpi: u32) -> Option<&'static Font> {
    let name = match id {
        'A' => "Z:A.FNT",
        'B' => "Z:B.FNT",
        'C' | 'D' => "Z:D.FNT",
        'E' => match dpi {
            0..=152 => "Z:E6.FNT",
            153..=250 => "Z:E8.FNT",
            251..=450 => "Z:E12.FNT",
            _ => "Z:E24.FNT",
        },
        'F' => "Z:F.FNT",
        'G' => "Z:G.FNT",
        'H' => match dpi {
            0..=152 => "Z:H6.FNT",
            153..=250 => "Z:H8.FNT",
            251..=450 => "Z:H12.FNT",
            _ => "Z:H24.FNT",
        },
        '@' => "Z:GS.FNT",
        _ => return None,
    };
    zd621::font_by_name(name)
}

pub mod captures;
pub mod collection;
pub use collection::{Font, Glyph, Metrics};
