//! Unmodified ZD621 NUL text controls; see the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn raw_nul_frames_pin_paint_counts_and_text_floor() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nul-text-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/nul-text-zd621-v1/manifest.tsv")
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
        if c[0] == "nul" {
            assert_eq!(&c[3..5], &["0", "0"]);
        }
        let both = reference
            .pixels
            .iter()
            .zip(&actual.pixels)
            .filter(|(a, b)| **a == 0 && **b == 0)
            .count();
        assert!(both * 100 >= (both + diff.reference_only + diff.candidate_only) * 80);
        assert_eq!(digest::sha256(&actual.pixels), c[5], "{}", c[0]);
        assert!(reference.pixels.contains(&0));
        count += 1;
    }
    assert_eq!(count, 3);
}

fn render(field: &str, options: zpl::render::Options) -> Result<Vec<u8>, zpl::render::RenderError> {
    let source = format!("^XA^PW832^LL200^CI28^FO40,40^A0N,32,0{field}^FS^XZ");
    let doc = zpl::render(source.as_bytes(), options)?;
    Ok(zpl::output::raster::rasterize(&doc.labels[0])
        .unwrap()
        .pixels)
}

#[test]
fn nul_termination_is_an_independent_printer_option() {
    use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
    let mut selected = SPECIFICATION;
    selected.compatibility.text_nul_processing = true;
    for block in ["", "^FB80,2,0,L,0", "^TBN,80,80"] {
        let input = format!("{block}^FH^FDAB_00CD");
        assert!(render(&input, SPECIFICATION).is_err());
        assert!(
            render(&input, selected).unwrap()
                == render(
                    &format!(
                        "{block}^FD{}",
                        if block.starts_with("^FB") {
                            "ABCD"
                        } else {
                            "AB"
                        }
                    ),
                    selected
                )
                .unwrap()
        );
        assert!(render(&input, selected).unwrap() == render(&input, ZD621_203_DPI).unwrap());
    }
    assert!(render("^FH^FD_00AB", selected)
        .unwrap()
        .iter()
        .all(|&v| v == 255));
}

#[test]
fn nul_termination_does_not_rewrite_barcode_payloads_or_later_fields() {
    use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
    let mut selected = SPECIFICATION;
    selected.compatibility.text_nul_processing = true;
    let barcode = "^BXN,3,200^FH^FDA_00B";
    assert!(render(barcode, selected).unwrap() == render(barcode, SPECIFICATION).unwrap());
    let nul = "^FH^FDAB_00CD^FS^FO100,40^FDCD";
    let plain = "^FDAB^FS^FO100,40^FDCD";
    assert!(render(nul, ZD621_203_DPI).unwrap() == render(plain, ZD621_203_DPI).unwrap());
}
