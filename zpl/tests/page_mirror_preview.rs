//! Unmodified ZD621 previews; capture provenance is in the fixture README.
use std::{fs, path::Path};
#[path = "support/digest.rs"]
mod digest;

#[test]
fn printer_page_mirror_controls_are_pixel_exact() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/page-mirror-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/page-mirror-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let input = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&input), c[1]);
        assert_eq!(digest::sha256(&png), c[2]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let document = zpl::render(&input, zpl::render::profiles::ZD621_203_DPI).unwrap();
        assert_eq!(document.labels.len(), 1);
        let actual = zpl::output::raster::rasterize(&document.labels[0]).unwrap();
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert!(
            diff.matches(),
            "{} underpaint={}, overpaint={}",
            c[0],
            diff.reference_only,
            diff.candidate_only
        );
        assert!(diff.both_black > 0);
        assert_eq!(digest::sha256(&actual.pixels), c[3]);
        count += 1;
    }
    assert_eq!(count, 13);
}
