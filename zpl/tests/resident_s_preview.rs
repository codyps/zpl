//! Unmodified ZD621 resident S controls; see the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn raw_resident_s_frames_pin_pixels_and_text_floor() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/resident-s-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/resident-s-zd621-v1/manifest.tsv")
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
        let (rows, columns, pitch, height) = match c[0] {
            "origins" => (1, 8, 200, 200),
            "blocks" => (8, 4, 190, 1600),
            "lines-40" | "lines-80" => (8, 2, 190, 1600),
            _ => {
                assert_eq!(&c[3..5], &["0", "0"]);
                (0, 0, 0, 0)
            }
        };
        for row in 0..rows {
            let top = if row == 0 { 0 } else { 40 + pitch * row };
            let bottom = if row + 1 == rows {
                height
            } else {
                40 + pitch * (row + 1)
            };
            for col in 0..columns {
                let (mut intersection, mut union) = (0, 0);
                for y in top..bottom {
                    for x in col * (832 / columns)..(col + 1) * (832 / columns) {
                        let i = y * 832 + x;
                        let a = actual.pixels[i] == 0;
                        let r = reference.pixels[i] == 0;
                        intersection += usize::from(a && r);
                        union += usize::from(a || r);
                    }
                }
                assert!(
                    union > 0 && intersection * 100 >= union * 80,
                    "{} row {row} col {col}",
                    c[0]
                );
            }
        }
        assert_eq!(digest::sha256(&actual.pixels), c[5], "{}", c[0]);
        assert!(reference.pixels.contains(&0));
        count += 1;
    }
    assert_eq!(count, 32);
}

#[test]
fn s_block_metrics_are_an_independent_profile_option() {
    use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
    for (h, w) in [(40, 35), (80, 70)] {
        let source = format!("^XA^PW832^LL600^FO60,60^ASN,{h},{w}^FB100,2,0,L,0^FDW\\&g^FS^XZ");
        let render = |options| {
            let doc = zpl::render(source.as_bytes(), options).unwrap();
            zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
        };
        let mut selected = SPECIFICATION;
        selected.compatibility.font_s_block_metrics = true;
        assert_ne!(render(SPECIFICATION), render(selected));
        assert_eq!(render(selected), render(ZD621_203_DPI));
    }
}

#[test]
fn s_rotation_pivot_is_an_independent_profile_option() {
    use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
    for (h, w) in [(40, 35), (80, 70)] {
        let source = format!("^XA^PW832^LL600^FO100,100^ASI,{h},{w}^FDW^FS^XZ");
        let render = |options| {
            let doc = zpl::render(source.as_bytes(), options).unwrap();
            zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
        };
        let mut selected = SPECIFICATION;
        selected.compatibility.preset_font_fo_last_dot = true;
        assert_ne!(render(SPECIFICATION), render(selected));
        assert_eq!(render(selected), render(ZD621_203_DPI));
    }
}
