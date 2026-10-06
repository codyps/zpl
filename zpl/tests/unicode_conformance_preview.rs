//! Unmodified ZD621 Unicode controls; see the fixture README.
use std::{fs, path::Path};
#[path = "support/digest.rs"]
mod digest;
#[test]
fn raw_unicode_frames_are_pixel_exact() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/unicode-conformance-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/unicode-conformance-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let source = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&source), c[1]);
        assert_eq!(digest::sha256(&png), c[2]);
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
        assert_eq!(&c[3..5], &["0", "0"]);
        assert_eq!(digest::sha256(&actual.pixels), c[5], "{}", c[0]);
        assert_eq!(reference.pixels.contains(&0), c[0] != "unicode-cjk");
        count += 1;
    }
    assert_eq!(count, 18);
}

fn render(data: &str, options: zpl::render::Options) -> raster_diff::Raster {
    let source = format!("^XA^PW832^LL200^CI28^FO40,40^A0N,40,24{data}^FS^XZ");
    let doc = zpl::render(source.as_bytes(), options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
}

#[test]
fn canonical_text_equivalence_preserves_barcode_payload() {
    // Unicode UAX #15, sections 1.1–1.2: canonical equivalence and NFC.
    // https://www.unicode.org/reports/tr15/
    for options in [
        zpl::render::profiles::SPECIFICATION,
        zpl::render::profiles::ZD621_203_DPI,
    ] {
        assert_eq!(
            render("^FDéÅ", options),
            render("^FDe\u{301}A\u{30a}", options)
        );
        assert_ne!(
            render("^BQN,2,3^FDLA,é", options),
            render("^BQN,2,3^FDLA,e\u{301}", options)
        );
    }
}

#[test]
fn block_format_controls_are_an_independent_printer_option() {
    use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
    let mut selected = SPECIFICATION;
    selected.compatibility.block_utf8_formatting_visible = true;
    for control in ['\u{ad}', '\u{200b}'] {
        let field = format!("^FB200,1,0,L,0^FDAB{control}CD");
        assert_eq!(
            render(&field, SPECIFICATION),
            render("^FB200,1,0,L,0^FDABCD", SPECIFICATION)
        );
        assert_ne!(render(&field, SPECIFICATION), render(&field, selected));
        assert_eq!(render(&field, selected), render(&field, ZD621_203_DPI));
        assert_eq!(
            render(&format!("^FDAB{control}CD"), selected),
            render("^FDABCD", selected)
        );
    }
}
