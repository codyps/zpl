use super::*;
use raster_diff::Raster;
use std::io::Cursor;

// Pixel parity uses the specification profile, not a printer capture.
#[test]
fn compressed_png_preserves_pixels_resolution_and_viewport() {
    let input =
        b"^XA^PW120^LL80^FO2,3^GB100,60,5^FS^FO20,20^GB20,20,20,W^FS^FO30,30^FR^GB30,30,30^FS^XZ";
    let mut actual = render_png(input, 203, 304, 203, 0);
    assert_eq!(actual.status, 200);
    assert_eq!((actual.width, actual.height, actual.labels), (203, 304, 1));
    let body = actual.take_body();
    assert!(actual.take_body().is_empty());
    let mut document = zpl::render(
        input,
        Options {
            width: 203,
            height: 304,
            dpi: 203,
            ..SPECIFICATION
        },
    )
    .unwrap();
    let scene = &mut document.labels[0];
    scene.width = 203;
    scene.height = 304;
    assert_eq!(
        Raster::decode_png(&body).unwrap(),
        rasterize(scene).unwrap()
    );
    let reader = png::Decoder::new(Cursor::new(&body)).read_info().unwrap();
    let dimensions = reader.info().pixel_dims.unwrap();
    assert_eq!(
        (dimensions.xppu, dimensions.yppu, dimensions.unit),
        (8000, 8000, png::Unit::Meter)
    );
    assert!(body.len() < 4_000, "compressed size: {}", body.len());
}

#[test]
fn binary_input_and_label_selection_survive_the_adapter() {
    // Zebra ^GF binary data is framed by its byte count, not UTF-8 decoding.
    // https://docs.zebra.com/us/en/printers/software/zpl-pg/c-zpl-zpl-commands/r-zpl-gf.html
    let input = b"^XA^XZ^XA^FO0,0^GFB,1,1,1,\x80^FS^XZ";
    let mut result = render_png(input, 8, 1, 203, 1);
    assert_eq!(
        result.status,
        200,
        "{}",
        String::from_utf8_lossy(&result.body)
    );
    assert_eq!(result.labels, 2);
    assert_eq!(
        Raster::decode_png(&result.take_body()).unwrap().pixels,
        [0, 255, 255, 255, 255, 255, 255, 255]
    );
    let missing = render_png(input, 8, 1, 203, 2);
    assert_eq!((missing.status, missing.labels), (404, 2));
}

#[test]
fn rejects_invalid_requests_and_resource_overrides() {
    for (width, height, dpi) in [(0, 1, 203), (4097, 1, 203), (4096, 4096, 203), (1, 1, 72)] {
        assert_eq!(render_png(b"^XA^XZ", width, height, dpi, 0).status, 400);
    }
    assert_eq!(
        render_png(&vec![b' '; MAX_INPUT_BYTES + 1], 8, 8, 203, 0).status,
        413
    );
    assert_eq!(render_png(b"^XA^PW10000^XZ", 8, 8, 203, 0).status, 413);
    assert_eq!(render_png(b"^XA^LL10000^XZ", 8, 8, 203, 0).status, 413);
    assert_eq!(
        render_png(b"^XA^XZ".repeat(51).as_slice(), 8, 8, 203, 0).status,
        413
    );
    assert_eq!(render_png(b"^XA^ZZ^XZ", 8, 8, 203, 0).status, 400);
    assert_eq!(render_png(b"^XA^XZ", 8, 8, 203, 0).status, 200);
}

#[test]
fn density_classes_preserve_physical_resolution() {
    for (dpi, ppm) in [(152, 6000), (203, 8000), (304, 12000), (609, 24000)] {
        let response = render_png(b"^XA^XZ", dpi, dpi, dpi, 0);
        assert_eq!(response.status, 200);
        let reader = png::Decoder::new(Cursor::new(&response.body))
            .read_info()
            .unwrap();
        assert_eq!(reader.info().width, dpi);
        assert_eq!(reader.info().pixel_dims.unwrap().xppu, ppm);
    }
}

#[test]
fn service_keeps_field_graphic_and_format_budgets() {
    let field = format!("^XA^AAN,9,5^FD{}^FS^XZ", "A".repeat(4097));
    let graphic = format!("^XA^GFA,25001,25001,1,{}^FS^XZ", "00".repeat(25_001));
    let mut formats = "^XA^DFR:F0^XZ".to_string();
    for i in 1..9 {
        formats.push_str(&format!("^XA^DFR:F{i}^XFR:F{}^XZ", i - 1));
    }
    formats.push_str("^XA^XFR:F8^XZ");
    for source in [field, graphic, formats] {
        let result = render_png(source.as_bytes(), 100, 100, 203, 0);
        assert_eq!(
            result.status,
            413,
            "{}",
            String::from_utf8_lossy(&result.body)
        );
    }
}
