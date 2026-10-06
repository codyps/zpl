//! Compact bitmap fonts: shared glyph records and a shared, densely packed bit pool.
//! Keys are printer input/source positions, not an inferred Unicode character map.
//! Generate modules with `zpl-font-extract compile`; see the workspace font guide.
#![no_std]
#![forbid(unsafe_code)]

extern crate self as zpl_bitmap_fonts;

/// Previously verified CI0 source-position fonts from the ZD621 at 203 dpi.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mapping {
    Input { encoding: u8 },
    Ci0Source,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metrics {
    pub cell_height: u16,
    pub cell_width: u16,
    /// One-based printer baseline (zero is also representable).
    pub baseline: u16,
    pub space_advance: u16,
}
/// Parallel arrays avoid padding each five-byte metric record to align its offset.
#[derive(Debug)]
pub struct Pool {
    pub metrics: &'static [[u8; 5]],
    pub offsets: &'static [u32],
    pub bits: &'static [u8],
}
#[derive(Debug)]
pub struct Font {
    pub name: &'static str,
    pub mapping: Mapping,
    pub metrics: Metrics,
    pub first_key: u8,
    /// u16::MAX means absent. Observed advancing blanks have real records.
    pub lookup: &'static [u16],
    pub pool: &'static Pool,
}
#[derive(Clone, Copy, Debug)]
pub struct Glyph {
    pub advance: u8,
    pub left: i8,
    pub top: i8,
    pub width: u8,
    pub height: u8,
    bits: &'static [u8],
}
impl Font {
    pub fn glyph(&self, key: u8) -> Option<Glyph> {
        let index = usize::from(
            *self
                .lookup
                .get(usize::from(key.checked_sub(self.first_key)?))?,
        );
        if index == usize::from(u16::MAX) {
            return None;
        }
        let &[advance, left, top, width, height] = self.pool.metrics.get(index)?;
        let start = *self.pool.offsets.get(index)? as usize;
        let len = (usize::from(width) * usize::from(height)).div_ceil(8);
        Some(Glyph {
            advance,
            left: left as i8,
            top: top as i8,
            width,
            height,
            bits: self.pool.bits.get(start..start.checked_add(len)?)?,
        })
    }
}
impl Glyph {
    /// Packed row-major MSB-first bitmap, without padding between rows.
    /// Only the first `width * height` bits are pixels; the final byte may
    /// contain unused trailing bits. Empty glyphs return an empty slice.
    pub fn bitmap(&self) -> &'static [u8] {
        self.bits
    }

    /// Continuous row-major MSB-first bits; rows have no byte padding.
    pub fn pixel(&self, x: u8, y: u8) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let bit = usize::from(y) * usize::from(self.width) + usize::from(x);
        self.bits[bit / 8] & (128 >> (bit % 8)) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    static POOL: Pool = Pool {
        metrics: &[[4, 255, 254, 3, 3], [5, 0, 0, 0, 0]],
        offsets: &[0, 2],
        bits: &[0b10101010, 0b10000000],
    };
    static FONT: Font = Font {
        name: "A",
        mapping: Mapping::Input { encoding: 27 },
        metrics: Metrics {
            cell_height: 7,
            cell_width: 5,
            baseline: 6,
            space_advance: 5,
        },
        first_key: 32,
        lookup: &[1, u16::MAX, 0],
        pool: &POOL,
    };
    #[test]
    fn dense_rows_bearings_blanks_and_missing_keys() {
        let g = FONT.glyph(34).unwrap();
        assert_eq!((g.left, g.top), (-1, -2));
        for y in 0..3 {
            for x in 0..3 {
                assert_eq!(g.pixel(x, y), (x + y) % 2 == 0);
            }
        }
        assert!(!g.pixel(3, 0));
        assert_eq!(FONT.glyph(32).unwrap().advance, 5);
        assert!(FONT.glyph(31).is_none());
        assert!(FONT.glyph(33).is_none());
        assert!(FONT.glyph(35).is_none());
    }
}
