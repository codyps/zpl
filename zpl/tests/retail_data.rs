use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
fn image(command: &str, value: &str, options: zpl::Options) -> raster_diff::Raster {
    let input = format!("^XA^PW832^LL300^FO80,80^BY2,3,70^{command}N,70,N,N^FD{value}^FS^XZ");
    let doc = zpl::render(input.as_bytes(), options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
}
#[test]
fn specification_pads_and_left_truncates_to_data_width() {
    // Zebra Programming Guide pp. 83, 109, 142: 7/12/11 data digits.
    // https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
    for (command, padded, truncated, long) in [
        ("B8", "0001234", "2345677", "912345677"),
        ("BE", "000000001234", "234567890127", "91234567890127"),
        ("BU", "00000001234", "23456789017", "9123456789017"),
    ] {
        assert_eq!(
            image(command, "1234", SPECIFICATION),
            image(command, padded, SPECIFICATION)
        );
        assert_eq!(
            image(command, long, SPECIFICATION),
            image(command, truncated, SPECIFICATION)
        );
        assert_eq!(
            image(command, "", SPECIFICATION),
            image(command, "0", SPECIFICATION)
        );
    }
}
#[test]
fn native_character_coercion_is_independent() {
    let mut options = SPECIFICATION;
    options.compatibility.retail_non_digits_as_zero = true;
    assert_eq!(
        image("BE", "A12B34", options),
        image("BE", "012034", SPECIFICATION)
    );
    assert!(zpl::render(b"^XA^BEN,70,N,N^FDA12B34^FS^XZ", SPECIFICATION).is_err());
}
#[test]
fn native_supplied_check_digit_is_ignored_independently() {
    let mut options = SPECIFICATION;
    options.compatibility.retail_ignore_supplied_check_digit = true;
    assert_eq!(
        image("BE", "1234567890120", options),
        image("BE", "123456789012", SPECIFICATION)
    );
    assert_ne!(
        image("BE", "1234567890120", options),
        image("BE", "1234567890120", SPECIFICATION)
    );
}
#[test]
fn native_overlong_windows_are_independent() {
    let mut options = SPECIFICATION;
    options.compatibility.retail_printer_overlong_data = true;
    assert_eq!(
        image("B8", "912345677", options),
        image("B8", "0005677", SPECIFICATION)
    );
    assert_eq!(
        image("BE", "91234567890127", options),
        image("BE", "934567890127", SPECIFICATION)
    );
}
#[test]
fn upce_does_not_inherit_other_retail_padding() {
    assert!(zpl::render(b"^XA^B9N,70,N,N^FD1^FS^XZ", ZD621_203_DPI).is_err());
}
