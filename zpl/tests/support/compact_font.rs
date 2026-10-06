//! Reconstruct historical strike encodings from the compact reader. Original
//! fixture hashes pin every glyph without retaining duplicate embedded assets.
pub fn asset(name: &str) -> Option<Vec<u8>> {
    let face = name.strip_prefix("font")?.split('-').next()?;
    let id = if face == "GS" {
        '@'
    } else if face.len() == 1 {
        face.chars().next()?
    } else {
        return None;
    };
    let font = zpl_bitmap_fonts::resident(id)?;
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
    let mut bytes = if unicode { b"ZBF2" } else { b"ZBF1" }.to_vec();
    bytes.push(id as u8);
    for n in [
        font.metrics.cell_height,
        font.metrics.cell_width,
        203,
        keys.len() as u16,
    ] {
        bytes.extend(n.to_le_bytes());
    }
    for (key, source) in keys {
        let g = font.glyph(source).unwrap();
        if unicode {
            bytes.extend(key.to_le_bytes());
        } else {
            bytes.push(key as u8);
        }
        for n in [
            u16::from(g.advance),
            i16::from(g.left) as u16,
            i16::from(g.top) as u16,
            u16::from(g.width),
            u16::from(g.height),
        ] {
            bytes.extend(n.to_le_bytes());
        }
        let mut bits = vec![0; (usize::from(g.width) * usize::from(g.height)).div_ceil(8)];
        for y in 0..g.height {
            for x in 0..g.width {
                if g.pixel(x, y) {
                    let bit = usize::from(y) * usize::from(g.width) + usize::from(x);
                    bits[bit / 8] |= 128 >> (bit % 8);
                }
            }
        }
        bytes.extend(bits);
    }
    Some(bytes)
}
