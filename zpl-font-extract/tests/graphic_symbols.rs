//! Native sampling evidence and guide references live in the fixture README.
use std::{fs, path::Path};
use zpl::{bitmap_font::Settings, output::raster::Raster};
#[test]
fn graphic_symbol_strike_reproduces_from_native_pages() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../zpl/tests/fixtures/graphic-symbols-zd621-v1/font-source");
    let settings = Settings {
        font: zpl::bitmap_font::GRAPHIC_SYMBOLS,
        height: 24,
        width: 24,
        dpi: 203,
    };
    let mut glyphs = Vec::new();
    for (page, codes) in (32..=126).collect::<Vec<u32>>().chunks(8).enumerate() {
        let plan = zpl_font_extract::page_plan(codes, settings, 27).unwrap();
        let name = format!("page-{page:03}");
        assert_eq!(
            plan.zpl.as_bytes(),
            fs::read(root.join(format!("{name}.zpl"))).unwrap()
        );
        let image =
            Raster::decode_png(&fs::read(root.join(format!("{name}.png"))).unwrap()).unwrap();
        glyphs.extend(zpl_font_extract::extract_page(&image, &plan).unwrap());
    }
    assert_eq!(glyphs.len(), 95);
    assert_eq!(
        zpl_font_extract::pack(&glyphs, settings).unwrap(),
        include_bytes!("../../zpl/assets/fontGS-24-24.zbf")
    );
    let (source, expected) =
        zpl_font_extract::verification_plan(&glyphs, settings, "ABCDEEDCBA", 27).unwrap();
    assert_eq!(
        source.as_bytes(),
        fs::read(root.join("verification.zpl")).unwrap()
    );
    assert_eq!(
        expected,
        Raster::decode_png(&fs::read(root.join("verification.png")).unwrap()).unwrap()
    );
}
