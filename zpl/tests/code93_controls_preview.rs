//! Unmodified ZD621 Code 93 payload-control controls; see the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn raw_code93_control_frames_are_pixel_exact() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/code93-controls-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/code93-controls-zd621-v1/manifest.tsv")
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
    assert_eq!(count, 4);
}

fn render(
    data: &str,
    show: bool,
    options: zpl::render::Options,
) -> Result<Vec<u8>, zpl::render::RenderError> {
    let source = format!(
        "^XA^PW640^LL200^CI27^FO40,30^BY2^BAN,60,{},N,N^FD{data}^FS^XZ",
        if show { "Y" } else { "N" }
    );
    let doc = zpl::render(source.as_bytes(), options)?;
    Ok(zpl::output::raster::rasterize(&doc.labels[0])
        .unwrap()
        .pixels)
}

#[test]
fn control_caption_override_is_independent_and_does_not_change_bars() {
    use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
    for profile in [SPECIFICATION, ZD621_203_DPI] {
        let mut off = profile;
        off.compatibility.code93_control_interpretation = false;
        let mut on = off;
        on.compatibility.code93_control_interpretation = true;
        for data in ["X&AZ", "X'AZ", "X'UZ", "X'XZ"] {
            assert!(render(data, true, off).is_err(), "{data}");
            assert!(render(data, true, on).unwrap().contains(&0));
            assert!(render(data, false, off).unwrap() == render(data, false, on).unwrap());
        }
        assert!(render("X'WZ", true, off).unwrap() != render("X'WZ", true, on).unwrap());
        assert!(render("ABC123", true, off).unwrap() == render("ABC123", true, on).unwrap());
    }
}
