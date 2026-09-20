//! Unmodified ZD621 QR module-state controls; see the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn raw_module_state_frames_are_pixel_exact() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qr-module-state-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/qr-module-state-zd621-v1/manifest.tsv")
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
    assert_eq!(count, 4);
}

#[test]
fn qr_module_state_is_an_independent_option_and_by_can_reset_it() {
    use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
    let render = |setup: &str, options| {
        let source = format!("^XA^PW832^LL240^BY2,3,50^FO40,40{setup}^BCN,50,Y,N,N,N^FDAB12^FS^XZ");
        let doc = zpl::render(source.as_bytes(), options).unwrap();
        zpl::output::raster::rasterize(&doc.labels[0])
            .unwrap()
            .pixels
    };
    let mut selected = SPECIFICATION;
    selected.compatibility.qr_updates_barcode_module_width = true;
    for options in [selected, ZD621_203_DPI] {
        assert_eq!(render("^BQN,2,3", options), render("^BY3", options));
        assert_ne!(render("^BQN,2,3", options), render("", options));
        assert_eq!(render("^BQN,2,3^BY2", options), render("", options));
        assert_eq!(render("^BY4^BQN,2", options), render("", options));
    }
    assert_eq!(render("^BQN,2,3", SPECIFICATION), render("", SPECIFICATION));
    let mut disabled = ZD621_203_DPI;
    disabled.compatibility.qr_updates_barcode_module_width = false;
    assert_eq!(render("^BQN,2,3", disabled), render("", disabled));
}

#[test]
fn invalid_qr_magnification_preserves_validation_and_module_state() {
    use zpl::render::profiles::ZD621_203_DPI;
    let source = b"^XA^PW832^LL240^BY2,3,50^CVY^FO40,40^BQN,2,0^FDLA,A^FS^FO300,40^BCN,50,N,N,N,N^FDAB12^FS^XZ";
    let render = |options| {
        let doc = zpl::render(source, options).unwrap();
        zpl::output::raster::rasterize(&doc.labels[0])
            .unwrap()
            .pixels
    };
    let mut disabled = ZD621_203_DPI;
    disabled.compatibility.qr_updates_barcode_module_width = false;
    assert_eq!(render(disabled), render(ZD621_203_DPI));
}
