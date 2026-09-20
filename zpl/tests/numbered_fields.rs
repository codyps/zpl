//! ^FN p. 200, Zebra ZPL II Programming Guide:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
fn pixels(body: &str, options: zpl::Options) -> Vec<u8> {
    let source = format!("^XA^PW832^LL400^CI13^CFA,18,10{body}^XZ");
    let doc = zpl::render(source.as_bytes(), options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0])
        .unwrap()
        .pixels
}
#[test]
fn specification_shares_data_with_earlier_and_later_references() {
    let body = "^FO80,80^FN1^FS^FO80,140^FN1^FDAB12^FS^FO80,200^FN1^FS";
    let plain = "^FO80,80^FDAB12^FS^FO80,140^FDAB12^FS^FO80,200^FDAB12^FS";
    assert_eq!(pixels(body, SPECIFICATION), pixels(plain, SPECIFICATION));
    let mut options = ZD621_203_DPI;
    options.compatibility.numbered_fields_forward_only = false;
    assert_eq!(pixels(body, options), pixels(plain, options));
}
#[test]
fn printer_consumes_each_binding_when_pending_references_exist() {
    let body = "^FO80,80^FN1^FS^FO80,140^FN1^FS^FO80,200^FN1^FDAB12^FS^FO80,260^FN1^FDCD34^FS^FO80,320^FN1^FS";
    let plain = "^FO80,80^FDAB12^FS^FO80,140^FDAB12^FS^FO80,260^FDCD34^FS";
    assert_eq!(pixels(body, ZD621_203_DPI), pixels(plain, ZD621_203_DPI));
    let mut options = SPECIFICATION;
    options.compatibility.numbered_fields_forward_only = true;
    assert_eq!(pixels(body, options), pixels(plain, options));
}
#[test]
fn hex_decoding_belongs_to_the_binding_but_style_belongs_to_the_reference() {
    let body = "^FO80,80^AAN,18,10^FN1^FS^FO80,140^A0N,32,0^FN1^FH^FDAB_31_32^FS";
    assert_eq!(
        pixels(body, ZD621_203_DPI),
        pixels("^FO80,80^AAN,18,10^FDAB12^FS", ZD621_203_DPI)
    );
    let body = "^FO80,80^FH^FN1^FS^FO80,140^FN1^FDAB_31_32^FS";
    assert_eq!(
        pixels(body, ZD621_203_DPI),
        pixels("^FO80,80^FDAB_31_32^FS", ZD621_203_DPI)
    );
}
#[test]
fn numbers_prompts_and_multiple_data_commands_are_validated() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        for value in [
            "-1",
            "10000",
            "1.5",
            "1,2",
            "1\"unclosed",
            "1\"a\"extra",
            "1\"a_b\"",
        ] {
            let source = format!("^XA^FN{value}^FS^XZ");
            assert!(zpl::render(source.as_bytes(), options).is_err(), "{value}");
        }
        for body in ["^FN1^FN2^FS", "^FN1^FDAB^FDCD^FS", "^FN1^SN123,1,Y^FS"] {
            assert!(zpl::render(format!("^XA{body}^XZ").as_bytes(), options).is_err());
        }
        assert_eq!(
            pixels("^FO80,80^FN\"Name\"^FDAB^FS", options),
            pixels("^FO80,80^FDAB^FS", options)
        );
    }
}
#[test]
fn numbering_is_label_local_and_does_not_rewrite_field_bytes() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        let source = b"^XA^FN1^FDAB^FS^XZ^XA^FN1^FS^XZ";
        let doc = zpl::render(source, options).unwrap();
        let raster = zpl::output::raster::rasterize(&doc.labels[1]).unwrap();
        assert!(raster.pixels.iter().all(|&p| p == 255));
        assert_eq!(
            pixels("^FO80,80^FDFN1 literal^FS", options),
            pixels("^FO80,80^FN1^FDFN1 literal^FS", options)
        );
    }
}
#[test]
fn substituted_data_errors_retain_original_reference_offsets() {
    let source = b"^XA^CI28^FO80,80^AAN,18,10^FN1^FS^FO80,140^FN1^FH^FD_FF^FS^XZ";
    let err = zpl::render(source, ZD621_203_DPI).unwrap_err();
    assert_eq!(
        err.offset,
        source.windows(3).position(|s| s == b"^FS").unwrap()
    );
    assert!(err.message.contains("UTF-8"));
}
#[test]
fn references_preserve_changed_prefixes_and_control_terminators() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        let source = "^XA^PW832^LL400^CI13^CFA,18,10^FO80,80^FN1^FS^FO80,140^FN1^FDAB12^FS^XZ";
        let expected = zpl::render(source.as_bytes(), options).unwrap();
        let expected = zpl::output::raster::rasterize(&expected.labels[0]).unwrap();
        for changed in [
            format!("^CC!{}", source.replace('^', "!")),
            source
                .replace("^XA", "\u{2}")
                .replace("^FS", "\u{f}")
                .replace("^XZ", "\u{3}"),
        ] {
            let doc = zpl::render(changed.as_bytes(), options).unwrap();
            assert_eq!(
                zpl::output::raster::rasterize(&doc.labels[0]).unwrap(),
                expected
            );
        }
    }
}
