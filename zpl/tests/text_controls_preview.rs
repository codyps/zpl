//! Unmodified ZD621 CR/LF/SOH text controls; see the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn raw_control_frames_pin_paint_counts_and_text_floor() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/text-controls-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/text-controls-zd621-v1/manifest.tsv")
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
        let diff = raster_diff::compare(&reference, &actual, false).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (c[3].parse().unwrap(), c[4].parse().unwrap()),
            "{}",
            c[0]
        );
        if matches!(c[0], "lf" | "cr" | "soh") {
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
        let (rows, cols, dx, dy) = if matches!(c[0], "lf" | "cr" | "soh") {
            (3, 4, 200, 180)
        } else {
            (5, 3, 250, 170)
        };
        for row in 0..rows {
            for col in 0..cols {
                let mut intersection = 0;
                let mut union = 0;
                for y in 20 + row * dy..20 + row * dy + 160 {
                    for x in 20 + col * dx..20 + col * dx + 180 {
                        let i = y * 832 + x;
                        let a = reference.pixels[i] == 0;
                        let b = actual.pixels[i] == 0;
                        intersection += usize::from(a && b);
                        union += usize::from(a || b);
                    }
                }
                assert!(
                    intersection * 100 >= union * 80,
                    "{} field {row}/{col}",
                    c[0]
                );
            }
        }
        count += 1;
    }
    assert_eq!(count, 5);
}

fn pixels(body: &str, options: zpl::render::Options) -> Result<Vec<u8>, zpl::render::RenderError> {
    let source = format!("^XA^PW832^LL240^CI28^FO40,40^A0N,32,0{body}^FS^XZ");
    let doc = zpl::render(source.as_bytes(), options)?;
    Ok(zpl::output::raster::rasterize(&doc.labels[0])
        .unwrap()
        .pixels)
}

#[test]
fn control_option_is_independent_and_layout_specific() {
    use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
    let mut selected = SPECIFICATION;
    selected.compatibility.text_control_processing = true;
    for block in ["", "^FB150,3,0,L,0"] {
        for control in ["0A", "0D"] {
            let input = format!("{block}^FH^FDAB_{control}CD");
            assert_eq!(
                pixels(&input, selected).unwrap(),
                pixels(&format!("{block}^FDAB"), selected).unwrap()
            );
        }
        let input = format!("{block}^FH^FDAB_01CD");
        let expected = if block.is_empty() { "ABCD" } else { "AB CD" };
        assert_eq!(
            pixels(&input, selected).unwrap(),
            pixels(&format!("{block}^FD{expected}"), selected).unwrap()
        );
    }
    assert!(pixels("^FH^FDAB_01CD", SPECIFICATION).is_err());
    for options in [selected, ZD621_203_DPI] {
        assert_eq!(
            pixels("^TBN,150,150^FH^FDAB_0D_0ACD", options).unwrap(),
            pixels("^TBN,150,150^FH^FDAB_0ACD", options).unwrap()
        );
        assert_ne!(
            pixels("^TBN,150,150^FH^FDAB_0ACD", options).unwrap(),
            pixels("^TBN,150,150^FDAB CD", options).unwrap()
        );
        assert_eq!(
            pixels("^TBN,150,150^FH^FDAB_01CD", options).unwrap(),
            pixels("^TBN,150,150^FDABCD", options).unwrap()
        );
    }
}

#[test]
fn controls_preserve_barcode_bytes_and_following_fields() {
    use zpl::render::profiles::ZD621_203_DPI;
    let mut disabled = ZD621_203_DPI;
    disabled.compatibility.text_control_processing = false;
    let barcode = "^BXN,3,200^FH^FDAB_01_0D_0ACD";
    assert_eq!(
        pixels(barcode, disabled).unwrap(),
        pixels(barcode, ZD621_203_DPI).unwrap()
    );
    assert_eq!(
        pixels("^FH^FDAB_0ACD^FS^FO200,40^FDCD", ZD621_203_DPI).unwrap(),
        pixels("^FDAB^FS^FO200,40^FDCD", ZD621_203_DPI).unwrap()
    );
}
