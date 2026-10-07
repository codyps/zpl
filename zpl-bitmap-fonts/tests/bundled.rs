#![cfg(feature = "zd621")]
// Golden FNV-1a computed independently from the verified fonts.json.
// Covers every name, metric, missing key, bearing, advance and individual pixel;
// excludes storage layout so deduplication can evolve without changing the font.
#[test]
fn bundled_fonts_match_verified_json_pixels_and_metrics() {
    let mut digest = 0xcbf29ce484222325u64;
    let mut add = |byte: u8| {
        digest = (digest ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    };
    let fonts = zpl_bitmap_fonts::zd621::FONTS
        .iter()
        .filter(|f| f.metrics.is_some())
        .collect::<Vec<_>>();
    assert_eq!(fonts.len(), 45);
    for font in fonts {
        for b in font.name.bytes().chain([0]) {
            add(b);
        }
        let m = font.cell_metrics().unwrap();
        for v in [m.cell_height, m.cell_width, m.baseline, m.space_advance] {
            for b in v.to_le_bytes() {
                add(b);
            }
        }
        for key in 0..=255 {
            if let Some(g) = font.glyph(key) {
                add(1);
                for b in [
                    u8::try_from(g.advance).unwrap(),
                    i8::try_from(g.left).unwrap() as u8,
                    i8::try_from(g.top).unwrap() as u8,
                    u8::try_from(g.width).unwrap(),
                    u8::try_from(g.height).unwrap(),
                ] {
                    add(b);
                }
                for y in 0..g.height {
                    for x in 0..g.width {
                        add(u8::from(g.pixel(x, y)));
                    }
                }
            } else {
                add(0);
            }
        }
    }
    assert_eq!(digest, 0x6e852aa299dfb9b8);
}
