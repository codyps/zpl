use zpl::{render, Options};

#[test]
fn captured_font_overlapping_ink_matches_verification() {
    let (s, g) = zpl::bitmap_font::unpack(include_bytes!("../../zpl/assets/font0-32.zbf")).unwrap();
    let (input, reference) = zpl_font_extract::verification_plan(&g, s, "WWW__|||~~").unwrap();
    let doc = render(input.as_bytes(), Options::default()).unwrap();
    let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
    assert!(raster_diff::compare(&reference, &actual, false)
        .unwrap()
        .matches());
}
