//! ZD621 preview regressions. Raw captures and provenance are in the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn printer_controls_pin_every_painted_pixel() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/field-block-limits-zd621-v1");
    let fields: Vec<Vec<&str>> = include_str!("fixtures/field-block-limits-zd621-v1/fields.tsv")
        .lines()
        .skip(1)
        .map(|line| line.split('\t').collect())
        .collect();
    let mut count = 0;
    let mut field_count = 0;
    for row in include_str!("fixtures/field-block-limits-zd621-v1/manifest.tsv")
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
        assert!(diff.ink_iou() >= 0.8, "{} full-frame text IoU", c[0]);
        let width = reference.width as usize;
        let mut coverage = vec![0u8; reference.pixels.len()];
        for f in fields.iter().filter(|f| f[0] == c[0]) {
            let n = |i: usize| f[i].parse::<usize>().unwrap();
            let (x0, y0, x1, y1) = (n(2), n(3), n(4), n(5));
            assert!(x0 < x1 && y0 < y1 && x1 <= width && y1 <= reference.height as usize);
            let (mut both, mut under, mut over) = (0usize, 0usize, 0usize);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = y * width + x;
                    coverage[i] += 1;
                    let a = reference.pixels[i] < 128;
                    let b = actual.pixels[i] < 128;
                    both += usize::from(a && b);
                    under += usize::from(a && !b);
                    over += usize::from(b && !a);
                }
            }
            assert_eq!(
                (under, over),
                (n(6), n(7)),
                "{} field {} errors",
                c[0],
                f[1]
            );
            let union = both + under + over;
            assert!(
                union > 0 && both * 5 >= union * 4,
                "{} field {} IoU below 80%",
                c[0],
                f[1]
            );
            field_count += 1;
        }
        let has_fields = fields.iter().any(|f| f[0] == c[0]);
        if !has_fields {
            assert!(diff.matches(), "{} must match exactly", c[0]);
        }
        // Neighboring good fields and empty canvas cannot hide a bad field.
        for (i, &covered) in coverage.iter().enumerate() {
            if has_fields && (reference.pixels[i] < 128 || actual.pixels[i] < 128) {
                assert_eq!(covered, 1, "{} ink coverage at pixel {}", c[0], i);
            }
        }
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
    assert_eq!(count, 43);
    assert_eq!(field_count, 211);
}
