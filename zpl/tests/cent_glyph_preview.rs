//! ZD621 preview regressions. Raw captures and provenance are in the fixture README.
#[path = "support/compact_font.rs"]
mod compact_font;
use std::{fs, path::Path};
#[path = "support/digest.rs"]
mod digest;
#[test]
fn printer_controls_pin_every_painted_pixel() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cent-glyph-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/cent-glyph-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let input = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&input), c[1], "{} input", c[0]);
        assert_eq!(digest::sha256(&png), c[2], "{} printer capture", c[0]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&input, zpl::render::profiles::ZD621_203_DPI).unwrap();
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert!(diff.matches(), "{} must match the printer exactly", c[0]);
        assert_eq!(
            diff.reference_only,
            c[3].parse::<usize>().unwrap(),
            "{} underpaint",
            c[0]
        );
        assert_eq!(
            diff.candidate_only,
            c[4].parse::<usize>().unwrap(),
            "{} overpaint",
            c[0]
        );
        assert_eq!(
            digest::sha256(&actual.pixels),
            c[5],
            "{} local pixels",
            c[0]
        );
        if matches!(c[0], "E-28-15/verification" | "H-21-13/verification") {
            assert_eq!(diff.both_black, 0, "{} blank glyphs", c[0]);
        } else {
            assert!(diff.both_black > 0, "{} positive control", c[0]);
        }
        count += 1;
    }
    assert_eq!(count, 32);
}

#[test]
fn captured_cent_assets_are_pinned() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut count = 0;
    for row in include_str!("fixtures/cent-glyph-zd621-v1/assets.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let bytes = compact_font::asset(c[0])
            .unwrap_or_else(|| fs::read(root.join("assets").join(c[0])).unwrap());
        assert_eq!(digest::sha256(&bytes), c[1], "{} asset", c[0]);
        let (settings, glyphs) = zpl::bitmap_font::unpack(&bytes).unwrap();
        assert_eq!(glyphs.len(), 1);
        assert_eq!(glyphs[0].codepoint, 162);
        assert!(glyphs[0].advance > 0);
        assert_eq!(
            glyphs[0].bitmap.is_empty(),
            matches!(settings.font, 'E' | 'H')
        );
        count += 1;
    }
    assert_eq!(count, 15);
}
