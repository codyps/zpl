use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};

const FIELDS: &str = "^FO7,9^GB23,17,3^FS^FO42,14^GFA,8,8,1,80C0E0F0F8FCFEFF^FS^FO60,31^GC13,2^FS";
fn image(body: &str, options: zpl::Options) -> raster_diff::Raster {
    let source = format!("^XA^PW97^LL61{body}^XZ");
    let doc = zpl::render(source.as_bytes(), options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
}

#[test]
fn specification_mirrors_the_entire_label_and_combines_with_orientation() {
    // Zebra Programming Guide ^PM p. 319 and ^PO p. 322. PM applies to the
    // entire printable area even when specified after the fields.
    let normal = image(FIELDS, SPECIFICATION);
    for (mirror, invert) in [(false, false), (true, false), (false, true), (true, true)] {
        let body = format!(
            "{FIELDS}^PM{}^PO{}",
            if mirror { 'Y' } else { 'N' },
            if invert { 'I' } else { 'N' }
        );
        let actual = image(&body, SPECIFICATION);
        for y in 0..61 {
            for x in 0..97 {
                let sx = if mirror ^ invert { 96 - x } else { x };
                let sy = if invert { 60 - y } else { y };
                assert_eq!(actual.pixels[y * 97 + x], normal.pixels[sy * 97 + sx]);
            }
        }
    }
}

#[test]
fn mirror_preview_departure_is_independent() {
    let normal = image(FIELDS, ZD621_203_DPI);
    let body = format!("^PMY{FIELDS}");
    assert_eq!(image(&body, ZD621_203_DPI).pixels, normal.pixels);
    let mut options = ZD621_203_DPI;
    options.compatibility.preview_ignores_print_mirror = false;
    let mirrored = image(&body, options);
    assert_ne!(mirrored.pixels, normal.pixels);
    for y in 0..61 {
        for x in 0..97 {
            assert_eq!(mirrored.pixels[y * 97 + x], normal.pixels[y * 97 + 96 - x]);
        }
    }
    assert_eq!(
        image(&format!("{body}^POI"), options).pixels,
        mirrored.pixels
    );
}

#[test]
fn mirror_persists_and_ignores_missing_or_invalid_parameters() {
    // ^PM p. 319 explicitly retains the setting until PMN or power-off.
    let label = format!("^XA^PW97^LL61{FIELDS}^XZ");
    let source = format!("^PMY{label}^PM^PMX{label}^PMN{label}");
    let doc = zpl::render(source.as_bytes(), SPECIFICATION).unwrap();
    assert_eq!(doc.labels.len(), 3);
    let frames: Vec<_> = doc
        .labels
        .iter()
        .map(|s| zpl::output::raster::rasterize(s).unwrap())
        .collect();
    assert_eq!(frames[0].pixels, frames[1].pixels);
    assert_eq!(frames[2].pixels, image(FIELDS, SPECIFICATION).pixels);
    assert_ne!(frames[0].pixels, frames[2].pixels);
}

#[test]
fn mirror_support_does_not_accept_the_distinct_control_command() {
    // Format ^PM is a visual command. Control ~PM is not a rendering command.
    let error = zpl::render(b"~PManything", ZD621_203_DPI).unwrap_err();
    assert_eq!(error.message, "unsupported control command PM");
}
