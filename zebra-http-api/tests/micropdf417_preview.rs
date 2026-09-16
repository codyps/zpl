//! Offline, exact comparisons against real ZD621 HTTP previews.
#[path = "../examples/font_support/mod.rs"]
mod digest;
use image_diff::{compare, Raster};

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
        let doc = zpl::render::render(&source, Default::default()).unwrap();
        let local = zpl::output::rasterize(&doc.labels[0]).unwrap();
        let diff = compare(&printer, &local, false).unwrap();
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
fn original_micropdf417_captures_match_with_preview_padding() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/barcodes-zd621-v1");
    for name in ["micropdf417_1", "micropdf417_3", "micropdf417_4"] {
        let source = std::fs::read(root.join(format!("{name}.zpl"))).unwrap();
        let printer =
            Raster::decode_png(&std::fs::read(root.join(format!("{name}.png"))).unwrap()).unwrap();
        let doc = zpl::render::render(&source, Default::default()).unwrap();
        let local = zpl::output::rasterize(&doc.labels[0]).unwrap();
        assert_eq!((local.width, local.height), (812, 1218));
        let mut padded = Raster {
            width: 832,
            height: 1218,
            pixels: vec![255; 832 * 1218],
        };
        // Fixed HTTP preview padding established independently, not fitted registration.
        for y in 0..1218 {
            padded.pixels[y * 832 + 10..y * 832 + 822]
                .copy_from_slice(&local.pixels[y * 812..(y + 1) * 812]);
        }
        assert!(
            compare(&printer, &padded, false).unwrap().matches(),
            "{name}"
        );
    }
}
