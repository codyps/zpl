//! Zebra Programming Guide ^PA p. 315, ^FB p. 188, ^TB p. 356:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
//! Unicode algorithm: https://www.unicode.org/reports/tr9/
use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
fn pixels(body: &str, options: zpl::Options) -> Vec<u8> {
    let source = format!("^XA^PW832^LL400^CI28^FO300,100^A0N,40,24{body}^FS^XZ");
    let doc = zpl::render(source.as_bytes(), options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0])
        .unwrap()
        .pixels
}
#[test]
fn bidi_reorders_runs_without_reversing_digits() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        assert_eq!(
            pixels("^PA0,1^FDABC שלום 123", options),
            pixels("^PA0,0^FDABC 123 םולש", options)
        );
        assert_eq!(
            pixels("^PA0,1^FDשלום ABC 123", options),
            pixels("^PA0,0^FDABC 123 םולש", options)
        );
    }
}
#[test]
fn bracket_compatibility_can_be_changed_independently() {
    let input = "^PA0,1^FDABC (שלום) 123";
    assert_eq!(
        pixels(input, SPECIFICATION),
        pixels("^PA0,0^FDABC (םולש) 123", SPECIFICATION)
    );
    assert_eq!(
        pixels(input, ZD621_203_DPI),
        pixels("^PA0,0^FDABC (123 (םולש", ZD621_203_DPI)
    );
    let mut options = SPECIFICATION;
    options.compatibility.bidi_skips_paired_bracket_resolution = true;
    assert_eq!(
        pixels(input, options),
        pixels("^PA0,0^FDABC (123 (םולש", options)
    );
}
#[test]
fn isolate_compatibility_uses_the_selected_missing_glyph() {
    let mut options = SPECIFICATION;
    let input = "^PA1,1^FDA \u{2067}ABC שלום\u{2069} Z";
    assert_eq!(
        pixels(input, options),
        pixels("^PA1,0^FDA םולש ABC Z", options)
    );
    options.compatibility.bidi_isolates_as_missing_glyphs = true;
    assert_eq!(
        pixels(input, options),
        pixels("^PA1,0^FDA ͸ABC םולש͸ Z", options)
    );
    assert_ne!(pixels(input, options), pixels(input, SPECIFICATION));
}
#[test]
fn omitted_properties_follow_the_selected_profile() {
    let input = "^PA1,1,1,1^PA^FDABC שלום ͸";
    for options in [SPECIFICATION, ZD621_203_DPI] {
        let explicit = if options.compatibility.advanced_text_omitted_flags_persist {
            "^PA1,1,1,1^FDABC שלום ͸"
        } else {
            "^PA0,0,0,0^FDABC שלום ͸"
        };
        assert_eq!(pixels(input, options), pixels(explicit, options));
    }
    let mut options = SPECIFICATION;
    options.compatibility.advanced_text_omitted_flags_persist = true;
    assert_eq!(
        pixels(input, options),
        pixels("^PA1,1,1,1^FDABC שלום ͸", options)
    );
}
#[test]
fn default_glyph_applies_only_to_captured_missing_characters() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        assert_eq!(
            pixels("^PA1^FDABC שלום", options),
            pixels("^PA0^FDABC שלום", options)
        );
        assert_ne!(pixels("^PA1^FDA͸A", options), pixels("^PA0^FDA͸A", options));
        // The Unicode conformance capture now establishes the native blank
        // fallback for U+1F600; U+1F642 remains outside the sampled repertoire.
        let source = "^XA^CI28^PA1,1,1,1^A0N,40,24^FD🙂^FS^XZ";
        assert!(zpl::render(source.as_bytes(), options).is_err());
    }
}
#[test]
fn field_blocks_ignore_bidi_but_bounded_blocks_support_it() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        assert_eq!(
            pixels("^PA1,1^FB180,5,0,L,0^FDABC שלום 123 ABC שלום 456", options),
            pixels("^PA1,0^FB180,5,0,L,0^FDABC שלום 123 ABC שלום 456", options)
        );
        assert_ne!(
            pixels("^PA1,1^TBN,180,200^FDABC שלום 123 ABC שלום 456", options),
            pixels("^PA1,0^TBN,180,200^FDABC שלום 123 ABC שלום 456", options)
        );
        assert_eq!(
            pixels("^PA1,1^TBN,180,200^FDשלום ABC 123", options),
            pixels("^FO300,100,1^PA1,1^TBN,180,200^FDשלום ABC 123", options)
        );
    }
}
