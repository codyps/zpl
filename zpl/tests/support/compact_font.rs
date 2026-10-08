//! Read compact glyphs directly for tests. The historical encoder below exists
//! only to check original capture hashes; production has no ZBF1/ZBF2 codec.
use zpl::bitmap_font::{Glyph, Settings};

pub fn decoded(name: &str) -> Option<(Settings, Vec<Glyph>)> {
    if let Some(c) = zpl_bitmap_fonts::captures::zd621::CAPTURES
        .iter()
        .find(|c| c.name == name)
    {
        let s = c.strike;
        return Some((
            Settings {
                font: s.font,
                height: s.height.into(),
                width: s.width.into(),
                dpi: s.dpi.into(),
            },
            s.keys
                .iter()
                .map(|&key| owned_glyph(key, s.glyph(key).unwrap()))
                .collect(),
        ));
    }
    let face = name.strip_prefix("font")?.split('-').next()?;
    let id = if face == "GS" {
        '@'
    } else if face.len() == 1 {
        face.chars().next()?
    } else {
        return None;
    };
    let font = zpl_bitmap_fonts::resident(id)?;
    let metrics = font.cell_metrics()?;
    let unicode = name.ends_with("-legacy-controls.zbf");
    let keys: Vec<(u32, u8)> = if name.ends_with("-cent.zbf") {
        vec![(162, 189)]
    } else if name.ends_with("-hyphen.zbf") {
        vec![(173, 240), (240, 208)]
    } else if name.ends_with("-legacy-backslash.zbf") {
        vec![(92, 92)]
    } else if unicode {
        vec![(0x2190, 27), (0x2302, 127)]
    } else {
        (32..=126u8)
            .map(|key| {
                // CI27 backslash uses source slot 31; OCR/GS positions are blanks.
                let source = if key == b'\\' && matches!(id, 'A' | 'B' | 'C' | 'D' | 'F' | 'G') {
                    31
                } else {
                    key
                };
                (u32::from(key), source)
            })
            .collect()
    };
    Some((
        Settings {
            font: id,
            height: metrics.cell_height.into(),
            width: metrics.cell_width.into(),
            dpi: 203,
        },
        keys.into_iter()
            .map(|(key, source)| owned_glyph(key, font.glyph(source).unwrap()))
            .collect(),
    ))
}

fn owned_glyph(codepoint: u32, g: zpl_bitmap_fonts::Glyph) -> Glyph {
    let mut bitmap = vec![vec![0; usize::from(g.width).div_ceil(8)]; usize::from(g.height)];
    for y in 0..g.height {
        for x in 0..g.width {
            if g.pixel(x, y) {
                bitmap[usize::from(y)][usize::from(x) / 8] |= 128 >> (x % 8);
            }
        }
    }
    Glyph {
        codepoint,
        advance: g.advance.into(),
        left: g.left.into(),
        top: g.top.into(),
        width: g.width.into(),
        height: g.height.into(),
        bitmap,
    }
}

// Historical layout is documented in zpl/assets/README.md. Preserve the original
// byte stream (including dense row packing) solely for provenance assertions.
#[allow(dead_code)] // Some test binaries need only decoded glyphs.
pub fn asset(name: &str) -> Option<Vec<u8>> {
    let (s, glyphs) = decoded(name)?;
    let unicode = zpl_bitmap_fonts::captures::zd621::CAPTURES
        .iter()
        .find(|c| c.name == name)
        .map_or_else(
            || name.ends_with("-legacy-controls.zbf"),
            |c| c.legacy_format == 2,
        );
    let mut bytes = if unicode { b"ZBF2" } else { b"ZBF1" }.to_vec();
    bytes.push(s.font as u8);
    for value in [
        s.height as u16,
        s.width as u16,
        s.dpi as u16,
        glyphs.len() as u16,
    ] {
        bytes.extend(value.to_le_bytes());
    }
    for g in glyphs {
        if unicode {
            bytes.extend(g.codepoint.to_le_bytes());
        } else {
            bytes.push(g.codepoint as u8);
        }
        for value in [
            g.advance as u16,
            g.left as u16,
            g.top as u16,
            g.width as u16,
            g.height as u16,
        ] {
            bytes.extend(value.to_le_bytes());
        }
        let mut bits = vec![0; (g.width as usize * g.height as usize).div_ceil(8)];
        for y in 0..g.height as usize {
            for x in 0..g.width as usize {
                if g.bitmap[y][x / 8] & (128 >> (x % 8)) != 0 {
                    let bit = y * g.width as usize + x;
                    bits[bit / 8] |= 128 >> (bit % 8);
                }
            }
        }
        bytes.extend(bits);
    }
    Some(bytes)
}
