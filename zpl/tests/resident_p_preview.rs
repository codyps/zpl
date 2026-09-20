//! Raw native controls and provenance: fixtures/resident-p-zd621-v1/README.md.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
#[test]
fn preset_font_native_frames_pin_underpaint_and_overpaint() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/resident-p-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/resident-p-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let source = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&source), c[1]);
        assert_eq!(digest::sha256(&png), c[2]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&source, ZD621_203_DPI).unwrap();
        assert_eq!(doc.labels.len(), 1);
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare(&reference, &actual, false).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (c[3].parse().unwrap(), c[4].parse().unwrap()),
            "{}",
            c[0]
        );
        assert!(reference.pixels.contains(&0));
        assert_eq!(digest::sha256(&actual.pixels), c[5]);
        if matches!(c[0], "P-native" | "P-tall" | "P-double") {
            // Separate regions for FO/FT at N/R/I/B. Native clipping is retained.
            for (x0, x1, y0, y1) in [
                (0, 400, 0, 180),
                (0, 400, 180, 300),
                (550, 700, 300, 600),
                (400, 550, 300, 600),
                (300, 832, 700, 850),
                (300, 832, 850, 1000),
                (0, 180, 900, 1218),
                (180, 350, 900, 1218),
            ] {
                let (mut intersection, mut union) = (0usize, 0usize);
                for y in y0..y1 {
                    for x in x0..x1 {
                        let i = y * 832 + x;
                        let a = reference.pixels[i] == 0;
                        let b = actual.pixels[i] == 0;
                        intersection += usize::from(a && b);
                        union += usize::from(a || b);
                    }
                }
                assert!(union > 0 && intersection * 100 >= union * 80);
            }
        }

        count += 1;
    }
    assert_eq!(count, 59);
}

#[test]
fn preset_rotation_pivot_is_an_independent_option() {
    let source = b"^XA^PW832^LL600^FO200,100^APR,40,36^FDHgypqj 123^FS^XZ";
    let raster = |options| {
        let doc = zpl::render(source, options).unwrap();
        zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
    };
    let generic = raster(SPECIFICATION);
    let mut options = SPECIFICATION;
    options.compatibility.preset_font_fo_last_dot = true;
    let printer = raster(options);
    assert_ne!(generic, printer);
    assert_eq!(printer, raster(ZD621_203_DPI));
}
