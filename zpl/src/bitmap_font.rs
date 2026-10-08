//! Decoded bitmap strike types and validation. No serialized font format is required.
//! Bundled fonts use the compact `zpl-bitmap-fonts` reader directly.
/// Internal strike tag for ^GS, distinct from the resident ^AS font.
pub const GRAPHIC_SYMBOLS: char = '@';

#[derive(Debug, Clone, Copy)]
pub struct Settings {
    /// Resident font ID, or GRAPHIC_SYMBOLS for the separate ^GS face.
    pub font: char,
    pub height: u32,
    pub width: u32,
    pub dpi: u32,
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !"0ABCDEFGHPQRSTUV@".contains(self.font)
            || !(1..=128).contains(&self.height)
            || self.width > 128
            || !(1..=2400).contains(&self.dpi)
        {
            return Err("invalid font, size or DPI".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glyph {
    pub codepoint: u32,
    pub advance: u32,
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub bitmap: Vec<Vec<u8>>,
}
/// Validate bitmap dimensions and metrics before rendering or export.
pub fn validate_glyphs(glyphs: &[Glyph]) -> Result<(), String> {
    if glyphs.len() > 4096 {
        return Err("too many glyphs".into());
    }
    for g in glyphs {
        if !valid_codepoint(g.codepoint)
            || g.width > 4096
            || g.height > 4096
            || g.advance > 4096
            || g.left.unsigned_abs() > 4096
            || g.top.unsigned_abs() > 4096
            || g.bitmap.len() != g.height as usize
            || g.bitmap
                .iter()
                .any(|r| r.len() != g.width.div_ceil(8) as usize)
        {
            return Err("invalid glyph metrics or bitmap".into());
        }
    }
    Ok(())
}
/// Unicode scalar values other than C0/C1 controls and DEL, accepted by font captures.
pub fn valid_codepoint(code: u32) -> bool {
    char::from_u32(code).is_some() && ((32..=126).contains(&code) || code >= 160)
}
