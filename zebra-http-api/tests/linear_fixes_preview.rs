//! Fixed, original printer captures; no network calls during tests.
#[path = "../examples/font_support/mod.rs"]
mod digest;
use raster_diff::{compare, Raster};

#[test]
fn four_barcode_printer_probes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/linear-fixes-zd621-v1");
    let manifest = std::fs::read_to_string(root.join("manifest.tsv")).unwrap();
    let mut failures = Vec::new();
    let mut count = 0;
    for line in manifest.lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 3);
        let name = fields[0];
        let source = std::fs::read(root.join(format!("{name}.zpl"))).unwrap();
        let png = std::fs::read(root.join(format!("{name}.png"))).unwrap();
        assert_eq!(digest::sha256(&source), fields[1]);
        assert_eq!(digest::sha256(&png), fields[2]);
        let printer = Raster::decode_png(&png).unwrap();
        assert!(
            printer.pixels.contains(&0),
            "{name}: blank is not barcode parity"
        );
        let doc = zpl::render(&source, Default::default()).unwrap();
        let local = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = compare(&printer, &local, false).unwrap();
        if !diff.matches() {
            failures.push(format!("{name}: {} pixels", diff.different_pixels()));
        }
        count += 1;
    }
    assert_eq!(count, 39);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn recaptured_four_barcode_previews_match() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/barcodes-zd621-v1");
    for name in ["code11", "code49", "code93", "plessey"] {
        let source = std::fs::read(root.join(format!("{name}.zpl"))).unwrap();
        let png = std::fs::read(root.join(format!("{name}.png"))).unwrap();
        let printer = Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&source, Default::default()).unwrap();
        let local = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        assert_eq!((local.width, local.height), (832, 1218));
        assert!(
            compare(&printer, &local, false).unwrap().matches(),
            "{name}"
        );
    }
}
