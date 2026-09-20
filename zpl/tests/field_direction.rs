use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
fn image(body: &str, options: zpl::Options) -> raster_diff::Raster {
    let source = format!("^XA^PW832^LL400{body}^XZ");
    let doc = zpl::render(source.as_bytes(), options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
}

#[test]
fn specification_places_glyphs_in_each_direction_with_gap() {
    // Zebra Programming Guide ^FP p. 202: H/V/R direction, inter-character gap.
    // Native font A has a six-dot cell, doubled here to a twelve-dot advance.
    for (d, step_x, step_y) in [('H', 15, 0), ('V', 0, 21), ('R', -15, 0)] {
        let actual = image(
            &format!("^FO100,100^FP{d},3^AAN,18,10^FDAB12^FS"),
            SPECIFICATION,
        );
        let mut expected = String::new();
        for (i, c) in "AB12".chars().enumerate() {
            expected += &format!(
                "^FO{},{}^AAN,18,10^FD{c}^FS",
                100 + i as i32 * step_x,
                100 + i as i32 * step_y
            );
        }
        assert_eq!(actual, image(&expected, SPECIFICATION), "{d}");
    }
}

#[test]
fn vertical_gap_departure_is_independent() {
    let body = "^FO100,100^FPV,3^AAN,18,10^FDAB12^FS";
    let native = image(body, ZD621_203_DPI);
    let mut options = ZD621_203_DPI;
    options.compatibility.field_vertical_ignores_gap = false;
    assert_eq!(image(body, options), image(body, SPECIFICATION));
    assert_ne!(image(body, options), native);
}

#[test]
fn field_direction_resets_at_field_separator_and_missing_parameters_use_defaults() {
    let body = "^FO100,100^FPR,8^AAN,18,10^FDAB12^FS^FO100,200^FP^AAN,18,10^FDAB12^FS";
    let expected = "^FO100,100^FPR,8^AAN,18,10^FDAB12^FS^FO100,200^AAN,18,10^FDAB12^FS";
    assert_eq!(image(body, ZD621_203_DPI), image(expected, ZD621_203_DPI));
}

#[test]
fn direction_applies_to_font_fields_without_moving_barcodes() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        let fields = "^FO300,200,1^BY2,3,70^BCI,70,N,N,N,N^FDAB1234^FS";
        assert_eq!(
            image(&format!("^FPR,8{fields}"), options),
            image(fields, options)
        );
    }
}

#[test]
fn invalid_direction_and_gap_are_rejected() {
    for operand in ["X", "H,-1", "H,10000", "V,1.5", "H,0,1"] {
        let input = format!("^XA^FP{operand}^XZ");
        assert!(zpl::render(input.as_bytes(), SPECIFICATION).is_err());
    }
}

#[test]
fn inverted_font_zero_margin_remains_an_independent_option() {
    let body = "^FO250,200,1^A0I,32,24^FDjW^FS";
    let mut options = ZD621_203_DPI;
    options
        .compatibility
        .right_justified_inverted_text_uses_ink_margin = false;
    assert_eq!(image(body, options), image(body, SPECIFICATION));
    assert_ne!(image(body, options), image(body, ZD621_203_DPI));
}
