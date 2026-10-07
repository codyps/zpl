//! Fixed scalable-font captures, compiled into the same pool and glyph view as
//! measured native fonts. Keys describe the capture's Unicode/layout namespace;
//! compatibility variants are separate maps, never inferred printer cmap entries.
use crate::{collection::Pool, Glyph};

#[derive(Debug)]
pub struct Strike {
    pub font: char,
    pub height: u16,
    /// Zero means the font's natural width (height for these scalable captures).
    pub width: u16,
    pub dpi: u16,
    pub keys: &'static [u32],
    pub records: &'static [u16],
    pub pool: &'static Pool,
}
impl Strike {
    pub fn glyph(&self, key: u32) -> Option<Glyph> {
        let index = self.keys.binary_search(&key).ok()?;
        Some(
            self.pool
                .record(usize::from(*self.records.get(index)?))?
                .captured_glyph(),
        )
    }
}

/// Original capture identity for provenance and byte-exact migration checks.
pub struct Capture {
    pub name: &'static str,
    pub legacy_format: u8,
    pub sha256: &'static str,
    pub strike: &'static Strike,
}

#[cfg(feature = "zd621")]
#[path = "zd621/captures.rs"]
pub mod zd621;
