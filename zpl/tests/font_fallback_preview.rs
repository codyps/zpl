//! Raw native controls and provenance: fixtures/font-fallback-zd621-v1/README.md.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
#[test]
fn unavailable_fonts_match_native_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/font-fallback-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/font-fallback-zd621-v1/manifest.tsv")
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
        if c[0] == "controls-rotations" {
            // Four isolated text fields occupy separate quadrants. Check each
            // foreground independently so blank canvas cannot inflate accuracy.
            for (x0, x1, y0, y1) in [
                (0, 416, 0, 609),
                (416, 832, 0, 609),
                (0, 416, 609, 1218),
                (416, 832, 609, 1218),
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
    assert_eq!(count, 33);
}
#[test]
fn fallback_is_optional_and_excludes_real_preset_fonts() {
    // ^A pp. 60–61, ^CF p. 154; unavailable is distinct from malformed IDs.
    for id in "123456789IJKLMNOWXYZ".chars() {
        for command in [format!("^A{id}N,32,24"), format!("^CF{id},32,24")] {
            let source = format!("^XA^PW832^LL200^CFB,32,24^FO80,80{command}^FDAB12^FS^XZ");
            assert!(zpl::render(source.as_bytes(), SPECIFICATION).is_err());
            assert!(zpl::render(source.as_bytes(), ZD621_203_DPI).is_ok());
            let mut options = ZD621_203_DPI;
            options.compatibility.unavailable_fonts_use_default = false;
            assert!(zpl::render(source.as_bytes(), options).is_err());
        }
    }
    for id in "QRSTUV?".chars() {
        let source = format!("^XA^FO80,80^A{id}N,32,24^FDAB12^FS^XZ");
        assert!(zpl::render(source.as_bytes(), ZD621_203_DPI).is_err());
    }
}
