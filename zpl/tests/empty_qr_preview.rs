//! Diagnostic native repeatability evidence; see the fixture README.
use std::{collections::BTreeMap, fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn identical_empty_qr_requests_have_unstable_native_previews() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/empty-qr-zd621-v1");
    let mut frames = BTreeMap::new();
    for line in include_str!("fixtures/empty-qr-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = line.split('\t').collect();
        let source = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&source), c[1]);
        assert_eq!(digest::sha256(&png), c[2]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        assert_eq!(digest::sha256(&reference.pixels), c[3]);
        frames.insert(c[0], (source, reference));
    }
    assert_eq!(frames.len(), 7);
    for names in [
        vec!["empty-isolated", "isolated-repeat1", "isolated-repeat2"],
        vec!["empty-prefix", "prefix-repeat1"],
    ] {
        for (i, a) in names.iter().enumerate() {
            for b in &names[i + 1..] {
                assert_eq!(frames[a].0, frames[b].0, "identical submitted bytes");
                assert_ne!(
                    frames[a].1.pixels, frames[b].1.pixels,
                    "unstable printer output"
                );
            }
        }
    }
    // No invented printer-buffer contents: absent data draws nothing, while
    // malformed QR data is rejected. This is not a native-pixel parity claim.
    for profile in [
        zpl::render::profiles::SPECIFICATION,
        zpl::render::profiles::ZD621_203_DPI,
    ] {
        let doc = zpl::render(&frames["empty-isolated"].0, profile).unwrap();
        let image = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        assert!(image.pixels.iter().all(|&p| p == 255));
        for name in ["empty-explicit", "empty-prefix"] {
            assert!(zpl::render(&frames[name].0, profile).is_err());
        }
    }
}
