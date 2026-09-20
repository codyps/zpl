//! Zebra ZPL II Programming Guide ^SF, pp. 335–337:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
fn image(body: &str, options: zpl::Options) -> raster_diff::Raster {
    let source = format!("^XA^PW832^LL400^CI27^CFA,18,10^FO80,80{body}^XZ");
    let doc = zpl::render(source.as_bytes(), options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
}
#[test]
fn mask_does_not_increment_the_initial_text_or_barcode_value() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        for (value, mask) in [
            ("BL0000", "AAdddd,1"),
            ("BL00-0", "AAdd%d,1%1"),
            ("aZ79", "aNOd,B"),
            ("0fF", "dHh,1"),
            ("ABC", "AAA"),
            ("ABC", "AAA,?"),
        ] {
            assert_eq!(
                image(&format!("^FD{value}^SF{mask}^FS"), options),
                image(&format!("^FD{value}^FS"), options)
            );
        }
        assert_eq!(
            image("^BY2,3,60^BCN,60,N,N^FDAB0099^SFAAdddd,1^FS", options),
            image("^BY2,3,60^BCN,60,N,N^FDAB0099^FS", options)
        );
        assert_eq!(
            image("^FH^FDBL_30_30_30_30^SFAAdddd,1^FS", options),
            image("^FDBL0000^FS", options)
        );
    }
}
#[test]
fn masks_require_standard_field_data_and_validate_the_documented_alphabets() {
    for options in [SPECIFICATION, ZD621_203_DPI] {
        for body in [
            "^SFdddd,1^FD1234^FS",
            "^FD1234^FS^SFdddd,1",
            "^SN1234,1,Y^SFdddd,1^FS",
            "^FD1234^SFxxxx,1^FS",
            "^FD1234^SFdddd,1,2^FS",
            "^FD1234^SF,1^FS",
        ] {
            assert!(
                zpl::render(format!("^XA{body}^XZ").as_bytes(), options).is_err(),
                "{body}"
            );
        }
        let too_long = format!("^XA^FD1^SF{},1^FS^XZ", "d".repeat(3072));
        assert!(zpl::render(too_long.as_bytes(), options).is_err());
        // Subsequent-label iteration is still an explicit error, not silently
        // represented by an unchanged first label.
        assert!(zpl::render(b"^XA^FD1234^SFdddd,1^FS^PQ3^XZ", options).is_err());
    }
}
