//! Unmodified ZD621 shipping-label font controls; see the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn raw_shipping_font_frames_pin_pixels_and_field_accuracy() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shipping-fonts-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/shipping-fonts-zd621-v1/manifest.tsv")
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
        let diff = raster_diff::compare(&reference, &actual, false).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (c[3].parse().unwrap(), c[4].parse().unwrap()),
            "{}",
            c[0]
        );
        if c[0] == "origins" {
            for row in 0..5 {
                let top = if row == 0 { 0 } else { 40 + 140 * row };
                let bottom = if row == 4 { 800 } else { 40 + 140 * (row + 1) };
                for column in 0..8 {
                    let (left, right) = (104 * column, 104 * (column + 1));
                    let (mut intersection, mut union) = (0, 0);
                    for y in top..bottom {
                        for x in left..right {
                            let i = y * 832 + x;
                            let a = actual.pixels[i] == 0;
                            let r = reference.pixels[i] == 0;
                            intersection += usize::from(a && r);
                            union += usize::from(a || r);
                        }
                    }
                    assert!(
                        union > 0 && intersection * 100 >= union * 80,
                        "{} field at {left},{top}",
                        c[0]
                    );
                }
            }
        } else if matches!(c[0], "original" | "shipping-repeat") {
            // Assign each text field its own region. FR uses white ink within
            // the filled black box, so background black cannot inflate IoU.
            let regions = [
                (35, 35, 790, 115, false),
                (35, 140, 510, 320, false),
                (45, 495, 765, 605, true),
                (400, 970, 735, 1100, false),
                (735, 830, 790, 1120, false),
            ];
            let mut text = vec![false; actual.pixels.len()];
            for (left, top, right, bottom, reverse) in regions {
                let (mut intersection, mut union) = (0, 0);
                for y in top..bottom {
                    for x in left..right {
                        let i = y * 832 + x;
                        text[i] = true;
                        let a = (actual.pixels[i] == 0) != reverse;
                        let r = (reference.pixels[i] == 0) != reverse;
                        intersection += usize::from(a && r);
                        union += usize::from(a || r);
                    }
                }
                assert!(
                    union > 0 && intersection * 100 >= union * 80,
                    "{} text field at {left},{top}",
                    c[0]
                );
            }
            for (i, is_text) in text.into_iter().enumerate() {
                if !is_text {
                    assert_eq!(
                        actual.pixels[i], reference.pixels[i],
                        "{} non-text pixel {i}",
                        c[0]
                    );
                }
            }
        } else {
            assert_eq!(&c[3..5], &["0", "0"]);
        }
        assert_eq!(digest::sha256(&actual.pixels), c[5], "{}", c[0]);
        assert!(reference.pixels.contains(&0));
        count += 1;
    }
    assert_eq!(count, 82);
}
