//! ^FB pp. 186–187; native comparisons in font0-common-zd621-v1.
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
fn image(options: zpl::Options) -> raster_diff::Raster {
    let source =
        b"^XA^PW832^LL300^FO40,80^AAN,9,5^FB90,6,0,L,24^FDAB CD EF GH IJ\\&AB CD EF GH IJ^FS^XZ";
    let doc = zpl::render(source, options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
}
#[test]
fn explicit_paragraphs_reset_indent_only_when_selected() {
    let mut printer = ZD621_203_DPI;
    let native = image(printer);
    printer.compatibility.block_hard_break_resets_indent = false;
    let continuous = image(printer);
    assert_ne!(native, continuous);
    assert_eq!(continuous, image(SPECIFICATION));
    let mut specification = SPECIFICATION;
    specification.compatibility.block_hard_break_resets_indent = true;
    assert_eq!(native, image(specification));
}
