//! Compact complete bitmap records with separately evidenced encoding maps.
//! Raw record IDs and input keys occupy different namespaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Input { ci: u8 },
    Ci0Source,
    CandidateCharacters,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Matched,
    BlankUnresolved,
    Unmatched,
    FilenameFallback,
    UnverifiedCandidate,
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
            4 => Status::UnverifiedCandidate,
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

/// Unverified byte-to-character table. Compose explicitly with a
/// CandidateCharacters map; never treat this as a measured input mapping.
#[derive(Debug)]
pub struct CandidateCodePage {
    pub ci: u8,
    pub byte_to_character: &'static [u16; 256],
}
impl CandidateCodePage {
    pub fn candidate(&self, font: &Font, byte: u8) -> Option<Resolution> {
        font.encoding(Encoding::CandidateCharacters)?
            .lookup(u32::from(self.byte_to_character[usize::from(byte)]))
    }
}
