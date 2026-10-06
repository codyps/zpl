//! Native TAB controls; source/command references are in the fixture README.
use std::{fs, path::Path};
use zpl::{
    output::raster::rasterize,
    render::profiles::{SPECIFICATION, ZD621_203_DPI},
};
#[path = "support/digest.rs"]
mod digest;
#[test]
fn native_tab_frames_pin_paint_and_foreground_accuracy() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tabs-zd621-v1");
    let (mut frames, mut regions) = (0, 0);
    for row in include_str!("fixtures/tabs-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let n = |i: usize| c[i].parse::<usize>().unwrap();
        let source = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&source), c[1]);
        assert_eq!(digest::sha256(&png), c[2]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&source, ZD621_203_DPI).unwrap();
        assert_eq!(doc.labels.len(), 1);
        let actual = rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (n(3), n(4)),
            "{}",
            c[0]
        );
        assert_eq!(digest::sha256(&actual.pixels), c[5]);
        for row in 0..n(7) {
            let bottom = if row + 1 == n(7) {
                reference.height as usize
            } else {
                (row + 1) * n(8)
            };
            for column in 0..n(6) {
                let (mut intersection, mut union) = (0, 0);
                for y in row * n(8)..bottom {
                    for x in column * 832 / n(6)..(column + 1) * 832 / n(6) {
                        let i = y * 832 + x;
                        let a = reference.pixels[i] == 0;
                        let b = actual.pixels[i] == 0;
                        intersection += usize::from(a && b);
                        union += usize::from(a || b);
                    }
                }
                assert!(
                    union > 0 && intersection * 100 >= union * 80,
                    "{} region {row}/{column}: {intersection}/{union}",
                    c[0]
                );
                regions += 1;
            }
        }
        frames += 1;
    }
    assert_eq!((frames, regions), (11, 120));
}

#[test]
fn tab_option_is_independent_and_preserves_barcode_data() {
    let source = b"^XA^PW832^LL300^FO20,20^A0N,32,0^FH^FD_09AB^FS^XZ";
    assert!(zpl::render(source, SPECIFICATION).is_err());
    let mut compatible = SPECIFICATION;
    compatible.compatibility.text_tab_stops = true;
    let actual = zpl::render(source, compatible).unwrap();
    let explicit = zpl::render(
        b"^XA^PW832^LL300^FO100,20^A0N,32,0^FDAB^FS^XZ",
        SPECIFICATION,
    )
    .unwrap();
    assert_eq!(
        rasterize(&actual.labels[0]).unwrap(),
        rasterize(&explicit.labels[0]).unwrap()
    );
    let mut strict = ZD621_203_DPI;
    strict.compatibility.text_tab_stops = false;
    assert!(zpl::render(source, strict).is_err());
    let barcode = b"^XA^PW832^LL300^FO20,20^BQN,2,3^FH^FDQA,AB_09CD^FS^XZ";
    let a = zpl::render(barcode, strict).unwrap();
    let b = zpl::render(barcode, ZD621_203_DPI).unwrap();
    assert_eq!(
        rasterize(&a.labels[0]).unwrap(),
        rasterize(&b.labels[0]).unwrap()
    );
}
