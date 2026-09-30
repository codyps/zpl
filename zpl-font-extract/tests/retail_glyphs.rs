//! Native retail glyph extraction and independent composition verification.
use std::{fs, path::Path};
use zpl::{bitmap_font::Settings, output::raster::Raster};
#[test]
fn retail_supplements_reproduce_from_native_samples() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../zpl/tests/fixtures/retail-font-zd621-v1");
    for (height, width) in [(25, 14), (32, 0)] {
        let settings = Settings {
            font: '0',
            height,
            width,
            dpi: 203,
        };
        let directory = root.join(format!("font-{height}-{width}"));
        let plan = zpl_font_extract::page_plan(&[32, 183, 192, 194, 9516], settings, 28).unwrap();
        assert_eq!(
            plan.zpl.as_bytes(),
            fs::read(directory.join("page-000.zpl")).unwrap()
        );
        let image = Raster::decode_png(&fs::read(directory.join("page-000.png")).unwrap()).unwrap();
        let glyphs = zpl_font_extract::extract_page(&image, &plan).unwrap();
        let packed = zpl_font_extract::pack(&glyphs, settings).unwrap();
        assert_eq!(
            packed,
            fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../zpl/assets/font0-{height}-{width}-retail.zbf"))
            )
            .unwrap()
        );
        let (source, expected) =
            zpl_font_extract::verification_plan(&glyphs, settings, "Â· À┬", 28).unwrap();
        assert_eq!(
            source.as_bytes(),
            fs::read(directory.join("verification.zpl")).unwrap()
        );
        assert_eq!(
            expected,
            Raster::decode_png(&fs::read(directory.join("verification.png")).unwrap()).unwrap()
        );
    }
}
