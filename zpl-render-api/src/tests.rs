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

// LabelZoom conversion parameters use exact DPI and source-defined dimensions.
// https://docs.labelzoom.com/reference/conversion-parameters/#parameter-reference
#[test]
fn labelzoom_png_preserves_first_label_source_dimensions_and_exact_dpi() {
    let input = b"^XA^PW8^LL1^FO0,0^GFB,1,1,1,\x80^FS^XZ^XA^PW2^LL2^XZ";
    for dpi in [152, 203, 300, 600] {
        let mut result = render_labelzoom(input, 0, 0, dpi, false);
        assert_eq!(result.status, 200);
        assert_eq!((result.width, result.height, result.labels), (8, 1, 2));
        let body = result.take_body();
        assert_eq!(
            Raster::decode_png(&body).unwrap().pixels,
            [0, 255, 255, 255, 255, 255, 255, 255]
        );
        let reader = png::Decoder::new(Cursor::new(&body)).read_info().unwrap();
        assert_eq!(
            reader.info().pixel_dims.unwrap().xppu,
            (f64::from(dpi) / 0.0254).round() as u32
        );
        assert!(result.take_body().is_empty());
    }
    let result = render_labelzoom(input, 16, 0, 203, false);
    assert_eq!((result.status, result.width, result.height), (200, 16, 1));
    let result = render_labelzoom(input, 0, 4, 203, false);
    assert_eq!((result.status, result.width, result.height), (200, 8, 4));
    let fallback = render_labelzoom(b"^XA^XZ", 0, 0, 300, false);
    assert_eq!((fallback.width, fallback.height), (1200, 1800));
}

// PDF includes all labels in source order, with physical page sizes from DPI.
// https://docs.labelzoom.com/reference/supported-formats/#multi-label-jobs
#[test]
fn labelzoom_pdf_matches_scene_encoder_with_source_and_overridden_sizes() {
    let input = b"^XA^PW300^LL600^FO2,3^GB30,60,5^FS^XZ^XA^PW600^LL300^FO4,5^GB20,10,10^FS^XZ";
    for (width, height) in [(0, 0), (1200, 1800), (1200, 0), (0, 1800)] {
        let mut response = render_labelzoom(input, width, height, 300, true);
        assert_eq!(response.status, 200);
        assert_eq!(response.labels, 2);
        let mut expected = zpl::render(
            input,
            Options {
                dpi: 300,
                ..SPECIFICATION
            },
        )
        .unwrap();
        for scene in &mut expected.labels {
            if width != 0 {
                scene.width = width;
            }
            if height != 0 {
                scene.height = height;
            }
        }
        let body = response.take_body();
        assert_eq!(body, Pdf.encode_pages(&expected.labels).unwrap());
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("/Type /Pages /Count 2"));
        if width == 0 && height == 0 {
            assert!(text.contains("/MediaBox [0 0 72.000000000000 144.000000000000]"));
            assert!(text.contains("/MediaBox [0 0 144.000000000000 72.000000000000]"));
        }
    }
}

#[test]
fn labelzoom_conversion_limits_apply_to_every_label_and_final_viewport() {
    for pdf in [false, true] {
        for input in [
            b"^XA^PW10000^XZ".as_slice(),
            b"^XA^XZ^XA^PW10000^XZ",
            b"^XA^PW100^LL3000^XZ", // Combined with the width override below.
        ] {
            let response = render_labelzoom(input, 4000, 0, 300, pdf);
            assert_eq!(response.status, 413, "{:?}", response.body);
        }
        assert_eq!(
            render_labelzoom(b"^XA^PW3200^XZ", 0, 0, 203, pdf).status,
            413
        );
        assert_eq!(render_labelzoom(b"", 0, 0, 203, pdf).status, 400);
        assert_eq!(render_labelzoom(b"^XA^XZ", 0, 0, 304, pdf).status, 400);
        assert_eq!(render_labelzoom(b"^XA^XZ", 4097, 1, 300, pdf).status, 400);
        assert_eq!(
            render_labelzoom(b"^XA^XZ", 4096, 4096, 300, pdf).status,
            400
        );
        assert_eq!(
            render_labelzoom(&vec![b' '; MAX_INPUT_BYTES + 1], 0, 0, 203, pdf).status,
            413
        );
        assert_eq!(
            render_labelzoom(b"^XA^XZ".repeat(51).as_slice(), 0, 0, 203, pdf).status,
            413
        );
        assert_eq!(
            render_labelzoom(b"^XA^XZ^XA^ZZ^XZ", 0, 0, 203, pdf).status,
            400
        );
        let excessive = format!("^XA{}^XZ", "^FO2,2^GB10,10,10^FS".repeat(500)).repeat(50);
        assert_eq!(
            render_labelzoom(excessive.as_bytes(), 0, 0, 203, pdf).status,
            413
        );
        assert_eq!(render_labelzoom(b"^XA^XZ", 0, 0, 203, pdf).status, 200);
    }
}
