//! Native controls and specification references: fixtures/bitmap-maximum-zd621-v1/README.md.
use std::{fs, path::Path};
use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn native_bitmap_maximum_frames_are_pixel_exact() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bitmap-maximum-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/bitmap-maximum-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let source = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&source), c[1]);
        assert_eq!(digest::sha256(&png), c[2]);
        assert_eq!(&c[3..5], &["0", "0"]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&source, ZD621_203_DPI).unwrap();
        assert_eq!(doc.labels.len(), 1);
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        assert_eq!(actual, reference, "{}", c[0]);
        assert_eq!(digest::sha256(&actual.pixels), c[5]);
        assert!(reference.pixels.contains(&0));
        count += 1;
    }
    assert_eq!(count, 19);
}

#[test]
fn bitmap_maximum_compatibility_is_independent_of_other_options() {
    // ^A p. 60 limits bitmap fonts to 10x; ^GS p. 217 allows 32000 dots.
    for command in ["^AAN,99,55", "^CFA,99,55"] {
        let source = format!("^XA^PW832^LL832^FO100,100{command}^FDWj^FS^XZ");
        assert!(zpl::render(source.as_bytes(), SPECIFICATION)
            .unwrap_err()
            .message
            .contains("ten times"));
        let mut strict = ZD621_203_DPI;
        strict.compatibility.bitmap_font_maximum_dimensions = false;
        assert!(zpl::render(source.as_bytes(), strict).is_err());
        let mut compatible = SPECIFICATION;
        compatible.compatibility.bitmap_font_maximum_dimensions = true;
        let capped = zpl::render(source.as_bytes(), compatible).unwrap();
        let explicit =
            zpl::render(source.replace("99,55", "90,50").as_bytes(), SPECIFICATION).unwrap();
        assert_eq!(
            zpl::output::raster::rasterize(&capped.labels[0]).unwrap(),
            zpl::output::raster::rasterize(&explicit.labels[0]).unwrap()
        );
    }
    let source = b"^XA^PW832^LL832^FO100,100^GSN,264,264^FDA^FS^XZ";
    let doc = zpl::render(source, SPECIFICATION).unwrap();
    let mut compatible = SPECIFICATION;
    compatible.compatibility.bitmap_font_maximum_dimensions = true;
    let capped = zpl::render(source, compatible).unwrap();
    assert_ne!(
        zpl::output::raster::rasterize(&doc.labels[0]).unwrap(),
        zpl::output::raster::rasterize(&capped.labels[0]).unwrap()
    );
}
