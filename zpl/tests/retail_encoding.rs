//! Native evidence and source references in fixtures/retail-font-zd621-v1/README.md.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn native_retail_encoding_controls() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/retail-font-zd621-v1/controls");
    let provenance: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("provenance.json")).unwrap()).unwrap();
    for name in ["legacy", "cp1252", "utf8", "rotations", "utf8-repeat"] {
        let source = fs::read(root.join(format!("{name}.zpl"))).unwrap();
        let png = fs::read(root.join(format!("{name}.png"))).unwrap();
        let row = provenance["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"] == name)
            .unwrap();
        assert_eq!(digest::sha256(&source), row["zpl_sha256"].as_str().unwrap());
        assert_eq!(digest::sha256(&png), row["png_sha256"].as_str().unwrap());
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&source, zpl::render::profiles::ZD621_203_DPI).unwrap();
        assert_eq!(doc.labels.len(), 1);
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert_eq!((diff.reference_only, diff.candidate_only), (0, 0), "{name}");
    }
}
