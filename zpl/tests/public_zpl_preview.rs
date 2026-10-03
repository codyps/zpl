//! October 2026 public documents, ZD621 V93.21.33Z HTTP previews.
//! Source revisions, licenses and capture state: fixtures/public-zpl-zd621-v1.
//! Zebra guide ^GF p. 215, ^BQ pp. 129–134, ^B3 pp. 70–72, ^BY p. 148,
//! ^GB p. 210: https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use std::{fs, path::Path};
use zpl::{
    output::raster::{rasterize, Raster},
    render,
    render::profiles::{SPECIFICATION, ZD621_203_DPI},
    Options,
};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;

fn raster(source: &str, options: Options) -> Raster {
    rasterize(&render(source.as_bytes(), options).unwrap().labels[0]).unwrap()
}

#[test]
fn all_public_documents_render_on_native_canvases() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/public-zpl-zd621-v1");
    let capture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/public-zpl-zd621-v1/capture.json")).unwrap();
    let cases = capture["cases"].as_array().unwrap();
    let baselines: Vec<_> = include_str!("fixtures/public-zpl-zd621-v1/baseline.tsv")
        .lines()
        .skip(1)
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .collect();
    assert_eq!(baselines.len(), 22);
    let mut count = 0;
    for c in cases.iter().filter(|c| c["name"] != "repeat-end") {
        let name = c["name"].as_str().unwrap();
        let input = fs::read(root.join(format!("{name}.zpl"))).unwrap();
        let png = fs::read(root.join(format!("{name}.png"))).unwrap();
        assert_eq!(digest::sha256(&input), c["zpl_sha256"], "{name}");
        assert_eq!(digest::sha256(&png), c["png_sha256"], "{name}");
        let reference = Raster::decode_png(&png).unwrap();
        let doc = render(&input, ZD621_203_DPI).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(doc.labels.len(), 1, "{name}");
        let actual = rasterize(&doc.labels[0]).unwrap();
        assert_eq!(actual.width as u64, c["width"].as_u64().unwrap(), "{name}");
        assert_eq!(
            actual.height as u64,
            c["height"].as_u64().unwrap(),
            "{name}"
        );
        // Native full canvases at the original origin; no alignment or resizing.
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert!(diff.both_black > 0, "{name}: nonblank positive control");
        let expected = baselines.iter().find(|row| row[0] == name).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (expected[1].parse().unwrap(), expected[2].parse().unwrap()),
            "{name}"
        );
        assert_eq!(digest::sha256(&actual.pixels), expected[3], "{name}");
        // Check the affected non-text regions at their native coordinates in
        // addition to the full-canvas baseline. Bounds include every graphic/
        // symbol dot, including clipping at the label bottom in Example6.
        let regions: &[(u32, u32, u32, u32)] = match name {
            "labelixa-carrier-style-shipping-4x6" => &[(40, 600, 348, 732)],
            "binarykits-example2-102x170" => &[(729, 1148, 801, 1295), (56, 663, 104, 879)],
            "binarykits-example4-102x152" => &[(30, 129, 262, 361), (149, 435, 152, 890)],
            "binarykits-example5-75x202" => &[(48, 516, 264, 648)],
            "binarykits-example6-75x254" => &[(190, 36, 222, 2032), (490, 36, 522, 2032)],
            "binarykits-example8-64x152" => &[(143, 356, 228, 432), (162, 781, 238, 915)],
            _ => &[],
        };
        for &(left, top, right, bottom) in regions {
            let mut ink = 0;
            for y in top..bottom {
                for x in left..right {
                    let i = (y * actual.width + x) as usize;
                    assert_eq!(
                        reference.pixels[i] < 128,
                        actual.pixels[i] < 128,
                        "{name}: affected non-text region at {x},{y}"
                    );
                    ink += usize::from(reference.pixels[i] < 128);
                }
            }
            assert!(ink > 0, "{name}: region must be nonblank");
        }
        println!(
            "{name}\t{}\t{}\t{}\t{:.6}",
            diff.reference_only,
            diff.candidate_only,
            digest::sha256(&actual.pixels),
            diff.ink_iou()
        );
        count += 1;
    }
    assert_eq!(count, 22);
}

#[test]
fn inline_graphics_finish_before_setup_and_reset_field_state() {
    for next in ["^FO30,20", "^FT30,20", "^BY3^FO30,20"] {
        for data in ["^GFA,1,1,1,80", "^GFB,1,1,1,\u{1}"] {
            let source = format!("^XA^PW80^LL80^FO5,5^FR{data}{next}^GB5,5,5^FS^XZ");
            let explicit = source.replace(next, &format!("^FS{next}"));
            assert!(render(source.as_bytes(), SPECIFICATION).is_err());
            let mut options = SPECIFICATION;
            options.compatibility.inline_graphic_implicit_separator = true;
            assert_eq!(
                raster(&source, options).pixels,
                raster(&explicit, options).pixels
            );
        }
    }
    // Missing separators on text/shapes and malformed graphic counts still fail.
    for body in [
        "^GB5,5,5^FO30,20",
        "^FDhello^FO30,20",
        "^GFA,2,2,1,80^FT30,20",
    ] {
        assert!(render(format!("^XA{body}^FS^XZ").as_bytes(), ZD621_203_DPI).is_err());
    }
    // Command framing changes are still handled by the parser, and the
    // synthetic FS must use the ordinary document segment budget.
    let changed = "^XA^CC!!FO5,5!GFA,1,1,1,80!FO30,20!GB5,5,5!FS!XZ";
    let explicit = changed.replace("!FO30", "!FS!FO30");
    assert_eq!(
        raster(changed, ZD621_203_DPI).pixels,
        raster(&explicit, ZD621_203_DPI).pixels
    );
    let limits = zpl::render::Limits {
        segments: 5,
        ..Default::default()
    };
    assert!(zpl::render::render_with_limits(changed.as_bytes(), ZD621_203_DPI, limits).is_err());
}

#[test]
fn each_public_tolerance_can_be_disabled_with_its_original_diagnostic() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/public-zpl-zd621-v1");
    for (name, flag, message) in [
        (
            "binarykits-example2-102x170",
            0,
            "drawing must end with FS before another command",
        ),
        (
            "binarykits-example4-102x152",
            1,
            "QR field requires an error-correction switch",
        ),
        (
            "binarykits-example5-75x202",
            2,
            "unsupported Code 39 character",
        ),
        (
            "binarykits-example6-75x254",
            3,
            "invalid barcode dimensions",
        ),
        (
            "binarykits-example8-64x152",
            2,
            "unsupported Code 39 character",
        ),
        ("binarykits-example4-102x152", 4, "invalid shape dimensions"),
    ] {
        let input = fs::read(root.join(format!("{name}.zpl"))).unwrap();
        let mut options = ZD621_203_DPI;
        match flag {
            0 => options.compatibility.inline_graphic_implicit_separator = false,
            1 => options.compatibility.qr_malformed_header_uses_defaults = false,
            2 => options.compatibility.code39_normalize_input = false,
            3 => options.compatibility.barcode_module_width_through_12 = false,
            _ => options.compatibility.box_zero_thickness_as_one = false,
        }
        let error = render(&input, options).unwrap_err();
        assert_eq!(error.message, message, "{name}");
        let offset = match (name, flag) {
            ("binarykits-example2-102x170", _) => 4589,
            ("binarykits-example4-102x152", 1) => 122,
            ("binarykits-example5-75x202", _) => 1177,
            ("binarykits-example6-75x254", _) => 1606,
            ("binarykits-example8-64x152", _) => 1369,
            _ => input.windows(10).position(|w| w == b"^GB0,460,0").unwrap(),
        };
        assert_eq!(error.offset, offset, "{name}");
    }
}

#[test]
fn code39_normalizes_before_checksums_and_captions() {
    for (raw, expected) in [("%s", "%S"), ("{0}", "0"), ("ab!cd", "ABCD")] {
        for checksum in ["N", "Y"] {
            let source = format!("^XA^FO30,30^BY2^B3N,{checksum},50,Y,N^FD{raw}^FS^XZ");
            assert!(render(source.as_bytes(), SPECIFICATION).is_err());
            let mut options = SPECIFICATION;
            options.compatibility.code39_normalize_input = true;
            assert_eq!(
                raster(&source, options).pixels,
                raster(&source.replace(raw, expected), options).pixels
            );
            let mut strict = ZD621_203_DPI;
            strict.compatibility.code39_normalize_input = false;
            assert!(render(source.as_bytes(), strict).is_err());
        }
    }
}

#[test]
fn malformed_qr_header_consumes_three_bytes() {
    let source = "^XA^FO30,30^BQN,2,4^FDPackage 4 in the Storage Box 2B^FS^XZ";
    assert!(render(source.as_bytes(), SPECIFICATION).is_err());
    let mut options = SPECIFICATION;
    options.compatibility.qr_malformed_header_uses_defaults = true;
    assert_eq!(
        raster(source, options).pixels,
        raster(&source.replace("Package", "MA,kage"), options).pixels
    );
    let r = raster(source, ZD621_203_DPI);
    let decoded = rxing::helpers::detect_in_luma(
        r.pixels,
        r.width,
        r.height,
        Some(rxing::BarcodeFormat::QR_CODE),
    )
    .unwrap();
    assert_eq!(decoded.getText(), "kage 4 in the Storage Box 2B");
    for data in ["Pa", "QM,B0005abc", "D0102ZZ,QA,test", "QZ,test", "QA;test"] {
        assert!(render(
            source
                .replace("Package 4 in the Storage Box 2B", data)
                .as_bytes(),
            ZD621_203_DPI
        )
        .is_err());
    }
}

#[test]
fn extended_module_width_and_zero_box_thickness_are_independent() {
    let source = "^XA^FO20,20^BY12,2.2^B3N,N,32,N,N^FD123^FS^XZ";
    assert!(render(source.as_bytes(), SPECIFICATION).is_err());
    let mut options = SPECIFICATION;
    options.compatibility.barcode_module_width_through_12 = true;
    assert!(render(source.as_bytes(), options).is_ok());
    for by in ["0,2.2", "13,2.2", "12,1.9", "12,3.1", "12,2.2,0"] {
        assert!(render(source.replace("12,2.2", by).as_bytes(), options).is_err());
    }
    let source = "^XA^FO20,20^GB0,30,0^FS^XZ";
    assert!(render(source.as_bytes(), options).is_err());
    options.compatibility.box_zero_thickness_as_one = true;
    assert_eq!(
        raster(source, options).pixels,
        raster(&source.replace("^GB0,30,0", "^GB0,30,1"), options).pixels
    );
    assert!(render(b"^XA^GC30,0^FS^XZ", options).is_err());
    assert!(render(b"^XA^GB30,30,-1^FS^XZ", options).is_err());
}
