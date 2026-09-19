//! ZD621 preview regressions. Raw captures and provenance are in the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn printer_controls_are_pixel_exact() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/accuracy-refinements-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/accuracy-refinements-zd621-v1/manifest.tsv")
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
        let diff = raster_diff::compare(&reference, &actual, false).unwrap();
        assert!(
            diff.matches(),
            "{}: under {} over {}",
            c[0],
            diff.reference_only,
            diff.candidate_only
        );
        count += 1;
    }
    assert_eq!(count, 19);
}
