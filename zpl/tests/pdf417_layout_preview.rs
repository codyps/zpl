//! Native ZD621 V93.21.33Z controls; fixture README records provenance and specs.
use std::{fs, path::Path};
use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;

const FIXTURES: &str = "tests/fixtures/pdf417-layout-zd621-v1";

#[test]
fn integer_layout_and_punctuation_match_every_printer_pixel() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURES);
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let cases = manifest["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 60);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let input = fs::read(root.join(format!("{name}.zpl"))).unwrap();
        let png = fs::read(root.join(format!("{name}.png"))).unwrap();
        assert_eq!(digest::sha256(&input), case["zpl_sha256"], "{name}");
        assert_eq!(digest::sha256(&png), case["png_sha256"], "{name}");
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&input, ZD621_203_DPI).unwrap();
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        assert_eq!((reference.width, reference.height), (832, 1218));
        assert_eq!((actual.width, actual.height), (832, 1218));
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert!(
            diff.both_black > 0,
            "{name}: blank is not positive evidence"
        );
        assert_eq!((diff.reference_only, diff.candidate_only), (0, 0), "{name}");
    }
    assert_eq!(
        fs::read(root.join("carrier.png")).unwrap(),
        fs::read(root.join("repeat.png")).unwrap()
    );
}

#[test]
fn carrier_and_punctuation_holdouts_decode_without_rewriting_the_payload() {
    // USS PDF417 §2.2.4.4: Punctuation latches must preserve payload bytes,
    // including transitions back to Alpha, Mixed, Lower and space.
    let carrier = "[)>_1E01_1D9697477_1D840_1D001_1D420974771Z9999AA10123456784";
    for payload in [
        carrier,
        "abc[)>_DEF",
        "123[)>_ 456",
        "[)>_lower",
        "[)>_",
        "[)>_!\u{1}abcde",
        "[)>_!0123456789012345ABC",
    ] {
        let field: String = payload.bytes().map(|v| format!("_{v:02X}")).collect();
        let input = format!("^XA^PW832^LL400^FO40,40^BY2^B7N,6,5^FH^FD{field}^FS^XZ");
        for profile in [SPECIFICATION, ZD621_203_DPI] {
            let doc = zpl::render(input.as_bytes(), profile).unwrap();
            let raster = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
            let result = rxing::helpers::detect_in_luma(
                raster.pixels,
                raster.width,
                raster.height,
                Some(rxing::BarcodeFormat::PDF_417),
            )
            .unwrap();
            assert_eq!(result.getText(), payload);
        }
    }
}

#[test]
fn layout_and_compaction_overrides_are_independent() {
    let input = include_bytes!("fixtures/pdf417-layout-zd621-v1/carrier.zpl");
    let render = |profile| {
        zpl::output::raster::rasterize(&zpl::render(input, profile).unwrap().labels[0]).unwrap()
    };
    let printer = render(ZD621_203_DPI);
    for disable_layout in [false, true] {
        let mut profile = ZD621_203_DPI;
        if disable_layout {
            profile.compatibility.pdf417_integer_grid_layout = false;
        } else {
            profile.compatibility.pdf417_punctuation_latches = false;
        }
        assert_ne!(printer.pixels, render(profile).pixels);
    }
    let mut profile = ZD621_203_DPI;
    profile.compatibility.pdf417_integer_grid_layout = false;
    profile.compatibility.pdf417_punctuation_latches = false;
    assert_eq!(render(profile).pixels, render(SPECIFICATION).pixels);
}
