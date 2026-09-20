//! Bitmap strike types and ZBF1/ZBF2 decoding used by the renderer.
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
/// Validate bitmap dimensions and metrics before decoding or export.
pub fn validate_glyphs(glyphs: &[Glyph]) -> Result<(), String> {
    if glyphs.len() > 4096 {
        return Err("too many glyphs".into());
    }
    for g in glyphs {
        if !valid_codepoint(g.codepoint)
            || g.width > 4096
            || g.height > 4096
            || g.advance > 4096
            || g.advance == 0
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
/// Decode a ZBF1 (byte codepoints) or ZBF2 (u32 Unicode codepoints) bitmap strike. No external font or image libraries are used.
pub fn unpack(data: &[u8]) -> Result<(Settings, Vec<Glyph>), String> {
    if data.len() > 2 * 1024 * 1024 {
        return Err("packed strike exceeds 2 MiB".into());
    }
    struct Reader<'a> {
        data: &'a [u8],
        pos: usize,
    }
    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
            let v = self
                .data
                .get(self.pos..self.pos + n)
                .ok_or("truncated ZBF strike")?;
            self.pos += n;
            Ok(v)
        }
        fn byte(&mut self) -> Result<u8, String> {
            Ok(self.take(1)?[0])
        }
        fn u16(&mut self) -> Result<u16, String> {
            Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
        }
    }
    let mut r = Reader { data, pos: 0 };
    let unicode = match r.take(4)? {
        b"ZBF1" => false,
        b"ZBF2" => true,
        _ => return Err("unknown bitmap strike format".into()),
    };
    let s = Settings {
        font: char::from(r.byte()?),
        height: r.u16()? as u32,
        width: r.u16()? as u32,
        dpi: r.u16()? as u32,
    };
    s.validate()?;
    let count = r.u16()? as usize;
    if !(1..=if unicode { 4096 } else { 191 }).contains(&count) {
        return Err("invalid strike glyph count".into());
    }
    let mut glyphs = Vec::with_capacity(count);
    let mut previous = 31;
    for _ in 0..count {
        let codepoint = if unicode {
            u32::from_le_bytes(r.take(4)?.try_into().unwrap())
        } else {
            r.byte()? as u32
        };
        if !valid_codepoint(codepoint) || codepoint <= previous {
            return Err("invalid strike glyph order".into());
        }
        previous = codepoint;
        let advance = r.u16()? as u32;
        let left = r.u16()? as i16 as i32;
        let top = r.u16()? as i16 as i32;
        let width = r.u16()? as u32;
        let height = r.u16()? as u32;
        if width > 4096 || height > 4096 {
            return Err("excessive strike glyph dimensions".into());
        }
        let n = width as usize * height as usize;
        let bits = r.take(n.div_ceil(8))?;
        if !n.is_multiple_of(8)
            && bits
                .last()
                .is_some_and(|b| b & ((1 << (8 - n % 8)) - 1) != 0)
        {
            return Err("nonzero strike padding".into());
        }
        let mut bitmap = vec![vec![0; width.div_ceil(8) as usize]; height as usize];
        for (y, row) in bitmap.iter_mut().enumerate() {
            for x in 0..width as usize {
                let i = y * width as usize + x;
                if bits[i / 8] & (128 >> (i % 8)) != 0 {
                    row[x / 8] |= 128 >> (x % 8)
                }
            }
        }
        glyphs.push(Glyph {
            codepoint,
            advance,
            left,
            top,
            width,
            height,
            bitmap,
        });
    }
    if r.pos != data.len() {
        return Err("trailing strike data".into());
    }
    validate_glyphs(&glyphs)?;
    Ok((s, glyphs))
}
