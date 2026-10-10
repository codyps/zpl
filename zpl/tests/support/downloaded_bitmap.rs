//! Shared setup for the saved downloaded-bitmap campaign.
//! See fixtures/downloaded-bitmap-zd621-v1/README.md for capture provenance.
use zpl::{
    bitmap_font::{Glyph, Settings},
    fonts::Fonts,
};

pub fn fonts() -> Fonts<'static> {
    // Preserve the exact saved submissions, including their unused font-0 alias.
    // These fixtures use bitmap text and automatic captions. A sentinel makes any
    // accidental font-0 text an error instead of importing an unrelated TTF.
    let mut fonts = Fonts::new();
    fonts
        .insert_named_bitmap(
            "R:FC0.TTF",
            Settings {
                font: '0',
                width: 1,
                height: 1,
                dpi: 203,
            },
            vec![Glyph {
                codepoint: 0x10ffff,
                advance: 1,
                left: 0,
                top: 0,
                width: 0,
                height: 0,
                bitmap: vec![],
            }],
            0.,
        )
        .unwrap();
    fonts
}
