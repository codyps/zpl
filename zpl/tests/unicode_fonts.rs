//! Zebra Programming Guide ^CI pp. 156–159, ^PA p. 315.
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
fn pixels(field: &str) -> Vec<u8> {
    let source = format!("^XA^PW832^LL200^CI28^FO80,100^A0N,40,24{field}^FS^XZ");
    let doc = zpl::render(source.as_bytes(), SPECIFICATION).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0])
        .unwrap()
        .pixels
}
#[test]
fn advanced_defaults_and_utf8_hex_render_the_same_glyphs() {
    assert_eq!(
        pixels("^PA0,0,0,0^FDשלום"),
        pixels("^FH^FD_D7_A9_D7_9C_D7_95_D7_9D")
    );
    assert_eq!(pixels("^PA^FDשלום"), pixels("^FDשלום"));
}
#[test]
fn captured_missing_characters_use_the_native_space_advance() {
    assert_eq!(pixels("^PA0^FDAمرحباA͸"), pixels("^FDA     A "));
}
#[test]
fn invalid_advanced_properties_and_uncaptured_glyphs_are_not_silently_ignored() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        for command in ["^PA2", "^PA0,2", "^PA0,0,2", "^PA0,0,0,2", "^PA0,0,0,0,0"] {
            let source = format!("^XA{command}^FO20,20^FDABC^FS^XZ");
            assert!(zpl::render(source.as_bytes(), options).is_err());
        }
        assert!(zpl::render("^XA^CI28^FO20,20^FD😀^FS^XZ".as_bytes(), options).is_err());
    }
}
