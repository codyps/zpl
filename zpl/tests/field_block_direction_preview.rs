//! Raw ZD621 responses and measured regions: see the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;

#[test]
fn printer_block_directions_pin_paint_and_each_text_region() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/field-block-direction-zd621-v1");
    let regions: Vec<Vec<&str>> =
        include_str!("fixtures/field-block-direction-zd621-v1/regions.tsv")
            .lines()
            .skip(1)
            .map(|s| s.split('\t').collect())
            .collect();
    let mut count = 0;
    let mut region_count = 0;
    for row in include_str!("fixtures/field-block-direction-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let input = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&input), c[1]);
        assert_eq!(digest::sha256(&png), c[2]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&input, zpl::render::profiles::ZD621_203_DPI).unwrap();
        assert_eq!(doc.labels.len(), 1);
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (c[4].parse().unwrap(), c[5].parse().unwrap()),
            "{}",
            c[0]
        );
        assert_eq!(digest::sha256(&actual.pixels), c[3], "{}", c[0]);
        for region in regions.iter().filter(|r| r[0] == c[0]) {
            let v: Vec<usize> = region[1..].iter().map(|s| s.parse().unwrap()).collect();
            let (mut under, mut over, mut both) = (0, 0, 0);
            for y in v[1]..v[3] {
                for x in v[0]..v[2] {
                    let i = y * actual.width as usize + x;
                    match (reference.pixels[i] < 128, actual.pixels[i] < 128) {
                        (true, false) => under += 1,
                        (false, true) => over += 1,
                        (true, true) => both += 1,
                        _ => {}
                    }
                }
            }
            assert_eq!((under, over), (v[4], v[5]), "{} region {:?}", c[0], &v[..4]);
            assert!(
                both > 0 && both * 5 >= (both + under + over) * 4,
                "{} region {:?} below 80% foreground IoU",
                c[0],
                &v[..4]
            );
            region_count += 1;
        }
        count += 1;
    }
    assert_eq!(count, 41);
    assert_eq!(region_count, 230);
}
