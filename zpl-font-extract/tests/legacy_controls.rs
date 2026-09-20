//! Reconstruct the legacy-only ESC/DEL assets from isolated native FO samples.
//! Guide font matrices/baselines pp. 1582–1584; raw source and references in
//! zpl/tests/fixtures/legacy-controls-zd621-v1/README.md.
use std::{fs, path::Path};
use zpl::{
    bitmap_font::{Glyph, Settings},
    output::raster::Raster,
};
fn bounds(
    image: &Raster,
    left: usize,
    top: usize,
    width: usize,
    height: usize,
) -> Option<(usize, usize, usize, usize)> {
    let (mut x0, mut y0, mut x1, mut y1) = (width, height, 0, 0);
    for y in 0..height {
        for x in 0..width {
            if image.pixels[(top + y) * image.width as usize + left + x] == 0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1, y1))
}
#[test]
fn legacy_control_strikes_reconstruct_from_printer_ink_and_advances() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../zpl");
    for (image_name, default_glyph) in [("glyphs", false), ("glyphs-pa", true)] {
        let image = Raster::decode_png(
            &fs::read(root.join(format!(
                "tests/fixtures/legacy-controls-zd621-v1/{image_name}.png"
            )))
            .unwrap(),
        )
        .unwrap();
        assert_eq!((image.width, image.height), (832, 900));
        for (column, font, height, width, baseline) in
            [(0, '0', 32, 0, 24), (1, 'A', 9, 5, 6), (2, '0', 16, 0, 12)]
        {
            let reference = bounds(&image, column * 208, 560, 208, 140).unwrap();
            let mut glyphs = Vec::new();
            for (row, codepoint) in [0x2190, 0x2302].into_iter().enumerate() {
                // Canonical keys are internal to legacy-only faces; this does not
                // claim that CI28 renders Unicode arrow/house identically.
                let probe = bounds(&image, column * 208, (row + 2) * 140, 208, 140).unwrap();
                let advance = probe.2.checked_sub(reference.2).unwrap();
                // The final pipe is translated by exactly the measured advance.
                for y in 0..140 {
                    for x in reference.2 - 1..reference.2 {
                        let a = image.pixels[(560 + y) * 832 + column * 208 + x];
                        let b =
                            image.pixels[((row + 2) * 140 + y) * 832 + column * 208 + x + advance];
                        assert_eq!(a, b);
                    }
                }
                let mut g = Glyph {
                    codepoint,
                    advance: advance as u32,
                    left: 0,
                    top: 0,
                    width: 0,
                    height: 0,
                    bitmap: Vec::new(),
                };
                if let Some((x0, y0, x1, y1)) = bounds(&image, column * 208, row * 140, 208, 140) {
                    g.left = x0 as i32 - 40;
                    g.top = y0 as i32 - 40 - baseline;
                    g.width = (x1 - x0) as u32;
                    g.height = (y1 - y0) as u32;
                    for y in y0..y1 {
                        let mut bits = vec![0; (x1 - x0).div_ceil(8)];
                        for x in x0..x1 {
                            if image.pixels[(row * 140 + y) * 832 + column * 208 + x] == 0 {
                                bits[(x - x0) / 8] |= 128 >> ((x - x0) % 8);
                            }
                        }
                        g.bitmap.push(bits);
                    }
                }
                glyphs.push(g);
            }
            let bytes = zpl_font_extract::pack(
                &glyphs,
                Settings {
                    font,
                    height,
                    width,
                    dpi: 203,
                },
            )
            .unwrap();
            let suffix = if font == '0' && default_glyph {
                "-default"
            } else {
                ""
            };
            assert_eq!(
                bytes,
                fs::read(root.join(format!(
                    "assets/font{font}-{height}-{width}-legacy-controls{suffix}.zbf"
                )))
                .unwrap()
            );
        }
    }
}
