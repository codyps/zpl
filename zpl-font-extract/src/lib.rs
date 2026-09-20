//! Resident-font sampling, extraction, export, and verification without transport.
use std::fmt::Write;
use zpl::{
    bitmap_font::{valid_codepoint, validate_glyphs, Glyph, Settings},
    output::raster::Raster,
};
#[derive(Debug, Clone)]
pub struct Tile {
    code: Option<u32>,
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
// ^GS p. 217 selects the symbol face; it has no printable vertical-bar probe.
fn field_command(font: char) -> String {
    if font == 'S' {
        "GS".into()
    } else {
        format!("A{font}")
    }
}

/// Encode field bytes explicitly: ^CI28 is UTF-8; ^CI27 is the printable
/// Latin-1 subset of Windows-1252. ^FH consumes byte pairs, not codepoints.
/// Zebra Programming Guide ^CI pp. 156–159 and ^FH p. 190.
/// https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
pub fn encode_field(text: &str, encoding: u8) -> Result<String, String> {
    let bytes: Vec<u8> = match encoding {
        28 => text.as_bytes().to_vec(),
        27 => text
            .chars()
            .map(|c| u8::try_from(c as u32).map_err(|_| "CI27 requires Latin-1".to_string()))
            .collect::<Result<_, _>>()?,
        0 | 13 if text.is_ascii() => text.as_bytes().to_vec(),
        _ => return Err("unsupported sampling encoding or character".into()),
    };
    Ok(bytes.iter().map(|b| format!("_{b:02X}")).collect())
}

/// Glyph field bytes are hex-escaped, including all ZPL syntax characters.
pub fn page_plan(codes: &[u32], s: Settings, encoding: u8) -> Result<Page, String> {
    page_plan_with_columns(codes, s, encoding, 2)
}

/// Plan one or two columns without resizing the returned printer image.
/// A single column keeps wide-glyph probes within a narrower printer canvas;
/// callers must still check the planned width against their device's limit.
pub fn page_plan_with_columns(
    codes: &[u32],
    s: Settings,
    encoding: u8,
    columns: u32,
) -> Result<Page, String> {
    s.validate()?;
    if !(1..=2).contains(&columns) {
        return Err("sampling columns must be one or two".into());
    }
    if codes.is_empty() || codes.len() > 16 || codes.iter().any(|c| !valid_codepoint(*c)) {
        return Err("sample must contain 1..16 printable Unicode glyphs".into());
    }
    let cw = s.height.max(s.width) * 6 + 32;
    let ch = s.height * 5 + 48;
    let width = (cw * columns).div_ceil(64) * 64;
    let height = (codes.len() as u32 + 1).div_ceil(columns) * ch;
    let mut zpl = format!(
        "^XA^PW{width}^LL{height}^LH0,0^LS0^LT0^PON^LRN^CI{encoding}{}",
        if encoding == 28 { "^PA0,0,0,0" } else { "" }
    );
    let command = field_command(s.font);
    let sentinel = if s.font == 'S' { "_41" } else { "_7C" };
    let mut tiles = Vec::new();
    for (i, code) in std::iter::once(None)
        .chain(codes.iter().copied().map(Some))
        .enumerate()
    {
        let (x, y) = (i as u32 % columns * cw, i as u32 / columns * ch);
        let base = y + 16 + s.height * 2;
        let second = y + 32 + s.height * 4;
        let encoded = code
            .map(|c| encode_field(&char::from_u32(c).unwrap().to_string(), encoding))
            .transpose()?
            .unwrap_or_default();
        if code.is_some() {
            write!(
                zpl,
                "\n^FT{},{}^{}N,{},{}^FH^FD{}^FS",
                x + 16,
                base,
                command,
                s.height,
                s.width,
                encoded
            )
            .unwrap();
        }
        write!(
            zpl,
            "\n^FT{},{}^{}N,{},{}^FH^FD{sentinel}{}{sentinel}^FS",
            x + 16,
            second,
            command,
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
        }
        // A validated, positive advance can belong to a blank glyph: ZD621
        // resident H lowercase is blank. The nonempty sentinel probes above
        // still reject blank or invalid captures (tests/fixtures/font-h-blank).
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
    encoding: u8,
) -> Result<(String, Raster), String> {
    s.validate()?;
    validate_glyphs(glyphs)?;
    if text.is_empty() || text.len() > 4096 {
        return Err("invalid verification text".into());
    }
    let selected: Vec<_> = text
        .chars()
        .map(|c| c as u32)
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
    let encoded = encode_field(text, encoding)?;
    let command = field_command(s.font);
    let advanced = if encoding == 28 { "^PA0,0,0,0" } else { "" };
    let zpl=format!("^XA^PW{width}^LL{height}^LH0,0^LS0^LT0^PON^LRN^CI{encoding}{advanced}^FT16,{baseline}^{}N,{},{}^FH^FD{encoded}^FS^XZ",command,s.height,s.width);
    Ok((zpl, image))
}

/// Compact ZBF1 (byte) or ZBF2 (Unicode) strike: settings and glyph metrics followed by contiguous
/// MSB-first bitmap bits (no per-row padding). Integers are little endian.
pub fn pack(glyphs: &[Glyph], settings: Settings) -> Result<Vec<u8>, String> {
    settings.validate()?;
    validate_glyphs(glyphs)?;
    if glyphs.is_empty()
        || glyphs.iter().any(|g| !valid_codepoint(g.codepoint))
        || glyphs.windows(2).any(|g| g[0].codepoint >= g[1].codepoint)
    {
        return Err("strike glyphs must be sorted, unique printable Unicode scalars".into());
    }
    let unicode = glyphs.iter().any(|g| g.codepoint > 255);
    let mut out = if unicode { b"ZBF2" } else { b"ZBF1" }.to_vec();
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
        if unicode {
            out.extend(g.codepoint.to_le_bytes());
        } else {
            out.push(g.codepoint as u8);
        }
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
#[cfg(test)]
mod packed_tests {
    use super::*;
    use zpl::bitmap_font::unpack;
    #[test]
    fn latin1_strikes_round_trip_and_use_windows1252_field_bytes() {
        let s = Settings {
            font: '0',
            height: 32,
            width: 0,
            dpi: 203,
        };
        let g = Glyph {
            codepoint: 233,
            advance: 2,
            left: 0,
            top: -1,
            width: 1,
            height: 1,
            bitmap: vec![vec![128]],
        };
        let packed = pack(std::slice::from_ref(&g), s).unwrap();
        assert_eq!(unpack(&packed).unwrap().1, vec![g.clone()]);
        let (request, _) = verification_plan(&[g], s, "éé", 27).unwrap();
        assert!(request.contains("^CI27"));
        assert!(request.contains("^FD_E9_E9"));
        assert!(page_plan(&[233], s, 27).unwrap().zpl.contains("^FD_E9"));
        assert!(page_plan(&[127], s, 27).is_err());
        assert!(page_plan(&[159], s, 27).is_err());
    }
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
    fn unicode_strikes_round_trip_and_emit_utf8_bytes() {
        let s = Settings {
            font: '0',
            height: 40,
            width: 24,
            dpi: 203,
        };
        let glyph = Glyph {
            codepoint: 'א' as u32,
            advance: 12,
            left: 1,
            top: -20,
            width: 1,
            height: 1,
            bitmap: vec![vec![128]],
        };
        let packed = pack(std::slice::from_ref(&glyph), s).unwrap();
        assert_eq!(&packed[..4], b"ZBF2");
        assert_eq!(
            zpl::bitmap_font::unpack(&packed).unwrap().1,
            vec![glyph.clone()]
        );
        let page = page_plan(&['א' as u32, '😀' as u32], s, 28).unwrap();
        assert!(page.zpl.contains("^CI28^PA0,0,0,0"));
        assert!(page.zpl.contains("^FD_D7_90"));
        assert!(page.zpl.contains("^FD_F0_9F_98_80"));
        let (verification, _) = verification_plan(&[glyph], s, "אא", 28).unwrap();
        assert!(verification.contains("^FD_D7_90_D7_90"));
        assert!(page_plan(&[0xd800], s, 28).is_err());
        assert!(page_plan(&[0x110000], s, 28).is_err());
        assert!(page_plan(&['א' as u32], s, 27).is_err());
        let mut invalid = packed;
        invalid[13..17].copy_from_slice(&0xd800u32.to_le_bytes());
        assert!(zpl::bitmap_font::unpack(&invalid).is_err());
    }
    #[test]
    fn legacy_strikes_repack_without_changing_bytes() {
        let original = include_bytes!("../../zpl/assets/font0-32.zbf");
        let (settings, glyphs) = zpl::bitmap_font::unpack(original).unwrap();
        assert_eq!(pack(&glyphs, settings).unwrap(), original);
    }
    #[test]
    fn graphic_symbols_use_a_visible_registered_symbol_probe() {
        let settings = Settings {
            font: 'S',
            height: 24,
            width: 24,
            dpi: 203,
        };
        let page = page_plan(&(64..=71).collect::<Vec<_>>(), settings, 27).unwrap();
        // ^GS p. 217: A-E are symbols; the usual vertical-bar probe is blank.
        assert_eq!(
            page.zpl.as_bytes(),
            include_bytes!("../tests/fixtures/graphic-symbols/page-004.zpl")
        );
        let raster = Raster::decode_png(include_bytes!(
            "../tests/fixtures/graphic-symbols/page-004.png"
        ))
        .unwrap();
        let glyphs = extract_page(&raster, &page).unwrap();
        assert!(glyphs.iter().all(|g| g.advance == 26));
        assert_eq!(
            glyphs
                .iter()
                .filter(|g| !g.bitmap.is_empty())
                .map(|g| g.codepoint)
                .collect::<Vec<_>>(),
            b"ABCDE".map(u32::from)
        );
        assert!(!page.zpl.contains("^AS"));
    }

    #[test]
    fn printer_blank_glyphs_keep_their_measured_advances() {
        let settings = Settings {
            font: 'H',
            height: 21,
            width: 13,
            dpi: 203,
        };
        let page = page_plan(&(96..=103).collect::<Vec<_>>(), settings, 27).unwrap();
        assert_eq!(
            page.zpl.as_bytes(),
            include_bytes!("../tests/fixtures/font-h-blank/page-008.zpl")
        );
        let mut raster = Raster::decode_png(include_bytes!(
            "../tests/fixtures/font-h-blank/page-008.png"
        ))
        .unwrap();
        let glyphs = extract_page(&raster, &page).unwrap();
        assert_eq!(glyphs.len(), 8);
        assert!(glyphs[0].width > 0);
        for glyph in &glyphs[1..] {
            assert_eq!(glyph.advance, 19);
            assert_eq!((glyph.width, glyph.height), (0, 0));
            assert!(glyph.bitmap.is_empty());
        }
        let (_, decoded) = zpl::bitmap_font::unpack(&pack(&glyphs, settings).unwrap()).unwrap();
        assert_eq!(decoded, glyphs);
        raster.pixels.fill(255);
        assert!(extract_page(&raster, &page)
            .unwrap_err()
            .contains("empty advance probe"));
    }

    #[test]
    fn large_underscore_stays_in_the_isolated_glyph_tile() {
        let settings = super::Settings {
            font: '0',
            height: 64,
            width: 0,
            dpi: 203,
        };
        let page = super::page_plan(&(88..=95).collect::<Vec<_>>(), settings, 27).unwrap();
        assert_eq!(
            page.zpl.as_bytes(),
            include_bytes!("../tests/fixtures/font64/page-007.zpl")
        );
        let raster =
            super::Raster::decode_png(include_bytes!("../tests/fixtures/font64/page-007.png"))
                .unwrap();
        let glyphs = super::extract_page(&raster, &page).unwrap();
        let underscore = glyphs
            .iter()
            .find(|g| g.codepoint == u32::from(b'_'))
            .unwrap();
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
        let p = page_plan(&[32, 65, 106], settings(), 27).unwrap();
        let mut r = Raster {
            width: p.width,
            height: p.height,
            pixels: vec![255; (p.width * p.height) as usize],
        };
        for t in &p.tiles {
            let text = t.code.map(|c| vec![c as u8]).unwrap_or_default();
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
        let (zpl, r) = verification_plan(&g, settings(), "A j", 27).unwrap();
        assert!(zpl.contains("^FD_41_20_6A^FS"));
        assert_eq!(r.pixels.iter().filter(|&&v| v == 0).count(), 14);
        assert_eq!(r.pixels[28 * r.width as usize + 18], 0);
        assert_eq!(r.pixels[33 * r.width as usize + 26], 0);
        let p = page_plan(&[94, 126, 95], settings(), 27).unwrap();
        for s in ["_5E", "_7E", "_5F"] {
            assert!(p.zpl.contains(&format!("^FD{s}^FS")))
        }
        assert!(verification_plan(&g, settings(), "missing", 27).is_err());
    }
    #[test]
    fn wide_glyph_page_can_fit_an_832_dot_canvas() {
        // ZD621 preview evidence: a requested PW896 page returned 832 dots.
        // Keep the two-column default, but permit a narrower capture layout.
        let settings = Settings {
            font: '0',
            height: 65,
            width: 0,
            dpi: 203,
        };
        let wide = page_plan(&[65], settings, 27).unwrap();
        let narrow = page_plan_with_columns(&[65], settings, 27, 1).unwrap();
        assert_eq!((wide.width, narrow.width), (896, 448));
        assert_eq!(narrow.tiles.len(), wide.tiles.len());
        assert!(narrow
            .tiles
            .iter()
            .all(|tile| tile.x + tile.width <= narrow.width));
        for columns in [0, 3] {
            assert!(page_plan_with_columns(&[65], settings, 27, columns).is_err());
        }
    }
    #[test]
    fn invalid_samples_and_clipping() {
        assert!(page_plan(&[], settings(), 27).is_err());
        assert!(page_plan(&[159], settings(), 27).is_err());
        assert!(page_plan(&[255], settings(), 27).is_ok());
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
