//! Fixed ZD621 V93 contour and scaling witnesses; no printer access in tests.
//! Provenance and measured arithmetic paths are described in the fixture README.
use std::{fs, path::Path};
#[path = "support/digest.rs"]
mod digest;

#[test]
fn controlled_contours_and_scaling_preserve_native_canvases() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/truetype-raster-zd621-v1");
    let json =
        |path| -> serde_json::Value { serde_json::from_slice(&fs::read(path).unwrap()).unwrap() };
    let index = json(root.join("index.json"));
    assert_eq!(index["profile"], "ZD621_203_DPI");
    assert_eq!(index["dpi"], zpl::render::profiles::ZD621_203_DPI.dpi);
    for (name, hash) in index["files"].as_object().unwrap() {
        assert_eq!(digest::sha256(&fs::read(root.join(name)).unwrap()), *hash);
    }
    let mut pages = 0;
    for group in index["groups"].as_array().unwrap() {
        let path = root.join(group["name"].as_str().unwrap());
        let manifest = json(path.join("manifest.json"));
        let capture = json(path.join("capture.json"));
        assert_eq!(capture["status"], "complete");
        assert!(capture["cleanup_errors"].as_array().unwrap().is_empty());
        assert_eq!(capture["identity"]["device.product_name"], "ZD621");
        assert_eq!(capture["identity"]["appl.name"], "V93.21.33Z");
        let records = capture["pages"].as_array().unwrap();
        let record = |name: &str| records.iter().find(|r| r["name"] == name).unwrap();
        assert_eq!(
            record("00-control-start")["png_sha256"],
            record("99-control-end")["png_sha256"]
        );
        let bytes = fs::read(path.join("probe.ttf")).unwrap();
        let font = manifest["fonts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|font| font["object"] == group["font_object"])
            .unwrap();
        assert_eq!(digest::sha256(&bytes), font["sha256"]);
        let mut fonts = zpl::fonts::Fonts::new();
        fonts
            .insert_named_truetype(
                group["font_object"].as_str().unwrap(),
                &bytes,
                zpl::truetype::Hinting::Native,
            )
            .unwrap();
        for expected in group["pages"].as_array().unwrap() {
            pages += 1;
            let name = expected["name"].as_str().unwrap();
            let source = fs::read(path.join(format!("{name}.zpl"))).unwrap();
            let png = fs::read(path.join(format!("{name}.png"))).unwrap();
            let first = record(&format!("{name}-repeat-0"));
            let repeat = record(&format!("{name}-repeat-1"));
            assert_eq!(first["png_sha256"], repeat["png_sha256"]);
            assert_eq!(first["source_sha256"], repeat["source_sha256"]);
            assert_eq!(digest::sha256(&source), first["source_sha256"]);
            assert_eq!(digest::sha256(&png), first["png_sha256"]);
            let document = zpl::render::render_with_fonts(
                &source,
                zpl::render::profiles::ZD621_203_DPI,
                &fonts,
            )
            .unwrap();
            assert_eq!(document.labels.len(), 1);
            let candidate = zpl::output::raster::rasterize(&document.labels[0]).unwrap();
            let reference = raster_diff::Raster::decode_png(&png).unwrap();
            assert_eq!((reference.width, reference.height), (832, 832));
            assert_eq!((candidate.width, candidate.height), (832, 832));
            let diff = raster_diff::compare_stats(&reference, &candidate, false).unwrap();
            assert_eq!(
                (diff.reference_only, diff.candidate_only),
                (0, 0),
                "{path:?}/{name}: native canvas must match exactly"
            );
            assert_eq!(
                diff.reference_only as u64, expected["reference_only"],
                "{path:?}/{name}"
            );
            assert_eq!(
                diff.candidate_only as u64, expected["candidate_only"],
                "{path:?}/{name}"
            );
            let shared = reference
                .pixels
                .iter()
                .zip(&candidate.pixels)
                .filter(|(a, b)| **a == 0 && **b == 0)
                .count();
            assert_eq!(shared as u64, expected["shared_ink"]);
            assert_eq!(
                digest::sha256(&candidate.pixels),
                expected["candidate_pixels_sha256"]
            );
        }
    }
    assert_eq!(pages, 36);
}
