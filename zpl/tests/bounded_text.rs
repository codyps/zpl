use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
fn image(body: &str, options: zpl::Options) -> raster_diff::Raster {
    let source = format!("^XA^PW832^LL400{body}^XZ");
    let doc = zpl::render(source.as_bytes(), options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
}
#[test]
fn specification_wraps_and_clips_at_the_requested_height() {
    // Programming Guide ^TB p. 356: wrap words, truncate at block height.
    // https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
    let actual = image(
        "^FO100,100^AAN,18,10^TBN,48,18^FDABCD EFGH^FS",
        SPECIFICATION,
    );
    assert_eq!(
        actual,
        image("^FO100,100^AAN,18,10^FDABCD^FS", SPECIFICATION)
    );
}
#[test]
fn text_block_ends_with_its_field_and_can_be_replaced_by_fb() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        let plain = "^FO100,100^AAN,18,10^FDAB^FS";
        let empty = "^FO0,0^AAN,18,10^TBN,1,1^FD^FS";
        assert_eq!(
            image(&format!("{empty}{plain}"), options),
            image(plain, options)
        );
        let a = "^FO100,100^AAN,18,10^TBN,1,1^FB100,2,0,L,0^FDAB^FS";
        let b = "^FO100,100^AAN,18,10^FB100,2,0,L,0^FDAB^FS";
        assert_eq!(image(a, options), image(b, options));
    }
}
#[test]
fn printer_line_pitch_can_be_disabled_independently() {
    let body = "^FO100,100^AAN,18,10^TBN,24,100^FDAB CD EF^FS";
    let mut options = ZD621_203_DPI;
    options.compatibility.bounded_text_printer_pitch = false;
    assert_eq!(image(body, options), image(body, SPECIFICATION));
    assert_ne!(image(body, options), image(body, ZD621_203_DPI));
}
#[test]
fn invalid_block_dimensions_are_rejected() {
    for params in ["N,0,20", "N,20,0", "N,-1,20", "N,1.5,20", "X,20,20"] {
        assert!(zpl::render(format!("^XA^TB{params}^XZ").as_bytes(), SPECIFICATION).is_err());
    }
}

#[test]
fn font_selection_cancels_native_block_only_when_enabled() {
    let body = "^FO100,100^TBN,24,100^AAN,18,10^FDAB CD^FS";
    let plain = "^FO100,100^AAN,18,10^FDAB CD^FS";
    assert_eq!(image(body, ZD621_203_DPI), image(plain, ZD621_203_DPI));
    let mut options = ZD621_203_DPI;
    options.compatibility.bounded_text_font_cancels_block = false;
    assert_ne!(image(body, options), image(plain, options));
}

#[test]
fn block_anchor_departure_can_be_disabled_independently() {
    let body = "^FO200,200^AAN,18,10^TBI,80,60^FDAB^FS";
    let mut options = ZD621_203_DPI;
    options.compatibility.bounded_text_printer_anchors = false;
    assert_eq!(image(body, options), image(body, SPECIFICATION));
    assert_ne!(image(body, options), image(body, ZD621_203_DPI));
}
