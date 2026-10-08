//! Migration contract: reconstruct every historical capture exactly, including
//! blank advances, bearings and original dense-row padding. Hashes originate in
//! the ZD621 203-dpi V93.21.33Z corpus, not regenerated renderer screenshots.
#[path = "support/compact_font.rs"]
mod compact_font;
#[path = "support/digest.rs"]
mod digest;

#[test]
fn all_captures_preserve_original_bytes() {
    let captures = zpl_bitmap_fonts::captures::zd621::CAPTURES;
    assert_eq!(captures.len(), 101);
    let mut glyph_count = 0;
    for c in captures {
        let bytes = compact_font::asset(c.name).unwrap();
        assert_eq!(digest::sha256(&bytes), c.sha256, "{}", c.name);
        let (_, glyphs) = compact_font::decoded(c.name).unwrap();
        zpl::bitmap_font::validate_glyphs(&glyphs).unwrap();
        glyph_count += glyphs.len();
        assert_eq!(glyphs.len(), c.strike.keys.len());
        assert!(c.strike.glyph(0x10ffff).is_none());
        for g in glyphs {
            let compact = c.strike.glyph(g.codepoint).unwrap();
            assert_eq!(compact.advance as u32, g.advance);
            assert_eq!((compact.left as i32, compact.top as i32), (g.left, g.top));
            for y in 0..g.height {
                for x in 0..g.width {
                    assert_eq!(
                        compact.pixel(x as u16, y as u16),
                        g.bitmap[y as usize][x as usize / 8] & (128 >> (x % 8)) != 0
                    );
                }
            }
        }
    }
    assert_eq!(glyph_count, 5710);
}
