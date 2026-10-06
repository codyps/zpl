//! Raw ZD621 structured-append controls; see the fixture README for provenance.
use std::{fs, path::Path};
#[path = "support/digest.rs"]
mod digest;

#[test]
fn structured_append_matches_printer_and_selects_native_masks() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qr-append-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/qr-append-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let input = fs::read_to_string(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(input.as_bytes()), c[1]);
        assert_eq!(digest::sha256(&png), c[2]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let render = |source: &str, automatic| {
            let mut options = zpl::render::profiles::ZD621_203_DPI;
            options.compatibility.qr_printer_mask_selection = automatic;
            let doc = zpl::render(source.as_bytes(), options).unwrap();
            zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
        };
        let actual = render(&input, true);
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert!(diff.matches(), "{} automatic mask", c[0]);
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (
                c[3].parse::<usize>().unwrap(),
                c[4].parse::<usize>().unwrap()
            ),
            "{} unchanged input",
            c[0]
        );
        assert_eq!(digest::sha256(&actual.pixels), c[5]);
        assert!(diff.both_black > 0);

        // ISO/IEC 18004:2000 §8.9, Annex C.1 and M.9: decode and BCH-check
        // captured format bits. Only replace the requested mask, never the data.
        let width = reference.width as usize;
        let ink: Vec<_> = reference
            .pixels
            .iter()
            .enumerate()
            .filter_map(|(i, &p)| (p < 128).then_some((i % width, i / width)))
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
            if reference.pixels[(top + y * 3) * width + left + x * 3] < 128 {
                format |= 1 << bit;
            }
        }
        format ^= if c[0].starts_with("m1-") {
            0x2825
        } else {
            0x5412
        };
        let mut remainder = format;
        for bit in (10..15).rev() {
            if remainder & (1 << bit) != 0 {
                remainder ^= 0x537 << (bit - 10);
            }
        }
        assert_eq!(remainder, 0, "{} captured format BCH", c[0]);
        let mask = (format >> 10) & 7;
        assert_eq!(input.matches(",7^FH").count(), 1);
        let diagnostic = input.replace(",7^FH", &format!(",{mask}^FH"));
        let diff =
            raster_diff::compare_stats(&reference, &render(&diagnostic, false), false).unwrap();
        assert!(
            diff.matches(),
            "{} structured append with captured mask",
            c[0]
        );
        count += 1;
    }
    assert_eq!(count, 18);
}
