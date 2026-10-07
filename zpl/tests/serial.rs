//! ^SN initial values: Zebra Programming Guide pp. 341–342.
//! Native evidence lives in fixtures/serial-zd621-v1.
use zpl::render::{
    profiles::{SPECIFICATION, ZD621_203_DPI},
    Options,
};
fn pixels(field: &str, options: Options) -> Vec<u8> {
    let source = format!("^XA^PW832^LL300^FO100,100^AAN,18,10{field}^FS^XZ");
    let doc = zpl::render(source.as_bytes(), options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0])
        .unwrap()
        .pixels
}
#[test]
fn serial_initial_value_uses_rightmost_number_and_preserves_width() {
    for (serial, data) in [
        ("000009,1,Y", "000009"),
        ("000009,1,N", "     9"),
        ("000000,2,N", "     0"),
        ("AB12CD0034Z,-2,N", "AB12CD  34Z"),
        ("ABC,0,N", "ABC"),
        (",,", "1"),
        ("00042", "   42"),
    ] {
        assert_eq!(
            pixels(&format!("^SN{serial}"), SPECIFICATION),
            pixels(&format!("^FD{data}"), SPECIFICATION)
        );
    }
}
#[test]
fn serial_hex_is_decoded_before_zero_suppression() {
    assert_eq!(
        pixels("^FH^SN_30_30_30_39,1,N", SPECIFICATION),
        pixels("^FD   9", SPECIFICATION)
    );
    assert_eq!(
        pixels("^CD;^SN0009;1;N", SPECIFICATION),
        pixels("^FD   9", SPECIFICATION)
    );
}
#[test]
fn serial_overlong_number_requires_explicit_compatibility() {
    let source = b"^XA^FO100,100^AAN,18,10^SN0000000000009,1,N^FS^XZ";
    assert!(zpl::render(source, SPECIFICATION).is_err());
    let mut options = SPECIFICATION;
    options.compatibility.serial_overlong_keeps_value = true;
    assert_eq!(
        pixels("^SN0000000000009,1,N", options),
        pixels("^FD0000000000009", options)
    );
    assert_eq!(
        pixels("^SN0000000000009,1,N", options),
        pixels("^SN0000000000009,1,N", ZD621_203_DPI)
    );
}
#[test]
fn serial_does_not_silently_accept_print_quantity_iteration() {
    assert!(zpl::render(b"^XA^FO20,20^SN001,1,Y^FS^PQ2^XZ", SPECIFICATION).is_err());
}

#[test]
fn serial_ci13_zero_departure_is_independent() {
    let ordinary = pixels("^CI13^FD0", SPECIFICATION);
    assert_eq!(ordinary, pixels("^CI13^SN0,1,Y", SPECIFICATION));
    let mut enabled = SPECIFICATION;
    enabled.compatibility.serial_ci13_zero_uses_source = true;
    assert_eq!(
        pixels("^CI13^SN0,1,Y", enabled),
        pixels("^CI0^FD0", SPECIFICATION)
    );
    assert_ne!(pixels("^CI13^SN0,1,Y", enabled), ordinary);
    assert_eq!(pixels("^CI13^FD0", enabled), ordinary);
    assert_eq!(pixels("^CI13^FD0^SFd,1", enabled), ordinary);
    assert_eq!(pixels("^CI13,26,48^SN0,1,Y", enabled), ordinary);
    let mut disabled = ZD621_203_DPI;
    disabled.compatibility.serial_ci13_zero_uses_source = false;
    assert_eq!(pixels("^CI13^SN0,1,Y", disabled), ordinary);
}
