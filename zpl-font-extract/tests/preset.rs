//! Native sampling evidence and guide references live in the fixture README.
use std::{fs, path::Path};
use zpl::{bitmap_font::Settings, output::raster::Raster};
#[test]
fn preset_and_comparison_strikes_reproduce_from_native_pages() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../zpl/tests/fixtures/resident-p-zd621-v1");
    for (font, height, width) in [('P', 20, 18), ('P', 40, 18), ('P', 40, 36), ('0', 20, 18)] {
        let settings = Settings {
            font,
            height,
            width,
            dpi: 203,
        };
        let source = root.join(format!("font-{font}-{height}-{width}"));
        let mut glyphs = Vec::new();
        for (page, codes) in (32..=126).collect::<Vec<u32>>().chunks(8).enumerate() {
            let plan = zpl_font_extract::page_plan(codes, settings, 27).unwrap();
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
        let (zpl, expected) =
            zpl_font_extract::verification_plan(&glyphs, settings, "Hgypqj 0123 ABCxyz", 27)
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
