//! Reproduce the 28-dot strike from native sampling, with independent text.
//! Zebra ^A/^FT: https://docs.zebra.com/us/en/printers/software/zpl-pg.html
use std::{fs, path::Path};
use zpl::{bitmap_font::Settings, output::raster::Raster};

#[test]
fn labelixa_font_reproduces_from_native_pages() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../zpl/tests/fixtures/qr-segmentation-zd621-v1/font-0-28");
    let settings = Settings {
        font: '0',
        height: 28,
        width: 0,
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
        include_bytes!("../../zpl/assets/font0-28-0.zbf")
    );
    let (source, expected) =
        zpl_font_extract::verification_plan(&glyphs, settings, "AVATAR Agj Wavy 123 _^~|!", 27)
            .unwrap();
    assert_eq!(
        source.as_bytes(),
        fs::read(root.join("verification.zpl")).unwrap()
    );
    let reference = Raster::decode_png(&fs::read(root.join("verification.png")).unwrap()).unwrap();
    assert_eq!(expected, reference);
}
