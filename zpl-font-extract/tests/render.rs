use zpl::render;
use zpl::render::profiles::SPECIFICATION;

#[test]
fn captured_font_overlapping_ink_matches_verification() {
    let (s, g) = zpl::bitmap_font::unpack(include_bytes!("../../zpl/assets/font0-32.zbf")).unwrap();
    let (input, reference) = zpl_font_extract::verification_plan(&g, s, "WWW__|||~~", 27).unwrap();
    let doc = render(input.as_bytes(), SPECIFICATION).unwrap();
    let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
    assert!(raster_diff::compare(&reference, &actual, false)
        .unwrap()
        .matches());
}
