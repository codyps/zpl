//! Independent native CI13/source-remapping controls; see the fixture README.
#[path = "support/digest.rs"]
mod digest;

#[test]
fn measured_ci13_zero_and_explicit_source_remapping_are_pixel_exact() {
    let source = include_bytes!("fixtures/bitmap-ci13-zd621-v1/control.zpl");
    let png = include_bytes!("fixtures/bitmap-ci13-zd621-v1/control.png");
    let repeated = include_bytes!("fixtures/bitmap-ci13-zd621-v1/repeat.png");
    assert_eq!(
        digest::sha256(source),
        "38cc46453591c6ebec8890c7915612b8a8aca485eafe0b3a54310f378782a197"
    );
    assert_eq!(
        digest::sha256(png),
        "e30e1344389cff69aac8b5a52616ce0bf9516cb71faac29f5e9c4aff6f56e7f9"
    );
    assert_eq!(png.as_slice(), repeated.as_slice());
    let reference = raster_diff::Raster::decode_png(png).unwrap();
    assert_eq!((reference.width, reference.height), (832, 256));
    assert!(reference.pixels.contains(&0));
    let scene = zpl::render(source, zpl::render::profiles::ZD621_203_DPI).unwrap();
    let actual = zpl::output::raster::rasterize(&scene.labels[0]).unwrap();
    assert_eq!(
        (actual.width, actual.height),
        (reference.width, reference.height)
    );
    assert!(
        actual.pixels == reference.pixels,
        "native preview pixels differ"
    );
}
