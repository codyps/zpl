//! Scalable size range: Zebra ZPL Programming Guide ^A p. 60, ^CF p. 154.
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
fn image(source: &str, options: zpl::Options) -> raster_diff::Raster {
    let doc = zpl::render(source.as_bytes(), options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
}
#[test]
fn minimum_size_is_an_independent_option_after_zero_inference() {
    for (small, normal) in [
        ("^A0N,1,1", "^A0N,10,10"),
        ("^A0N,1,0", "^A0N,10,10"),
        ("^A0N,0,1", "^A0N,10,10"),
        ("^CF0,1,32", "^CF0,10,32"),
        ("^CF0,32,1", "^CF0,32,10"),
    ] {
        let source = format!("^XA^PW832^LL300^FO40,40{small}^FDWim09^FS^XZ");
        let expected = format!("^XA^PW832^LL300^FO40,40{normal}^FDWim09^FS^XZ");
        assert!(zpl::render(source.as_bytes(), SPECIFICATION).is_err());
        assert_eq!(
            image(&source, ZD621_203_DPI),
            image(&expected, ZD621_203_DPI)
        );
        let mut options = SPECIFICATION;
        options.compatibility.font0_minimum_dimensions = true;
        assert_eq!(image(&source, options), image(&expected, options));
        options = ZD621_203_DPI;
        options.compatibility.font0_minimum_dimensions = false;
        assert!(zpl::render(source.as_bytes(), options).is_err());
    }
    // Bitmap fonts retain their native-matrix scaling rules.
    assert!(zpl::render(b"^XA^AAN,1,1^FDA^FS^XZ", SPECIFICATION).is_ok());
}
#[test]
fn whole_dot_fo_baseline_is_separate_from_minimum_size_and_ft() {
    let source = "^XA^PW832^LL300^FO40,40^A0R,10,10^FDWim09^FS^XZ";
    let native = image(source, ZD621_203_DPI);
    let mut options = ZD621_203_DPI;
    options.compatibility.font0_fo_floor_baseline = false;
    assert_ne!(native, image(source, options));
    let ft = source.replace("^FO", "^FT");
    assert_eq!(image(&ft, ZD621_203_DPI), image(&ft, options));
    options = SPECIFICATION;
    options.compatibility.font0_fo_floor_baseline = true;
    assert_eq!(native, image(source, options));
}
