//! ZD621 preview regressions. Raw captures and provenance are in the fixture README.
use std::{fs, path::Path};
#[path = "support/digest.rs"]
mod digest;
#[test]
fn printer_controls_pin_every_painted_pixel() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/resident-g-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/resident-g-zd621-v1/manifest.tsv")
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
        assert!(
            diff.both_black > 0,
            "{}: positive control must contain ink",
            c[0]
        );
        count += 1;
    }
    assert_eq!(count, 72);
}

#[test]
fn native_font_g_asset_is_pinned_to_the_capture() {
    let asset = include_bytes!("../assets/fontG-60-40.zbf");
    assert_eq!(
        digest::sha256(asset),
        include_str!("fixtures/resident-g-zd621-v1/asset.sha256").trim()
    );
    let (settings, glyphs) = zpl::bitmap_font::unpack(asset).unwrap();
    assert_eq!(
        (settings.font, settings.height, settings.width, settings.dpi),
        ('G', 60, 40, 203)
    );
    assert_eq!(
        glyphs.iter().map(|g| g.codepoint).collect::<Vec<_>>(),
        (32..=126).collect::<Vec<u32>>()
    );
    // ZPL Guide Table 29, p. 1582: 40 matrix dots plus eight gap dots.
    assert!(glyphs.iter().all(|g| g.advance == 48));
}
