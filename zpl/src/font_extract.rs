//! Resident-font sampling geometry, bitmap extraction and verification, without transport.
use crate::output::Raster;
use std::fmt::Write;
#[derive(Debug, Clone, Copy)]
pub struct Settings {
    pub font: char,
    pub height: u32,
    pub width: u32,
    pub dpi: u32,
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !"0ABCDEFGH".contains(self.font)
            || !(1..=128).contains(&self.height)
            || self.width > 128
            || !(1..=2400).contains(&self.dpi)
        {
            return Err("invalid font, size or DPI".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct Tile {
    code: Option<u8>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    baseline: u32,
    split: u32,
}
#[derive(Debug, Clone)]
pub struct Page {
    pub zpl: String,
    pub width: u32,
    pub height: u32,
    tiles: Vec<Tile>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glyph {
    pub codepoint: u8,
    pub advance: u32,
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub bitmap: Vec<Vec<u8>>,
}
/// ASCII glyphs are hex-escaped, including all ZPL syntax characters.
pub fn page_plan(codes: &[u8], s: Settings) -> Result<Page, String> {
    s.validate()?;
    if codes.is_empty() || codes.len() > 16 || codes.iter().any(|c| !(32..=126).contains(c)) {
        return Err("sample must contain 1..16 printable ASCII glyphs".into());
    }
    let cw = s.height.max(s.width) * 6 + 32;
    let ch = s.height * 5 + 48;
    let width = (cw * 2).div_ceil(64) * 64;
    let height = (codes.len() as u32 + 1).div_ceil(2) * ch;
    let mut zpl = format!("^XA^PW{width}^LL{height}^LH0,0^LS0^LT0^PON^LRN^CI27");
    let mut tiles = Vec::new();
    for (i, code) in std::iter::once(None)
        .chain(codes.iter().copied().map(Some))
        .enumerate()
    {
        let (x, y) = (i as u32 % 2 * cw, i as u32 / 2 * ch);
        let base = y + 16 + s.height * 2;
        let second = y + 32 + s.height * 4;
        let encoded = code.map(|c| format!("_{c:02X}")).unwrap_or_default();
        if code.is_some() {
            write!(
                zpl,
                "\n^FT{},{}^A{}N,{},{}^FH^FD{}^FS",
                x + 16,
                base,
                s.font,
                s.height,
                s.width,
                encoded
            )
            .unwrap();
        }
        write!(
            zpl,
            "\n^FT{},{}^A{}N,{},{}^FH^FD_7C{}_7C^FS",
            x + 16,
            second,
            s.font,
            s.height,
            s.width,
            encoded
        )
        .unwrap();
        tiles.push(Tile {
            code,
            x,
            y,
            width: cw,
            height: ch,
            baseline: base,
            // Leave room for scaled descenders and underscores below FT.
            split: base + 16.max(s.height / 2),
        });
    }
    zpl.push_str("\n^XZ");
    Ok(Page {
        zpl,
        width,
        height,
        tiles,
    })
}
fn valid_image(image: &Raster) -> Result<(), String> {
    if image.width == 0
        || image.height == 0
        || (image.width as usize).checked_mul(image.height as usize) != Some(image.pixels.len())
    {
        return Err("invalid raster dimensions".into());
    }
    Ok(())
}
fn crop(image: &Raster, x: u32, y: u32, w: u32, h: u32) -> Result<Vec<&[u8]>, String> {
    if x + w > image.width || y + h > image.height {
        return Err("preview smaller than sampling page".into());
    }
    Ok((y..y + h)
        .map(|row| {
            let start = (row * image.width + x) as usize;
            &image.pixels[start..start + w as usize]
        })
        .collect())
}
type Bounds = (usize, usize, usize, usize);
fn bounds(rows: &[&[u8]]) -> Result<Option<Bounds>, String> {
    let (mut left, mut top, mut right, mut bottom) = (usize::MAX, usize::MAX, 0, 0);
    for (y, row) in rows.iter().enumerate() {
        for (x, &p) in row.iter().enumerate() {
            if p == 0 {
                left = left.min(x);
                top = top.min(y);
                right = right.max(x + 1);
                bottom = bottom.max(y + 1);
            }
        }
    }
    if left == usize::MAX {
        return Ok(None);
    }
    if left == 0 || top == 0 || right == rows[0].len() || bottom == rows.len() {
        return Err("glyph/probe touches tile edge".into());
    }
    Ok(Some((left, top, right, bottom)))
}
fn probe<'a>(image: &'a Raster, t: &Tile) -> Result<(Vec<&'a [u8]>, Bounds), String> {
    let rows = crop(image, t.x, t.split, t.width, t.y + t.height - t.split)?;
    let b = bounds(&rows)?.ok_or("empty advance probe")?;
    Ok((rows, b))
}
pub fn extract_page(image: &Raster, page: &Page) -> Result<Vec<Glyph>, String> {
    valid_image(image)?;
    if (image.width, image.height) != (page.width, page.height) {
        return Err("preview dimensions differ from requested page".into());
    }
    let (reference, b) = probe(image, &page.tiles[0])?;
    let mut sentinel = b.2 - 1;
    while sentinel > 0 && reference.iter().any(|row| row[sentinel - 1] == 0) {
        sentinel -= 1
    }
    if sentinel <= b.0 {
        return Err("advance probes have no gap between sentinels".into());
    }
    let mut glyphs = Vec::new();
    for t in &page.tiles[1..] {
        let rows = crop(image, t.x, t.y, t.width, t.split - t.y)?;
        let bbox = bounds(&rows)?;
        let (measured, mb) = probe(image, t)?;
        let advance =
            mb.2.checked_sub(b.2)
                .filter(|&n| n > 0)
                .ok_or("invalid glyph advance")?;
        for (y, row) in reference.iter().enumerate() {
            for (x, &pixel) in row.iter().enumerate().take(b.2).skip(sentinel) {
                if pixel != measured[y][x + advance] {
                    return Err("advance sentinel mismatch".into());
                }
            }
        }
        let mut g = Glyph {
            codepoint: t.code.unwrap(),
            advance: advance as u32,
            left: 0,
            top: 0,
            width: 0,
            height: 0,
            bitmap: Vec::new(),
        };
        if let Some((x0, y0, x1, y1)) = bbox {
            g.left = x0 as i32 - 16;
            g.top = y0 as i32 - (t.baseline - t.y) as i32;
            g.width = (x1 - x0) as u32;
            g.height = (y1 - y0) as u32;
            for row in &rows[y0..y1] {
                let mut bits = vec![0; (x1 - x0).div_ceil(8)];
                for (x, &p) in row[x0..x1].iter().enumerate() {
                    if p == 0 {
                        bits[x / 8] |= 128 >> (x % 8)
                    }
                }
                g.bitmap.push(bits);
            }
        } else if g.codepoint != 32 {
            return Err(format!("blank printable glyph U+{:04X}", g.codepoint));
        }
        glyphs.push(g);
    }
    Ok(glyphs)
}
pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::new();
    for b in bytes {
        write!(s, "{b:02X}").unwrap();
    }
    s
}
fn validate_glyphs(glyphs: &[Glyph]) -> Result<(), String> {
    if glyphs.len() > 95 {
        return Err("too many glyphs".into());
    }
    for g in glyphs {
        if g.width > 4096
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
pub fn bdf(glyphs: &[Glyph], s: Settings) -> Result<String, String> {
    s.validate()?;
    validate_glyphs(glyphs)?;
    let (mut left, mut top, mut right, mut bottom) = (0, 0, 0, 0);
    for g in glyphs.iter().filter(|g| g.width > 0) {
        left = left.min(g.left);
        top = top.min(g.top);
        right = right.max(g.left + g.width as i32);
        bottom = bottom.max(g.top + g.height as i32);
    }
    let points = (s.height as f64 * 72. / s.dpi as f64)
        .round_ties_even()
        .max(1.) as u32;
    let mut out=format!("STARTFONT 2.1\nFONT ZebraPreview-{}-{}\nSIZE {points} {} {}\nFONTBOUNDINGBOX {} {} {left} {}\nSTARTPROPERTIES 4\nFONT_ASCENT {}\nFONT_DESCENT {bottom}\nCHARSET_REGISTRY \"ISO10646\"\nCHARSET_ENCODING \"1\"\nENDPROPERTIES\nCHARS {}\n",s.font,s.height,s.dpi,s.dpi,right-left,bottom-top,-bottom,-top,glyphs.len());
    for g in glyphs {
        let sw = (g.advance as f64 * 72000. / (points * s.dpi) as f64).round_ties_even() as u32;
        write!(
            out,
            "STARTCHAR U{:04X}\nENCODING {}\nSWIDTH {sw} 0\nDWIDTH {} 0\nBBX {} {} {} {}\nBITMAP\n",
            g.codepoint,
            g.codepoint,
            g.advance,
            g.width,
            g.height,
            g.left,
            -g.top - g.height as i32
        )
        .unwrap();
        for row in &g.bitmap {
            writeln!(out, "{}", hex(row)).unwrap();
        }
        out.push_str("ENDCHAR\n");
    }
    out.push_str("ENDFONT\n");
    Ok(out)
}
pub fn verification_plan(
    glyphs: &[Glyph],
    s: Settings,
    text: &str,
) -> Result<(String, Raster), String> {
    s.validate()?;
    validate_glyphs(glyphs)?;
    if text.is_empty() || text.len() > 4096 || !text.is_ascii() {
        return Err("invalid verification text".into());
    }
    let selected: Vec<_> = text
        .bytes()
        .map(|c| {
            glyphs
                .iter()
                .find(|g| g.codepoint == c)
                .ok_or("verification text contains uncaptured glyph")
        })
        .collect::<Result<_, _>>()?;
    let width = (selected.iter().map(|g| g.advance).sum::<u32>() + 32 + s.height).div_ceil(64) * 64;
    let height = s.height * 4 + 32;
    let baseline = s.height * 2 + 16;
    if width > 4096 {
        return Err("verification text too wide".into());
    }
    let mut image = Raster {
        width,
        height,
        pixels: vec![255; (width * height) as usize],
    };
    let mut pen = 16i32;
    for g in selected {
        for (y, row) in g.bitmap.iter().enumerate() {
            for dx in 0..g.width as usize {
                if row[dx / 8] & (128 >> (dx % 8)) != 0 {
                    let (x, y) = (pen + g.left + dx as i32, baseline as i32 + g.top + y as i32);
                    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                        return Err("verification glyph exceeds canvas".into());
                    }
                    image.pixels[(y as u32 * width + x as u32) as usize] = 0;
                }
            }
        }
        pen += g.advance as i32;
    }
    let encoded: String = text.bytes().map(|c| format!("_{c:02X}")).collect();
    let zpl=format!("^XA^PW{width}^LL{height}^LH0,0^LS0^LT0^PON^LRN^CI27^FT16,{baseline}^A{}N,{},{}^FH^FD{encoded}^FS^XZ",s.font,s.height,s.width);
    Ok((zpl, image))
}

/// Compact ZBF1 strike: settings and per-glyph metrics followed by contiguous
/// MSB-first bitmap bits (no per-row padding). Integers are little endian.
pub fn pack(glyphs: &[Glyph], settings: Settings) -> Result<Vec<u8>, String> {
    settings.validate()?;
    validate_glyphs(glyphs)?;
    if glyphs.is_empty()
        || glyphs.iter().any(|g| !(32..=126).contains(&g.codepoint))
        || glyphs.windows(2).any(|g| g[0].codepoint >= g[1].codepoint)
    {
        return Err("strike glyphs must be sorted, unique printable ASCII".into());
    }
    let mut out = b"ZBF1".to_vec();
    out.push(settings.font as u8);
    for n in [
        settings.height,
        settings.width,
        settings.dpi,
        glyphs.len() as u32,
    ] {
        out.extend((n as u16).to_le_bytes());
    }
    for g in glyphs {
        out.push(g.codepoint);
        out.extend((g.advance as u16).to_le_bytes());
        out.extend((g.left as i16).to_le_bytes());
        out.extend((g.top as i16).to_le_bytes());
        out.extend((g.width as u16).to_le_bytes());
        out.extend((g.height as u16).to_le_bytes());
        let size = (g.width as usize * g.height as usize).div_ceil(8);
        if out.len() + size > 2 * 1024 * 1024 {
            return Err("packed strike exceeds 2 MiB".into());
        }
        let mut bits = vec![0; size];
        for (y, row) in g.bitmap.iter().enumerate() {
            for x in 0..g.width as usize {
                if row[x / 8] & (128 >> (x % 8)) != 0 {
                    let i = y * g.width as usize + x;
                    bits[i / 8] |= 128 >> (i % 8);
                }
            }
        }
        out.extend(bits);
    }
    Ok(out)
}
/// Decode a ZBF1 bitmap strike. No external font or image libraries are used.
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
    if r.take(4)? != b"ZBF1" {
        return Err("unknown bitmap strike format".into());
    }
    let s = Settings {
        font: char::from(r.byte()?),
        height: r.u16()? as u32,
        width: r.u16()? as u32,
        dpi: r.u16()? as u32,
    };
    s.validate()?;
    let count = r.u16()? as usize;
    if !(1..=95).contains(&count) {
        return Err("invalid strike glyph count".into());
    }
    let mut glyphs = Vec::with_capacity(count);
    let mut previous = 31;
    for _ in 0..count {
        let codepoint = r.byte()?;
        if !(32..=126).contains(&codepoint) || codepoint <= previous {
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

#[cfg(test)]
mod packed_tests {
    use super::*;
    #[test]
    fn compact_roundtrip_and_truncation() {
        let s = Settings {
            font: '0',
            height: 32,
            width: 0,
            dpi: 203,
        };
        let glyphs = vec![
            Glyph {
                codepoint: 32,
                advance: 9,
                left: 0,
                top: 0,
                width: 0,
                height: 0,
                bitmap: vec![],
            },
            Glyph {
                codepoint: 65,
                advance: 5,
                left: -1,
                top: -3,
                width: 3,
                height: 3,
                bitmap: vec![vec![0x40], vec![0xa0], vec![0xe0]],
            },
        ];
        let packed = pack(&glyphs, s).unwrap();
        assert_eq!(packed.len(), 13 + 22 + 2);
        let (got, decoded) = unpack(&packed).unwrap();
        assert_eq!(
            (got.font, got.height, got.width, got.dpi),
            ('0', 32, 0, 203)
        );
        assert_eq!(decoded, glyphs);
        for n in 0..packed.len() {
            assert!(unpack(&packed[..n]).is_err())
        }
        let mut bad = packed.clone();
        *bad.last_mut().unwrap() |= 1;
        assert!(unpack(&bad).is_err());
        let mut bad = packed.clone();
        bad.extend([0]);
        assert!(unpack(&bad).is_err());
        let mut duplicates = glyphs.clone();
        duplicates.push(glyphs[1].clone());
        assert!(pack(&duplicates, s).is_err());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn large_underscore_stays_in_the_isolated_glyph_tile() {
        let settings = super::Settings {
            font: '0',
            height: 64,
            width: 0,
            dpi: 203,
        };
        let page = super::page_plan(&(88..=95).collect::<Vec<_>>(), settings).unwrap();
        assert_eq!(
            page.zpl.as_bytes(),
            include_bytes!("../tests/fixtures/font64/page-007.zpl")
        );
        let raster =
            super::Raster::decode_png(include_bytes!("../tests/fixtures/font64/page-007.png"))
                .unwrap();
        let glyphs = super::extract_page(&raster, &page).unwrap();
        let underscore = glyphs.iter().find(|g| g.codepoint == b'_').unwrap();
        assert!(underscore.height > 0);
        assert_eq!(
            (underscore.top, underscore.height, underscore.width),
            (12, 4, 32)
        );
    }

    use super::*;
    fn settings() -> Settings {
        Settings {
            font: '0',
            height: 8,
            width: 0,
            dpi: 203,
        }
    }
    fn draw(image: &mut Raster, text: &[u8], mut x: u32, baseline: u32) {
        for &c in text {
            let (advance, left, top, rows): (u32, u32, i32, &[&str]) = match c {
                b'|' => (3, 1, -5, &["1", "1", "1", "1", "1"]),
                b'A' => (5, 1, -4, &["010", "101", "111", "101"]),
                b'j' => (4, 0, -3, &["001", "001", "001", "101", "010"]),
                b' ' => (4, 0, 0, &[]),
                _ => panic!(),
            };
            for (y, row) in rows.iter().enumerate() {
                for (dx, p) in row.bytes().enumerate() {
                    if p == b'1' {
                        image.pixels[((baseline as i32 + top + y as i32) as u32 * image.width
                            + x
                            + left
                            + dx as u32) as usize] = 0;
                    }
                }
            }
            x += advance;
        }
    }
    fn fixture() -> (Page, Raster) {
        let p = page_plan(b" Aj", settings()).unwrap();
        let mut r = Raster {
            width: p.width,
            height: p.height,
            pixels: vec![255; (p.width * p.height) as usize],
        };
        for t in &p.tiles {
            let text = t.code.map(|c| vec![c]).unwrap_or_default();
            draw(&mut r, &text, t.x + 16, t.baseline);
            let mut probe = vec![b'|'];
            probe.extend(text);
            probe.push(b'|');
            draw(&mut r, &probe, t.x + 16, t.baseline + 32);
        }
        (p, r)
    }
    #[test]
    fn metrics_space_descender_and_bdf() {
        let (p, r) = fixture();
        let g = extract_page(&r, &p).unwrap();
        assert_eq!(g.iter().map(|g| g.advance).collect::<Vec<_>>(), [4, 5, 4]);
        assert!(g[0].bitmap.is_empty());
        assert_eq!((g[1].left, g[1].top), (1, -4));
        assert_eq!(
            g[1].bitmap,
            vec![vec![0x40], vec![0xa0], vec![0xe0], vec![0xa0]]
        );
        assert_eq!(g[2].top + g[2].height as i32, 2);
        let b = bdf(&g, settings()).unwrap();
        assert!(b.contains("DWIDTH 4 0\nBBX 3 5 0 -2"));
        assert!(b.contains("FONT_DESCENT 2"));
    }
    #[test]
    fn verification_and_escaping() {
        let (p, r) = fixture();
        let g = extract_page(&r, &p).unwrap();
        let (zpl, r) = verification_plan(&g, settings(), "A j").unwrap();
        assert!(zpl.contains("^FD_41_20_6A^FS"));
        assert_eq!(r.pixels.iter().filter(|&&v| v == 0).count(), 14);
        assert_eq!(r.pixels[28 * r.width as usize + 18], 0);
        assert_eq!(r.pixels[33 * r.width as usize + 26], 0);
        let p = page_plan(b"^~_", settings()).unwrap();
        for s in ["_5E", "_7E", "_5F"] {
            assert!(p.zpl.contains(&format!("^FD{s}^FS")))
        }
        assert!(verification_plan(&g, settings(), "missing").is_err());
    }
    #[test]
    fn invalid_samples_and_clipping() {
        assert!(page_plan(&[], settings()).is_err());
        assert!(page_plan(b"\xff", settings()).is_err());
        let (p, mut r) = fixture();
        let t = &p.tiles[1];
        r.pixels[(t.y * r.width + t.x) as usize] = 0;
        assert!(extract_page(&r, &p).unwrap_err().contains("edge"));
        r.pixels.fill(255);
        assert!(extract_page(&r, &p).unwrap_err().contains("empty advance"));
        r.width -= 1;
        assert!(extract_page(&r, &p).is_err());
    }
}
