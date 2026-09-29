//! ZD621 preview regressions. Raw captures and provenance are in the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn printer_controls_pin_every_painted_pixel() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/code93-checks-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/code93-checks-zd621-v1/manifest.tsv")
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
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert!(diff.matches(), "{} must match the printer exactly", c[0]);
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
    assert_eq!(count, 23);
}

#[test]
fn exhaustive_checksum_controls_keep_their_payload_inventory() {
    use std::collections::BTreeSet;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/code93-checks-zd621-v1");
    let (mut c_values, mut k_values, mut pairs) =
        (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    let mut count = 0;
    for row in include_str!("fixtures/code93-checks-zd621-v1/checks.tsv")
        .lines()
        .skip(1)
    {
        let fields: Vec<_> = row.split('\t').collect();
        let source = fs::read_to_string(root.join(format!("{}.zpl", fields[0]))).unwrap();
        let index = fields[1].parse::<usize>().unwrap();
        let payload = source
            .split("^FD")
            .nth(index + 1)
            .unwrap()
            .split("^FS")
            .next()
            .unwrap();
        let hex: String = payload.bytes().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, fields[2], "{} field {}", fields[0], index);
        let c = fields[3].parse::<usize>().unwrap();
        let k = fields[4].parse::<usize>().unwrap();
        assert!(c < 47 && k < 47);
        if fields[0].starts_with("extended-") {
            assert!(c >= 43);
            assert!(pairs.insert((c, k)), "duplicate extended pair");
        } else {
            c_values.insert(c);
            k_values.insert(k);
        }
        count += 1;
    }
    assert_eq!(count, 282);
    assert_eq!(c_values, (0..47).collect());
    assert_eq!(k_values, (0..47).collect());
    assert_eq!(pairs.len(), 188);
}
