//! Compact complete bitmap records with separately evidenced encoding maps.
//! Raw record IDs and input keys occupy different namespaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Input { ci: u8 },
    Ci0Source,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Matched,
    BlankUnresolved,
    Unmatched,
    FilenameFallback,
}
#[derive(Debug)]
pub struct Pool {
    /// advance, signed left/top as u16 bits, declared width, height.
    pub metrics: &'static [[u16; 5]],
    pub offsets: &'static [u32],
    pub bits: &'static [u8],
}
#[derive(Debug)]
pub struct Font {
    pub name: &'static str,
    /// Measured cell width/height, baseline and space advance, if available.
    pub metrics: Option<[u16; 4]>,
    pub ids: &'static [u16],
    pub records: &'static [u16],
    pub pool: &'static Pool,
    pub encodings: &'static [Map],
}
#[derive(Debug)]
pub struct Map {
    pub encoding: Encoding,
    pub inputs: &'static [u32],
    /// Offset/count in the shared raw-ID candidate array.
    pub spans: &'static [[u32; 2]],
    pub statuses: &'static [u8],
    pub candidates: &'static [u16],
}
#[derive(Clone, Copy, Debug)]
pub struct Resolution {
    pub status: Status,
    pub candidates: &'static [u16],
}
impl Map {
    /// None means untested. Even one matched candidate is observed pixel/metric
    /// equivalence, not proof of the printer's internal record selection.
    pub fn lookup(&self, input: u32) -> Option<Resolution> {
        let i = self.inputs.binary_search(&input).ok()?;
        let status = match *self.statuses.get(i)? {
            0 => Status::Matched,
            1 => Status::BlankUnresolved,
            2 => Status::Unmatched,
            3 => Status::FilenameFallback,
            _ => return None,
        };
        let &[start, len] = self.spans.get(i)?;
        let start = start as usize;
        Some(Resolution {
            status,
            candidates: self
                .candidates
                .get(start..start.checked_add(len as usize)?)?,
        })
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Record {
    pub advance: u16,
    pub left: i16,
    pub top: i16,
    pub width: u16,
    pub height: u16,
    bits: &'static [u8],
}
impl Font {
    pub fn record(&self, id: u16) -> Option<Record> {
        let i = self.ids.binary_search(&id).ok()?;
        let i = usize::from(*self.records.get(i)?);
        let &[advance, left, top, width, height] = self.pool.metrics.get(i)?;
        let start = *self.pool.offsets.get(i)? as usize;
        let len = usize::from(width)
            .div_ceil(8)
            .checked_mul(usize::from(height))?;
        Some(Record {
            advance,
            left: left as i16,
            top: top as i16,
            width,
            height,
            bits: self.pool.bits.get(start..start.checked_add(len)?)?,
        })
    }
    pub fn encoding(&self, encoding: Encoding) -> Option<&Map> {
        self.encodings.iter().find(|m| m.encoding == encoding)
    }
}
impl Record {
    /// Original padded rows. Padding ink is retained; declared width is separate.
    pub fn bitmap(&self) -> &'static [u8] {
        self.bits
    }
    pub fn pixel(&self, x: u16, y: u16) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        self.storage_pixel(x, y)
    }
    pub fn storage_pixel(&self, x: u16, y: u16) -> bool {
        let stride = usize::from(self.width).div_ceil(8);
        if usize::from(x) >= stride * 8 || y >= self.height {
            return false;
        }
        self.bits[usize::from(y) * stride + usize::from(x) / 8] & (128 >> (x % 8)) != 0
    }
}

/// Native cell dimensions inferred from printer previews.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metrics {
    pub cell_width: u16,
    pub cell_height: u16,
    pub baseline: u16,
    pub space_advance: u16,
}
/// Tight glyph view over a padded record; no second bitmap allocation or copy.
#[derive(Clone, Copy, Debug)]
pub struct Glyph {
    pub advance: u16,
    pub left: i16,
    pub top: i16,
    pub width: u16,
    pub height: u16,
    record: Record,
    x: u16,
    y: u16,
}
impl Glyph {
    /// Original padded bytes. Use row_offset for the beginning of each tight row.
    pub fn bitmap(&self) -> &'static [u8] {
        self.record.bitmap()
    }
    pub fn row_offset(&self, y: usize) -> usize {
        (y + usize::from(self.y)) * usize::from(self.record.width).div_ceil(8) * 8
            + usize::from(self.x)
    }
    pub fn pixel(&self, x: u16, y: u16) -> bool {
        x < self.width && y < self.height && self.record.storage_pixel(x + self.x, y + self.y)
    }
}
impl Font {
    pub fn cell_metrics(&self) -> Option<Metrics> {
        let [cell_width, cell_height, baseline, space_advance] = self.metrics?;
        Some(Metrics {
            cell_width,
            cell_height,
            baseline,
            space_advance,
        })
    }
    /// Resolve a measured CI0 source input for rendering. Multiple matched IDs
    /// are observed pixel/advance equivalents; this does not identify one raw ID.
    /// Unverified candidates and unresolved observations never supply renderer glyphs.
    pub fn glyph(&self, key: u8) -> Option<Glyph> {
        self.encoded_glyph(Encoding::Ci0Source, u32::from(key))
    }
    /// Resolve a verified input mapping, excluding candidates and unresolved entries.
    pub fn encoded_glyph(&self, encoding: Encoding, input: u32) -> Option<Glyph> {
        let resolution = self.encoding(encoding)?.lookup(input)?;
        if resolution.status != Status::Matched {
            return None;
        }
        let record = self.record(*resolution.candidates.first()?)?;
        let (mut left, mut top, mut right, mut bottom) = (u16::MAX, u16::MAX, 0, 0);
        for y in 0..record.height {
            for x in 0..(usize::from(record.width).div_ceil(8) * 8) as u16 {
                if record.storage_pixel(x, y) {
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x + 1);
                    bottom = bottom.max(y + 1);
                }
            }
        }
        if left == u16::MAX {
            return Some(Glyph {
                advance: record.advance,
                left: 0,
                top: 0,
                width: 0,
                height: 0,
                record,
                x: 0,
                y: 0,
            });
        }
        Some(Glyph {
            advance: record.advance,
            left: record.left.checked_add_unsigned(left)?,
            top: record.top.checked_add_unsigned(top)?,
            width: right - left,
            height: bottom - top,
            record,
            x: left,
            y: top,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_glyph_requires_measured_matches() {
        static POOL: Pool = Pool {
            metrics: &[[1, 0, 0, 1, 1]],
            offsets: &[0],
            bits: &[128],
        };
        static FONT: Font = Font {
            name: "test",
            metrics: None,
            ids: &[7],
            records: &[0],
            pool: &POOL,
            encodings: &[Map {
                encoding: Encoding::Input { ci: 28 },
                inputs: &[65, 66, 67, 68, 69],
                spans: &[[0, 1]; 5],
                statuses: &[0, 1, 2, 3, 4],
                candidates: &[7],
            }],
        };
        let encoding = Encoding::Input { ci: 28 };
        assert!(FONT.encoded_glyph(encoding, 65).unwrap().pixel(0, 0));
        // Even a populated candidate list does not make unresolved or candidate
        // evidence safe for rendering. Untested encodings/inputs also fail.
        for input in 66..=70 {
            assert!(FONT.encoded_glyph(encoding, input).is_none());
        }
        assert!(FONT.encoded_glyph(Encoding::Input { ci: 27 }, 65).is_none());
    }
}
