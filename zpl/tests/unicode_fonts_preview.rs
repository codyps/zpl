//! Unmodified ZD621 Unicode font controls; see the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn raw_unicode_font_frames_pin_every_pixel_and_meet_text_goal() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/unicode-fonts-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/unicode-fonts-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let source = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&source), c[1]);
        assert_eq!(digest::sha256(&png), c[2]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&source, zpl::render::profiles::ZD621_203_DPI).unwrap();
        assert_eq!(doc.labels.len(), 1);
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (
                c[4].parse::<usize>().unwrap(),
                c[5].parse::<usize>().unwrap()
            ),
            "{}",
            c[0]
        );
        if c[0].starts_with("holdout-heldout-") {
            // Four separately positioned N/R/I/B fields in the raw 832x1920
            // holdouts. Foreground IoU prevents blank canvas from hiding loss.
            for (lo, hi) in [(0, 300), (300, 900), (900, 1300), (1300, 1920)] {
                let range = lo * reference.width as usize..hi * reference.width as usize;
                let (mut intersection, mut union) = (0_u64, 0_u64);
                for (&a, &b) in reference.pixels[range.clone()]
                    .iter()
                    .zip(&actual.pixels[range])
                {
                    intersection += u64::from(a < 128 && b < 128);
                    union += u64::from(a < 128 || b < 128);
                }
                assert!(
                    union > 0 && intersection * 100 >= union * 80,
                    "{} rows {lo}..{hi}",
                    c[0]
                );
            }
        } else {
            assert_eq!(&c[4..], &["0", "0"]);
        }
        assert_eq!(digest::sha256(&actual.pixels), c[3], "{}", c[0]);
        assert!(reference.pixels.contains(&0));
        count += 1;
    }
    assert_eq!(count, 40);
}

#[test]
fn captured_unicode_font_assets_are_pinned() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    for row in include_str!("fixtures/unicode-fonts-zd621-v1/assets.tsv")
        .lines()
        .skip(1)
    {
        let (name, expected) = row.split_once('\t').unwrap();
        assert_eq!(
            digest::sha256(&fs::read(root.join(name)).unwrap()),
            expected,
            "{name}"
        );
    }
}
