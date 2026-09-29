//! Native SurePost regressions; source, capture provenance and standards in the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;

#[test]
fn surepost_label_maxicode_and_font_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/surepost-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/surepost-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let source = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&source), c[1], "{} source", c[0]);
        assert_eq!(digest::sha256(&png), c[2], "{} capture", c[0]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&source, zpl::render::profiles::ZD621_203_DPI).unwrap();
        assert_eq!(doc.labels.len(), 1);
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (c[3].parse().unwrap(), c[4].parse().unwrap()),
            "{}",
            c[0]
        );
        assert_eq!(digest::sha256(&actual.pixels), c[5], "{} raster", c[0]);
        assert!(diff.both_black > 0, "{} blank reference", c[0]);
        if c[0] == "maxicode" {
            // Independent decoder, sampled at the native 7x6 module centres.
            // The ISO/IEC 16023 decoder inserts the primary carrier fields into
            // the structured envelope; assert the remaining secondary bytes.
            let mut matrix = anyd::output::BitMatrix::new(30, 33, 0);
            for y in 0..33 {
                for x in 0..30 {
                    let px = 34 + x * 7 + (y % 2) * 3;
                    let py = 34 + y * 6;
                    matrix.set(x, y, actual.pixels[py * 832 + px] == 0);
                }
            }
            let decoded = anyd::codes::maxicode::MaxiCodeDecoder::new()
                .decode_matrix(&matrix)
                .unwrap();
            assert!(decoded.payload_bytes().ends_with(
                b"1Z00000000\x1dUPSN\x1d4X7V81\x1e07W'EEH636*N$%,Q(\x1cT3.4FQ&KAJKWR5J&Q$.:,C9F(V'G\r\x1e\x04"
            ));
        }
        if c[0].starts_with("origins-") {
            // Independent FO/FT and four-rotation holdouts. Measure foreground
            // in every cell so white margins cannot hide a text regression.
            for row in 0..2 {
                for column in 0..4 {
                    let (mut intersection, mut union) = (0, 0);
                    for y in row * 200..(row + 1) * 200 {
                        for x in column * 208..(column + 1) * 208 {
                            let i = y * 832 + x;
                            let r = reference.pixels[i] == 0;
                            let a = actual.pixels[i] == 0;
                            intersection += usize::from(r && a);
                            union += usize::from(r || a);
                        }
                    }
                    assert!(
                        union > 0 && intersection * 100 >= union * 95,
                        "{} cell {row},{column}",
                        c[0]
                    );
                }
            }
        } else {
            // The entire label, isolated MaxiCode, capacity/switching controls,
            // all sampling pages and composed verification strings are exact.
            assert!(diff.matches(), "{} must be pixel-exact", c[0]);
        }
        count += 1;
    }
    assert_eq!(count, 151);
    for (a, b) in [("label", "label-repeat"), ("maxicode", "maxicode-repeat")] {
        let decode = |name| {
            raster_diff::Raster::decode_png(&fs::read(root.join(format!("{name}.png"))).unwrap())
                .unwrap()
        };
        assert_eq!(decode(a), decode(b), "repeated {a} native pixels");
    }
}
