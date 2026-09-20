//! Native sampling evidence and guide references live in the fixture README.
use std::{fs, path::Path};
use zpl::{bitmap_font::Settings, output::raster::Raster};
#[test]
fn dimension_font0_strikes_reproduce_from_native_pages() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../zpl/tests/fixtures/font0-dimensions-zd621-v1");
    for (font, height, width) in [
        ('0', 15, 0),
        ('0', 17, 0),
        ('0', 31, 0),
        ('0', 33, 0),
        ('0', 63, 0),
        ('0', 65, 0),
        ('0', 64, 16),
        ('0', 16, 64),
        ('0', 96, 96),
    ] {
        let settings = Settings {
            font,
            height,
            width,
            dpi: 203,
        };
        let source = root.join(format!("font-{height}-{width}"));
        let mut glyphs = Vec::new();
        for (page, codes) in (32..=126).collect::<Vec<u32>>().chunks(8).enumerate() {
            let plan = zpl_font_extract::page_plan_with_columns(
                codes,
                settings,
                27,
                if matches!(height, 65 | 96) { 1 } else { 2 },
            )
            .unwrap();
            let name = format!("page-{page:03}");
            assert_eq!(
                plan.zpl.as_bytes(),
                fs::read(source.join(format!("{name}.zpl"))).unwrap()
            );
            let image =
                Raster::decode_png(&fs::read(source.join(format!("{name}.png"))).unwrap()).unwrap();
            glyphs.extend(zpl_font_extract::extract_page(&image, &plan).unwrap());
        }
        assert_eq!(glyphs.len(), 95);
        let packed = zpl_font_extract::pack(&glyphs, settings).unwrap();
        assert_eq!(
            packed,
            fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../zpl/assets/font{font}-{height}-{width}.zbf"))
            )
            .unwrap()
        );
        let (zpl, expected) = zpl_font_extract::verification_plan(
            &glyphs,
            settings,
            if height == 96 {
                "Hg0j"
            } else {
                "Hgypqj 0123 ABCxyz"
            },
            27,
        )
        .unwrap();
        assert_eq!(
            zpl.as_bytes(),
            fs::read(source.join("verification.zpl")).unwrap()
        );
        let reference =
            Raster::decode_png(&fs::read(source.join("verification.png")).unwrap()).unwrap();
        assert_eq!(expected, reference);
    }
}
