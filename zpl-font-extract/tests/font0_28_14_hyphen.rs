//! Native sampling evidence and guide references live in the fixture README.
use std::{fs, path::Path};
use zpl::{bitmap_font::Settings, output::raster::Raster};
#[test]
fn font0_28_14_hyphen_reproduces_from_native_pages() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../zpl/tests/fixtures/font0-28-14-hyphen-zd621-v1");
    let (font, height, width) = ('0', 28, 14);
    let settings = Settings {
        font,
        height,
        width,
        dpi: 203,
    };
    let source = root.join("font-source");
    let mut glyphs = Vec::new();
    for (page, codes) in [0xad, 0xf0].chunks(8).enumerate() {
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
    assert_eq!(glyphs.len(), 2);
    let packed = zpl_font_extract::pack(&glyphs, settings).unwrap();
    assert_eq!(
        packed,
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../zpl/assets/font{font}-{height}-{width}-hyphen.zbf"
        )))
        .unwrap()
    );
    let (zpl, expected) =
        zpl_font_extract::verification_plan(&glyphs, settings, "\u{ad}\u{f0}\u{ad}", 27).unwrap();
    assert_eq!(
        zpl.as_bytes(),
        fs::read(source.join("verification.zpl")).unwrap()
    );
    let reference =
        Raster::decode_png(&fs::read(source.join("verification.png")).unwrap()).unwrap();
    assert_eq!(expected, reference);
}
