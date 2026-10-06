//! Unmodified ZD621 code-page controls; see the fixture README.
use std::{fs, path::Path};
#[path = "support/digest.rs"]
mod digest;
#[test]
fn raw_code_page_frames_are_pixel_exact() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/code-pages-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/code-pages-zd621-v1/manifest.tsv")
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
        assert!(reference.pixels.contains(&0));
        count += 1;
    }
    assert_eq!(count, 7);
}

fn pixels(field: &str, options: zpl::render::Options) -> Vec<u8> {
    let source = format!("^XA^PW832^LL200^PA0,0,0,0^FO40,40^A0N,40,24{field}^FS^XZ");
    let doc = zpl::render(source.as_bytes(), options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0])
        .unwrap()
        .pixels
}

#[test]
fn native_code_page_pairs_match_utf8_in_both_profiles() {
    // ^CI pp. 156–159 in the Zebra Programming Guide; raw native controls
    // and source link are in fixtures/code-pages-zd621-v1/README.md.
    for options in [
        zpl::render::profiles::SPECIFICATION,
        zpl::render::profiles::ZD621_203_DPI,
    ] {
        for (ci, bytes, unicode) in [
            (27, "_E9_A3_80", "é£€"),
            (31, "_E9_80", "é€"),
            (33, "_CF_F0_E8_E2_E5_F2_C6", "ПриветЖ"),
            (34, "_C5_EB_EB_E7_ED_E9_EA_DC_D9", "ΕλληνικάΩ"),
            (35, "_E9_F6_A3_80", "éö£€"),
            (36, "_F9_EC_E5_ED", "שלום"),
        ] {
            assert!(
                pixels(&format!("^CI{ci}^FH^FD{bytes}"), options)
                    == pixels(&format!("^CI28^FD{unicode}"), options),
                "CI{ci}"
            );
        }
    }
}

#[test]
fn code_page_selection_preserves_barcode_bytes_and_changes_between_fields() {
    use zpl::render::profiles::SPECIFICATION;
    for ci in [27, 31, 33, 34, 35, 36] {
        assert!(
            pixels(&format!("^CI{ci}^BQN,2,3^FH^FDLA,_E9_80"), SPECIFICATION)
                == pixels("^CI28^BQN,2,3^FH^FDLA,_E9_80", SPECIFICATION)
        );
    }
    let a = pixels(
        "^CI33^FH^FD_C6^FS^CI34^FO100,40^A0N,40,24^FH^FD_D9",
        SPECIFICATION,
    );
    let b = pixels("^CI28^FDЖ^FS^FO100,40^A0N,40,24^FDΩ", SPECIFICATION);
    assert!(a == b);
}
