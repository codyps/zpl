//! ^BR retail layout and input validation against unmodified ZD621 captures.
//! See fixture README for capture provenance and GS1/Zebra references.
use std::{fs, path::Path};
#[path = "support/digest.rs"]
mod digest;

#[test]
fn retail_controls_match_or_reject_invalid_printer_inputs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/databar-retail-zd621-v1");
    let mut counts = (0, 0);
    for row in include_str!("fixtures/databar-retail-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let input = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&input), c[1], "{} input", c[0]);
        assert_eq!(digest::sha256(&png), c[2], "{} capture", c[0]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&input, zpl::render::profiles::ZD621_203_DPI);
        match c[3] {
            "exact" => {
                let actual = zpl::output::raster::rasterize(&doc.unwrap().labels[0]).unwrap();
                let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
                assert!(
                    diff.matches(),
                    "{}: under {} over {}",
                    c[0],
                    diff.reference_only,
                    diff.candidate_only
                );
                assert!(
                    diff.both_black > 0,
                    "{}: positive barcode must contain ink",
                    c[0]
                );
                counts.0 += 1;
            }
            "invalid" => {
                assert!(reference.pixels.iter().all(|&p| p == 255));
                assert!(doc
                    .unwrap_err()
                    .to_string()
                    .contains("11 uncompressed UPC-A digits"));
                counts.1 += 1;
            }
            _ => panic!("unknown fixture status"),
        }
    }
    assert_eq!(counts, (10, 5));
}
