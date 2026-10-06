//! Offline, exact comparisons against real ZD621 HTTP previews.
#[path = "../../zpl/tests/support/digest.rs"]
mod digest;
use raster_diff::{compare_stats, Raster};

#[test]
fn micropdf417_matches_printer_probes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/micropdf417-zd621-v1");
    let manifest = std::fs::read_to_string(root.join("manifest.tsv")).unwrap();
    let mut count = 0;
    for line in manifest.lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 3);
        let name = fields[0];
        let source = std::fs::read(root.join(format!("{name}.zpl"))).unwrap();
        let png = std::fs::read(root.join(format!("{name}.png"))).unwrap();
        assert_eq!(digest::sha256(&source), fields[1], "{name}: request");
        assert_eq!(digest::sha256(&png), fields[2], "{name}: capture");
        let printer = Raster::decode_png(&png).unwrap();
        assert!(printer.pixels.contains(&0), "{name}: blank preview");
        let doc = zpl::render(&source, zpl::render::profiles::ZD621_203_DPI).unwrap();
        let local = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = compare_stats(&printer, &local, false).unwrap();
        assert!(
            diff.matches(),
            "{name}: {} differing pixels",
            diff.different_pixels()
        );
        count += 1;
    }
    assert_eq!(count, 55);
}

#[test]
fn recaptured_micropdf417_previews_match() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/barcodes-zd621-v1");
    for name in ["micropdf417_1", "micropdf417_3", "micropdf417_4"] {
        let source = std::fs::read(root.join(format!("{name}.zpl"))).unwrap();
        let printer =
            Raster::decode_png(&std::fs::read(root.join(format!("{name}.png"))).unwrap()).unwrap();
        let doc = zpl::render(&source, zpl::render::profiles::ZD621_203_DPI).unwrap();
        let local = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        assert_eq!((local.width, local.height), (832, 1218));
        assert!(
            compare_stats(&printer, &local, false).unwrap().matches(),
            "{name}"
        );
    }
}
