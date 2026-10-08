//! Native supplied-font captures; see fixtures/external-fonts-zd621-v1/README.md.
//! ZPL field origins and scalable-font cells: ^FO and Table 29, pp. 201/1582:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use std::{fs, path::Path};
#[path = "support/digest.rs"]
mod digest;

#[test]
fn supplied_truetype_preserves_native_origin_and_exact_residuals() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/external-fonts-zd621-v1");
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["sessions"][0]["identity"]["firmware"],
        "V93.21.33Z"
    );
    assert_eq!(manifest["sessions"][0]["repeat_pixels_equal"], true);
    assert_eq!(manifest["sessions"][0]["restored_pixels_equal"], true);
    assert_eq!(manifest["dpi"], 203);
    for (name, hash) in manifest["files"].as_object().unwrap() {
        assert_eq!(
            digest::sha256(&fs::read(root.join(name)).unwrap()),
            hash.as_str().unwrap()
        );
    }
    let data = fs::read(root.join("ComparisonHerosCondensedBold.ttf")).unwrap();
    let mut fonts = zpl::fonts::Fonts::new();
    fonts
        .insert_truetype('0', &data, zpl::truetype::Hinting::Native)
        .unwrap();
    let cases = manifest["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 14);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let source = fs::read(root.join(format!("{name}.zpl"))).unwrap();
        let png = fs::read(root.join(format!("{name}.png"))).unwrap();
        let submitted = fs::read(root.join(format!("{name}.submitted.zpl"))).unwrap();
        assert_eq!(digest::sha256(&source), case["source_sha256"]);
        assert_eq!(digest::sha256(&png), case["png_sha256"]);
        assert_eq!(digest::sha256(&submitted), case["submitted_sha256"]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc =
            zpl::render::render_with_fonts(&source, zpl::render::profiles::ZD621_203_DPI, &fonts)
                .unwrap();
        assert_eq!(doc.labels.len(), 1);
        let candidate = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        assert_eq!(
            (candidate.width, candidate.height),
            (reference.width, reference.height),
            "{name}"
        );
        assert_eq!(
            (reference.width as u64, reference.height as u64),
            (
                case["printer_dimensions"][0].as_u64().unwrap(),
                case["printer_dimensions"][1].as_u64().unwrap()
            )
        );
        let diff = raster_diff::compare_stats(&reference, &candidate, false).unwrap();
        assert_eq!(
            (diff.reference_only as u64, diff.candidate_only as u64),
            (
                case["reference_only"].as_u64().unwrap(),
                case["candidate_only"].as_u64().unwrap()
            ),
            "{name}"
        );
        let shared = reference
            .pixels
            .iter()
            .zip(&candidate.pixels)
            .filter(|(a, b)| **a == 0 && **b == 0)
            .count();
        assert_eq!(
            shared as u64,
            case["shared_ink"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            digest::sha256(&candidate.pixels),
            case["candidate_pixels_sha256"],
            "{name}"
        );
    }
}
