//! Offline ZD621 controls for formerly unsupported barcode modes.
//! Capture provenance: fixtures/barcode-modes-zd621-v1/README.md.
use std::{fs, path::PathBuf};
use zpl::{output::raster::rasterize, render, render::profiles::ZD621_203_DPI};
#[path = "support/digest.rs"]
mod digest;
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/barcode-modes-zd621-v1")
}
#[test]
fn captured_barcode_modes_pin_ink_and_pixels() {
    let mut count = 0;
    for row in include_str!("fixtures/barcode-modes-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        assert_eq!(c.len(), 7);
        let input = fs::read(root().join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root().join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&input), c[2], "{} source", c[0]);
        assert_eq!(digest::sha256(&png), c[3], "{} capture", c[0]);
        let reference =
            raster_diff::Raster::decode_png(&png).unwrap_or_else(|e| panic!("{}: {e}", c[0]));
        let doc = render(&input, ZD621_203_DPI).unwrap();
        let actual = rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (c[4].parse().unwrap(), c[5].parse().unwrap()),
            "{}",
            c[0]
        );
        assert_eq!(digest::sha256(&actual.pixels), c[6], "{} pixels", c[0]);
        if c[1] != "qr-model1" {
            assert!(diff.matches(), "{}", c[0]);
        }
        count += 1;
    }
    assert_eq!(count, 159);
}

#[test]
fn model1_all_versions_match_printer_with_the_captured_mask() {
    // ISO/IEC 18004:2000 M.9: decode the mask actually present in the printer
    // symbol, since firmware ignores ^BQ's requested mask. This conformance
    // check isolates encoding/RS/placement. The unmodified input's differences
    // remain pinned by captured_barcode_modes_pin_ink_and_pixels above.
    let mut versions = std::collections::BTreeSet::new();
    for row in include_str!("fixtures/barcode-modes-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        if c[1] != "qr-model1" {
            continue;
        }
        let input = fs::read_to_string(root().join(format!("{}.zpl", c[0]))).unwrap();
        let reference = raster_diff::Raster::decode_png(
            &fs::read(root().join(format!("{}.png", c[0]))).unwrap(),
        )
        .unwrap();
        let (mut x0, mut y0, mut x1) = (reference.width, reference.height, 0);
        for y in 0..reference.height {
            for x in 0..reference.width {
                if reference.pixels[(y * reference.width + x) as usize] < 128 {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x + 1);
                }
            }
        }
        let size = (x1 - x0) / 3;
        versions.insert((size - 17) / 4);
        let mut format = 0usize;
        for i in 0..15 {
            let (x, y) = match i {
                0..=5 => (8, i),
                6 => (8, 7),
                7 => (8, 8),
                8 => (7, 8),
                _ => (14 - i, 8),
            };
            if reference.pixels[((y0 + 3 * y) * reference.width + x0 + 3 * x) as usize] < 128 {
                format |= 1 << i;
            }
        }
        let mask = ((format ^ 0x2825) >> 10) & 7;
        let start = input.find("^BQ").unwrap();
        let end = start + input[start + 1..].find('^').unwrap() + 1;
        let command = &input[start..end];
        let last = command.rfind(',').unwrap();
        let controlled = format!(
            "{}{},{}{}",
            &input[..start],
            &command[..last],
            mask,
            &input[end..]
        );
        let doc = render(controlled.as_bytes(), ZD621_203_DPI).unwrap();
        let actual = rasterize(&doc.labels[0]).unwrap();
        assert!(
            raster_diff::compare_stats(&reference, &actual, false)
                .unwrap()
                .matches(),
            "{} mask {mask}",
            c[0]
        );
    }
    assert_eq!(versions, (1..=14).collect());
}

#[test]
fn model2_mixed_segments_match_printer_with_the_captured_mask() {
    // ISO/IEC 18004:2000 §§8.2–8.4/Annex H. The immutable printer symbol
    // encodes Byte("Hello") followed by Alphanumeric(" QR 123"). As in the
    // Model 1 test, isolate compaction from the still-open automatic-mask gap.
    let input = include_str!("../../zebra-http-api/tests/fixtures/barcodes-zd621-v1/qr.zpl");
    let png = include_bytes!("../../zebra-http-api/tests/fixtures/barcodes-zd621-v1/qr.png");
    let reference = raster_diff::Raster::decode_png(png).unwrap();
    let controlled = input.replace("^BQN,2,4,L,0", "^BQN,2,4,L,7");
    assert_ne!(controlled, input);
    let doc = render(controlled.as_bytes(), ZD621_203_DPI).unwrap();
    let actual = rasterize(&doc.labels[0]).unwrap();
    assert!(raster_diff::compare_stats(&reference, &actual, false)
        .unwrap()
        .matches());
}
