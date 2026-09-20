//! Zebra Programming Guide ^FE pp. 191–192:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
fn image(body: &str, options: zpl::Options) -> raster_diff::Raster {
    let source = format!("^XA^PW832^LL600^CI28^CF0,32,0{body}^XZ");
    let doc = zpl::render(source.as_bytes(), options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
}
fn field(value: &str, options: zpl::Options) -> raster_diff::Raster {
    image(
        &format!("^FO80,80^FN1^FDABCDEFGHIJ^FS^FO80,160{value}^FS"),
        options,
    )
}
#[test]
fn whole_values_forward_slices_missing_references_and_clamping() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        assert_eq!(
            field("^FE^FD#1#/#1,f,2,3#/#1,f,9,99#/#99#", options),
            field("^FDABCDEFGHIJ/BCD/IJ/", options)
        );
        assert_eq!(
            field("^FE^FD<#1,f,0,3#><#1,f,-1,3#><#1,f,1,0#>", options),
            field("^FD<><><>", options)
        );
        assert_eq!(
            image("^FO80,80^FE^FD<#99#>^FS", options),
            image("^FO80,80^FD<>^FS", options)
        );
    }
}
#[test]
fn backward_compatibility_changes_the_extraction_direction_independently() {
    assert_eq!(
        field("^FE^FD#1,b,1,4#", SPECIFICATION),
        field("^FDGHIJ", SPECIFICATION)
    );
    assert_eq!(
        field("^FE^FD#1,b,1,4#", ZD621_203_DPI),
        field("^FDJ", ZD621_203_DPI)
    );
    let mut options = SPECIFICATION;
    options.compatibility.concatenation_backward_reads_forward = true;
    assert_eq!(field("^FE^FD#1,b,4,3#", options), field("^FDGHI", options));
}
#[test]
fn adjacency_compatibility_does_not_leak_into_following_fields() {
    for middle in ["^FH", "^FXcomment"] {
        assert_eq!(
            field(&format!("^FE{middle}^FD#1#"), SPECIFICATION),
            field("^FD#1#", SPECIFICATION)
        );
        assert_eq!(
            field(&format!("^FE{middle}^FD#1#"), ZD621_203_DPI),
            field("^FDABCDEFGHIJ", ZD621_203_DPI)
        );
    }
    let mut options = SPECIFICATION;
    options.compatibility.concatenation_retains_delimiter = true;
    assert_eq!(
        field("^FE^FH^FD#1#", options),
        field("^FDABCDEFGHIJ", options)
    );
    for options in [SPECIFICATION, ZD621_203_DPI] {
        assert_eq!(
            field("^FE^FS^FO80,160^FD#1#", options),
            field("^FD#1#", options)
        );
    }
}
#[test]
fn printer_token_syntax_is_selectable_separately() {
    let input = "^FE^FD#1,F,2,3#|#1,f,1,-1#|A##B";
    assert_eq!(
        field(input, SPECIFICATION),
        field("^FD||A##B", SPECIFICATION)
    );
    assert_eq!(
        field(input, ZD621_203_DPI),
        field("^FDBCD|ABCDEFGHIJ|A#B", ZD621_203_DPI)
    );
    let mut options = SPECIFICATION;
    options.compatibility.concatenation_printer_syntax = true;
    assert_eq!(
        field(input, options),
        field("^FDBCD|ABCDEFGHIJ|A#B", options)
    );
    assert_eq!(
        field("^FE,^FD,1,", SPECIFICATION),
        field("^FDABCDEFGHIJ", SPECIFICATION)
    );
    assert_eq!(
        field("^FE,^FD,1,", ZD621_203_DPI),
        field("^FD,1,", ZD621_203_DPI)
    );
    assert_eq!(
        field("^FE ^FD 1 ", SPECIFICATION),
        field("^FDABCDEFGHIJ", SPECIFICATION)
    );
    assert_eq!(
        field("^FE ^FD 1 ", ZD621_203_DPI),
        field("^FD 1 ", ZD621_203_DPI)
    );
    for options in [SPECIFICATION, ZD621_203_DPI] {
        assert_eq!(field("^FE^FD<A#1>", options), field("^FD<A#1>", options));
    }
}
#[test]
fn bindings_are_resolved_in_order_and_inserted_text_is_not_reinterpreted() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        let prefix = "^FO80,40^FE^FD<#1#>^FS^FO80,80^FN1^FDAB12^FS^FO80,120^FN1^FDCD34^FS^FO80,160^FN2^FDA#1#^FS^FO80,200";
        assert_eq!(
            image(&format!("{prefix}^FE^FD#1#/#2#^FS"), options),
            image(&format!("{prefix}^FDAB12/A#1#^FS"), options)
        );
    }
}
#[test]
fn unicode_substrings_count_characters_and_hex_decoding_precedes_concatenation() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        let prefix = "^FO80,80^FN1^FDABשלוםCD^FS^FO80,160";
        assert_eq!(
            image(&format!("{prefix}^FE^FD#1,f,3,2#^FS"), options),
            image(&format!("{prefix}^FDשל^FS"), options)
        );
        assert_eq!(
            field("^FH^FE^FD_23_31_23", options),
            field("^FDABCDEFGHIJ", options)
        );
    }
}
#[test]
fn concatenated_values_keep_the_field_data_limit() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        let source = format!("^XA^FN1^FD{}^FS^FE^FD#1##1#^FS^XZ", "A".repeat(4096));
        let err = zpl::render(source.as_bytes(), options).unwrap_err();
        assert!(err.message.contains("4096"));
        assert_eq!(err.offset, source.find("^FD#1#").unwrap());
        assert!(zpl::render(b"^XA^FEab^FD#1#^FS^XZ", options).is_err());
    }
}
#[test]
fn substitution_preserves_command_bytes_as_data_and_respects_changed_prefixes() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        let prefix = "^FO80,80^FN1^FH^FD_5EFS_7EHS^FS^FO80,160";
        assert_eq!(
            image(&format!("{prefix}^FE^FD#1#^FS"), options),
            image(&format!("{prefix}^FH^FD_5EFS_7EHS^FS"), options)
        );
        let source = "^XA^PW832^LL600^CI28^CF0,32,0^FO80,80^FN1^FDAB12^FS^FO80,160^FE@^FD@1@^FS^XZ";
        let changed = format!("^CC!{}", source.replace('^', "!"));
        let a = zpl::render(source.as_bytes(), options).unwrap();
        let b = zpl::render(changed.as_bytes(), options).unwrap();
        assert_eq!(
            zpl::output::raster::rasterize(&a.labels[0]).unwrap(),
            zpl::output::raster::rasterize(&b.labels[0]).unwrap()
        );
    }
}
