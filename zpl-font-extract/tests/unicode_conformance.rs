//! Native CI28 sampling; provenance and standards are in the fixture README.
use std::{collections::BTreeMap, fs, path::Path};
use zpl::{bitmap_font::Settings, output::raster::Raster};
#[test]
fn unicode_and_zero_advance_strikes_reproduce_from_native_pages() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../zpl/tests/fixtures/unicode-conformance-zd621-v1");
    let settings = Settings {
        font: '0',
        height: 40,
        width: 24,
        dpi: 203,
    };
    let mut merged = BTreeMap::new();
    for (directory, codes, asset) in [
        (
            "unicode-conformance",
            &[
                32u32, 48, 49, 50, 51, 65, 66, 67, 68, 72, 97, 102, 103, 106, 109, 110, 112, 113,
                114, 115, 116, 121, 160, 163, 197, 233, 246, 917, 937, 940, 951, 953, 954, 955,
                957, 1046, 1055, 1074, 1077, 1080, 1088, 1090, 8364, 20013, 25991, 26085, 26412,
                35486, 44397, 50612, 54620, 128512,
            ] as &[u32],
            include_bytes!("../../zpl/assets/font0-40-24-conformance.zbf") as &[u8],
        ),
        (
            "zero-advance",
            &[32u32, 65, 101, 173, 769, 778, 8203] as &[u32],
            include_bytes!("../../zpl/assets/font0-40-24-controls.zbf") as &[u8],
        ),
    ] {
        let mut glyphs = Vec::new();
        for (page, codes) in codes.chunks(8).enumerate() {
            let plan = zpl_font_extract::page_plan(codes, settings, 28).unwrap();
            let base = root.join(directory).join(format!("page-{page:03}"));
            assert_eq!(
                plan.zpl.as_bytes(),
                fs::read(base.with_extension("zpl")).unwrap()
            );
            let image = Raster::decode_png(&fs::read(base.with_extension("png")).unwrap()).unwrap();
            glyphs.extend(zpl_font_extract::extract_page(&image, &plan).unwrap());
            let mut blank = image;
            blank.pixels.fill(255);
            assert!(zpl_font_extract::extract_page(&blank, &plan).is_err());
        }
        assert_eq!(zpl_font_extract::pack(&glyphs, settings).unwrap(), asset);
        for glyph in glyphs {
            if directory == "zero-advance" && matches!(glyph.codepoint, 173 | 8203) {
                assert_eq!(glyph.advance, 0);
                assert_eq!(glyph.width * glyph.height, 0);
            }
            if let Some(previous) = merged.insert(glyph.codepoint, glyph.clone()) {
                assert_eq!(previous, glyph);
            }
        }
    }
    let glyphs: Vec<_> = merged.into_values().collect();
    for (directory, text) in [
        ("unicode-conformance", "Aé£€ÅΩЖ中😀"),
        ("zero-advance", "Ae\u{301} A\u{30a} A\u{200b}e\u{ad}"),
    ] {
        let (source, expected) =
            zpl_font_extract::verification_plan(&glyphs, settings, text, 28).unwrap();
        let base = root.join(directory).join("verification");
        assert_eq!(
            source.as_bytes(),
            fs::read(base.with_extension("zpl")).unwrap()
        );
        assert_eq!(
            expected,
            Raster::decode_png(&fs::read(base.with_extension("png")).unwrap()).unwrap()
        );
    }
}
