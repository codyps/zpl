//! Unmodified ZD621 resident T/U/V controls; see the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn raw_resident_tuv_frames_pin_pixels_and_text_floor() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/resident-tuv-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/resident-tuv-zd621-v1/manifest.tsv")
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
            (c[3].parse().unwrap(), c[4].parse().unwrap()),
            "{}",
            c[0]
        );
        if c[0] == "origins" {
            for row in 0..3 {
                let top = if row == 0 { 0 } else { 40 + 130 * row };
                let bottom = if row == 2 { 520 } else { 40 + 130 * (row + 1) };
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
        } else {
            assert_eq!(&c[3..5], &["0", "0"]);
        }
        assert_eq!(digest::sha256(&actual.pixels), c[5], "{}", c[0]);
        assert!(reference.pixels.contains(&0));
        count += 1;
    }
    assert_eq!(count, 43);
}

use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
#[test]
fn preset_rotation_pivot_is_an_independent_option() {
    for (font, height, width) in [('T', 48, 42), ('U', 59, 53), ('V', 80, 71)] {
        let source =
            format!("^XA^PW832^LL600^FO200,100^A{font}I,{height},{width}^FDHgypqj 123^FS^XZ");
        let raster = |options| {
            let doc = zpl::render(source.as_bytes(), options).unwrap();
            zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
        };
        let generic = raster(SPECIFICATION);
        let mut options = SPECIFICATION;
        options.compatibility.preset_font_fo_last_dot = true;
        let printer = raster(options);
        assert_ne!(generic, printer);
        assert_eq!(printer, raster(ZD621_203_DPI));
    }
}
