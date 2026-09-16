//! Exact comparisons against pre-fix ZD621 captures at PW832 (no padding).
#[path = "../examples/font_support/mod.rs"]
mod digest;
use raster_diff::{compare, Raster};

#[test]
fn original_pdf417_captures_match_after_known_preview_padding() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/barcodes-zd621-v1");
    for name in ["pdf417", "pdf417_truncated"] {
        let source = std::fs::read(root.join(format!("{name}.zpl"))).unwrap();
        let printer =
            Raster::decode_png(&std::fs::read(root.join(format!("{name}.png"))).unwrap()).unwrap();
        let doc = zpl::render::render(&source, zpl::render::Options::default()).unwrap();
        let local = zpl::output::rasterize(&doc.labels[0]).unwrap();
        assert_eq!((printer.width, printer.height), (832, 1218));
        assert_eq!((local.width, local.height), (812, 1218));
        // Fixed experimentally established HTTP preview padding, not fitted
        // registration or content-dependent cropping/scaling.
        let mut padded = Raster {
            width: 832,
            height: 1218,
            pixels: vec![255; 832 * 1218],
        };
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

#[test]
fn pdf417_matches_printer_probes() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdf417-zd621-v1");
    let mut count = 0;
    for manifest_file in ["manifest.tsv", "sizing.tsv"] {
        let manifest = std::fs::read_to_string(root.join(manifest_file)).unwrap();
        for line in manifest.lines().skip(1) {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 4);
            let name = fields[0];
            count += 1;
            let source = std::fs::read(root.join(format!("{name}.zpl"))).unwrap();
            let png = std::fs::read(root.join(format!("{name}.png"))).unwrap();
            assert_eq!(
                digest::sha256(&source),
                fields[1],
                "{name}: request changed"
            );
            assert_eq!(
                digest::sha256(&png),
                fields[2],
                "{name}: printer capture changed"
            );
            let printer = Raster::decode_png(&png).unwrap();
            let doc = zpl::render::render(&source, zpl::render::Options::default());
            if fields[3] == "error" {
                assert!(
                    printer.pixels.iter().all(|&p| p == 255),
                    "{name}: expected blank preview"
                );
                assert!(doc
                    .unwrap_err()
                    .to_string()
                    .contains("PDF417 data does not fit"));
            } else {
                assert_eq!(fields[3], "exact");
                assert!(printer.pixels.contains(&0), "{name}: blank isn't parity");
                let actual = zpl::output::rasterize(&doc.unwrap().labels[0]).unwrap();
                let diff = compare(&printer, &actual, false).unwrap();
                assert!(
                    diff.matches(),
                    "{name}: {} differing pixels",
                    diff.different_pixels()
                );
            }
        }
    }
    assert_eq!(count, 30);
}
