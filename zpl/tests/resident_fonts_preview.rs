//! Captured resident font strikes and independent text/caption controls.
//! See fixtures/resident-fonts-zd621-v1/README.md for capture provenance.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn captured_strikes_and_interpretation_match_printer_pixels() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/resident-fonts-zd621-v1");
    let mut failures = Vec::new();
    let mut count = 0;
    for row in include_str!("fixtures/resident-fonts-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let source = root.join(c[0]);
        let input = fs::read(&source).unwrap();
        let png = fs::read(source.with_extension("png")).unwrap();
        assert_eq!(digest::sha256(&input), c[1], "{} source", c[0]);
        assert_eq!(digest::sha256(&png), c[2], "{} capture", c[0]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&input, zpl::render::profiles::ZD621_203_DPI).unwrap();
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        if !diff.matches() {
            failures.push(format!(
                "{}: under {} over {}",
                c[0], diff.reference_only, diff.candidate_only
            ));
        }
        count += 1;
    }
    assert_eq!(count, 118);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn justified_rotations_pin_residuals_and_exceed_text_accuracy_floor() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/resident-fonts-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/resident-fonts-zd621-v1/justification.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let source = root.join(c[0]);
        let input = fs::read(&source).unwrap();
        let png = fs::read(source.with_extension("png")).unwrap();
        assert_eq!(digest::sha256(&input), c[1]);
        assert_eq!(digest::sha256(&png), c[2]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&input, zpl::render::profiles::ZD621_203_DPI).unwrap();
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (c[3].parse().unwrap(), c[4].parse().unwrap()),
            "{}",
            c[0]
        );
        assert_eq!(digest::sha256(&actual.pixels), c[5]);
        // Ink IoU, not whole-canvas agreement: whitespace cannot hide errors.
        assert!(diff.ink_iou() >= 0.8, "{}", c[0]);
        count += 1;
    }
    assert_eq!(count, 34);
}
