//! ^DF/^XF: https://docs.zebra.com/us/en/printers/software/zpl-pg/c-zpl-zpl-commands/r-zpl-xf.html
use zpl::render::profiles::ZD621_203_DPI;
fn pixels(source: &[u8]) -> Vec<Vec<u8>> {
    zpl::render(source, ZD621_203_DPI)
        .unwrap()
        .labels
        .iter()
        .map(|scene| zpl::output::raster::rasterize(scene).unwrap().pixels)
        .collect()
}
#[test]
fn definition_does_not_print_and_recall_binds_numbered_fields() {
    let definition = b"^XA^DFR:TEST.ZPL^FO20,20^A0N,32,0^FN1^FS^XZ";
    assert!(pixels(definition).is_empty());
    let mut source = definition.to_vec();
    source.extend_from_slice(b"^XA^XFTEST^FN1^FDSESSION-42^FS^XZ^XA^XFTEST^FN1^FDSECOND^FS^XZ");
    assert_eq!(
        pixels(&source),
        pixels(b"^XA^FO20,20^A0N,32,0^FDSESSION-42^FS^XZ^XA^FO20,20^A0N,32,0^FDSECOND^FS^XZ")
    );
}
#[test]
fn resources_are_request_local_and_errors_retain_original_offsets() {
    let input = b"^XA^DFT^FO20,20^A0N,32,0^ZZ^FS^XZ^XA^XFT^XZ";
    let error = zpl::render(input, ZD621_203_DPI).unwrap_err();
    assert_eq!(&input[error.offset..error.offset + 3], b"^ZZ");
    let error = zpl::render(b"^XA^XFT^XZ", ZD621_203_DPI).unwrap_err();
    assert!(error.message.contains("not found"));
}
#[test]
fn nested_recalls_and_search_priority() {
    let input=b"^XA^DFE:T^FO5,5^GB10,10,1^FS^XZ^XA^DFR:T^FO30,30^GB20,20,2^FS^XZ^XA^DFR:WRAP^XFT^XZ^XA^XFWRAP^XFE:T^XZ";
    assert_eq!(
        pixels(input),
        pixels(b"^XA^FO30,30^GB20,20,2^FS^FO5,5^GB10,10,1^FS^XZ")
    );
}
#[test]
fn recursive_formats_and_expansion_are_bounded() {
    let error = zpl::render(b"^XA^DFT^XFT^XZ^XA^XFT^XZ", ZD621_203_DPI).unwrap_err();
    assert!(error.message.contains("recall limit"));
}
#[test]
fn downloaded_graphic_and_stored_template_match_inline_label() {
    let input = include_bytes!("fixtures/external-stored-resources.zpl");
    let inline=b"~DGR:CMPEX.GRF,16,2,FFFF800180018001800180018001FFFF\n^XA^PW448^LL220^FO20,20^GB360,160,3,B,2^FS^FO40,45^A0N,28,15^FDStored asset^FS^FO40,90^A0N,24,13^FDSESSION-42^FS^FO380,20^XGR:CMPEX.GRF,2,2^FS^XZ";
    assert_eq!(pixels(input), pixels(inline));
    assert_eq!(pixels(input).len(), 1);
}

#[test]
fn malformed_definitions_and_outside_recalls_are_rejected() {
    for input in [
        &b"^XAextra^DFT^XZ"[..],
        b"^XA^XA^DFT^XZ^XZ",
        b"^XA^DFT^XZextra",
        b"^XA^DFT^XZ^XFT",
        b"^XA^DFT^DFA^XZ",
        b"^XA^DFT^CC! !XZ",
    ] {
        assert!(zpl::render(input, ZD621_203_DPI).is_err(), "{input:?}");
    }
}

#[test]
fn replacement_and_binary_template_data_are_preserved() {
    let source =
        b"^XA^DFT^FO4,4^GFB,1,1,1,\xff^FS^XZ^XA^XFT^XZ^XA^DFT^FO8,8^GB12,12,2^FS^XZ^XA^XFT^XZ";
    assert_eq!(
        pixels(source),
        pixels(b"^XA^FO4,4^GFB,1,1,1,\xff^FS^XZ^XA^FO8,8^GB12,12,2^FS^XZ")
    );
}
