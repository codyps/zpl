//! Unmodified ZD621 QR mask holdouts; see the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn fresh_holdouts_are_pixel_exact() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qr-mask-holdouts-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/qr-mask-holdouts-zd621-v1/manifest.tsv")
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
        {
            assert_eq!(&c[3..5], &["0", "0"]);
        }
        let both = reference
            .pixels
            .iter()
            .zip(&actual.pixels)
            .filter(|(a, b)| **a == 0 && **b == 0)
            .count();
        assert!(both * 100 >= (both + diff.reference_only + diff.candidate_only) * 80);
        assert_eq!(digest::sha256(&actual.pixels), c[5], "{}", c[0]);
        assert!(reference.pixels.contains(&0));
        count += 1;
    }
    assert_eq!(count, 8);
}

#[test]
fn automatic_selection_is_independent_and_ignores_the_mask_operand() {
    use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
    for model in [1, 2] {
        let render = |mask, options| {
            let input = format!("^XA^PW832^LL300^FO30,30^BQN,{model},3,H,{mask}^FDHA,A^FS^XZ");
            let doc = zpl::render(input.as_bytes(), options).unwrap();
            zpl::output::raster::rasterize(&doc.labels[0])
                .unwrap()
                .pixels
        };
        let mut selected = SPECIFICATION;
        selected.compatibility.qr_printer_mask_selection = true;
        for options in [selected, ZD621_203_DPI] {
            let first = render(0, options);
            for mask in 1..8 {
                assert_eq!(first, render(mask, options));
            }
        }
        assert_ne!(render(0, SPECIFICATION), render(7, SPECIFICATION));
        let mut disabled = ZD621_203_DPI;
        disabled.compatibility.qr_printer_mask_selection = false;
        assert_ne!(render(0, disabled), render(7, disabled));
    }
}
