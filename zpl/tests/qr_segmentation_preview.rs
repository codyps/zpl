//! ZD621 V93.21.33Z run-merging and Labelixa regressions. Raw native requests,
//! captures, license, and provenance: fixtures/qr-segmentation-zd621-v1.
use std::{fs, path::Path};
use zpl::{
    output::raster::rasterize,
    render::profiles::{SPECIFICATION, ZD621_203_DPI},
};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;

#[test]
fn native_frames_are_pixel_exact() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qr-segmentation-zd621-v1");
    let provenance: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("provenance.json")).unwrap()).unwrap();
    for (path, hash) in provenance["files"].as_object().unwrap() {
        assert_eq!(
            digest::sha256(&fs::read(root.join(path)).unwrap()),
            hash.as_str().unwrap(),
            "{path}"
        );
    }
    assert_eq!(
        digest::sha256(include_bytes!("../assets/font0-28-0.zbf")),
        provenance["font_asset_sha256"].as_str().unwrap()
    );
    let mut count = 0;
    for row in include_str!("fixtures/qr-segmentation-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let input = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&input), c[1]);
        assert_eq!(digest::sha256(&png), c[2]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let document = zpl::render(&input, ZD621_203_DPI).unwrap();
        assert_eq!(document.labels.len(), 1);
        let actual = rasterize(&document.labels[0]).unwrap();
        assert_eq!(
            (actual.width, actual.height),
            (c[6].parse().unwrap(), c[7].parse().unwrap())
        );
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (0, 0),
            "{}",
            c[0]
        );
        assert!(diff.both_black > 0);
        assert_eq!(&c[3..5], &["0", "0"]);
        assert_eq!(digest::sha256(&actual.pixels), c[5], "{}", c[0]);
        count += 1;
    }
    assert_eq!(count, 13);
}

#[test]
fn segmentation_is_independent_and_preserves_the_payload() {
    // ISO/IEC 18004:2000 §§8.3–8.4, version dimensions and mode bitstreams:
    // https://www.iso.org/standard/30789.html
    // Zebra ^BQ automatic/manual input switches:
    // https://docs.zebra.com/us/en/printers/software/zpl-pg/zpl-commands/%5Ebq.html
    let source =
        b"^XA^PW448^LL406^FO90,60^BQN,2,8,Q,7^FDQA,https://example.com/product/48213^FS^XZ";
    for base in [SPECIFICATION, ZD621_203_DPI] {
        for enabled in [false, true] {
            let mut options = base;
            options.compatibility.qr_printer_segmentation = enabled;
            let doc = zpl::render(source, options).unwrap();
            let actual = rasterize(&doc.labels[0]).unwrap();
            let top = if options.compatibility.qr_fo_uses_by_height {
                69
            } else {
                60
            };
            let side = if enabled { 33 * 8 } else { 29 * 8 };
            let ink: Vec<_> = actual
                .pixels
                .iter()
                .enumerate()
                .filter_map(|(i, &v)| (v == 0).then_some((i % 448, i / 448)))
                .collect();
            assert_eq!(ink.iter().map(|p| p.0).min(), Some(90));
            assert_eq!(ink.iter().map(|p| p.0).max(), Some(90 + side - 1));
            assert_eq!(ink.iter().map(|p| p.1).min(), Some(top));
            assert_eq!(ink.iter().map(|p| p.1).max(), Some(top + side - 1));
            let decoded = rxing::helpers::detect_in_luma(
                actual.pixels,
                actual.width,
                actual.height,
                Some(rxing::BarcodeFormat::QR_CODE),
            )
            .unwrap();
            assert_eq!(decoded.getText(), "https://example.com/product/48213");
        }
    }
    // Manual byte mode is not subject to the automatic segmentation option.
    let manual = String::from_utf8(source.to_vec())
        .unwrap()
        .replace("QA,", "QM,B0033");
    let render = |enabled| {
        let mut options = ZD621_203_DPI;
        options.compatibility.qr_printer_segmentation = enabled;
        rasterize(&zpl::render(manual.as_bytes(), options).unwrap().labels[0]).unwrap()
    };
    assert_eq!(render(false), render(true));
    assert!(!SPECIFICATION.compatibility.qr_printer_segmentation);
    assert!(ZD621_203_DPI.compatibility.qr_printer_segmentation);
    assert!(
        zpl::Options::default()
            .compatibility
            .qr_printer_segmentation
    );
}
