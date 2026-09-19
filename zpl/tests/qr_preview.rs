//! ZD621 preview regressions. Raw captures and provenance are in the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn printer_controls_pin_every_painted_pixel() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qr-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/qr-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let input = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&input), c[1], "{} input", c[0]);
        assert_eq!(digest::sha256(&png), c[2], "{} printer capture", c[0]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&input, zpl::render::profiles::ZD621_203_DPI).unwrap();
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare(&reference, &actual, false).unwrap();
        assert_eq!(
            diff.reference_only,
            c[3].parse::<usize>().unwrap(),
            "{} underpaint",
            c[0]
        );
        assert_eq!(
            diff.candidate_only,
            c[4].parse::<usize>().unwrap(),
            "{} overpaint",
            c[0]
        );
        assert_eq!(
            digest::sha256(&actual.pixels),
            c[5],
            "{} local pixels",
            c[0]
        );
        assert!(
            diff.both_black > 0,
            "{}: positive control must contain ink",
            c[0]
        );
        count += 1;
    }
    assert_eq!(count, 32);
}

#[test]
fn encoding_matches_every_printer_module_with_the_captured_mask() {
    // Isolate encoding and placement from automatic mask selection. The test
    // above still renders the unchanged requests and pins their known gaps.
    // Format-bit placement: ISO/IEC 18004:2000 §8.9; BCH and format XOR masks:
    // Annex C.1 (Model 2) and Annex M.9 (Model 1).
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qr-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/qr-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let name = row.split('\t').next().unwrap();
        let input = fs::read_to_string(root.join(format!("{name}.zpl"))).unwrap();
        let png = fs::read(root.join(format!("{name}.png"))).unwrap();
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let start = input.find("^BQ").unwrap() + 3;
        let end = start + input[start..].find('^').unwrap();
        let mut operands: Vec<_> = input[start..end].split(',').collect();
        assert_eq!(operands.len(), 5);
        let scale: usize = operands[2].parse().unwrap();
        let width = reference.width as usize;
        let ink: Vec<_> = reference
            .pixels
            .iter()
            .enumerate()
            .filter_map(|(i, &pixel)| (pixel < 128).then_some((i % width, i / width)))
            .collect();
        let left = ink.iter().map(|p| p.0).min().unwrap();
        let top = ink.iter().map(|p| p.1).min().unwrap();
        let mut format = 0u16;
        for bit in 0..15 {
            let (x, y) = match bit {
                0..=5 => (8, bit),
                6 => (8, 7),
                7 => (8, 8),
                8 => (7, 8),
                _ => (14 - bit, 8),
            };
            if reference.pixels[(top + y * scale) * width + left + x * scale] < 128 {
                format |= 1 << bit;
            }
        }
        format ^= if operands[1] == "1" { 0x2825 } else { 0x5412 };
        let mut remainder = format;
        for bit in (10..15).rev() {
            if remainder & (1 << bit) != 0 {
                remainder ^= 0x537 << (bit - 10);
            }
        }
        assert_eq!(remainder, 0, "{name}: captured format BCH");
        let mask = ((format >> 10) & 7).to_string();
        operands[4] = &mask;
        let diagnostic = format!("{}{}{}", &input[..start], operands.join(","), &input[end..]);
        let doc = zpl::render(diagnostic.as_bytes(), zpl::render::profiles::ZD621_203_DPI).unwrap();
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare(&reference, &actual, false).unwrap();
        assert_eq!(diff.reference_only, 0, "{name}: encoding underpaint");
        assert_eq!(diff.candidate_only, 0, "{name}: encoding overpaint");
        count += 1;
    }
    assert_eq!(count, 32);
}
