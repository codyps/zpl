//! Unmodified ZD621 retail caption edge controls; see the fixture README.
use std::{fs, path::Path};
#[path = "support/digest.rs"]
mod digest;
#[test]
fn raw_retail_edge_frames_are_pixel_exact() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/retail-caption-edges-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/retail-caption-edges-zd621-v1/manifest.tsv")
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
    assert_eq!(count, 5);
}

fn pixels(x: usize, options: zpl::render::Options) -> Vec<u8> {
    let source = format!("^XA^PW832^LL200^FO{x},30^BY3^BUN,40,Y,N^FD12345678901^FS^XZ");
    let doc = zpl::render(source.as_bytes(), options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0])
        .unwrap()
        .pixels
}

#[test]
fn caption_edge_override_changes_captions_without_moving_bars() {
    use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
    for mut options in [SPECIFICATION, ZD621_203_DPI] {
        // This option refines the independently selected printer retail layout.
        options.compatibility.retail_interpretation_printer_layout = true;
        options
            .compatibility
            .retail_caption_clamps_negative_inline_origin = false;
        let clipped = pixels(5, options);
        let interior = pixels(60, options);
        options
            .compatibility
            .retail_caption_clamps_negative_inline_origin = true;
        let clamped = pixels(5, options);
        assert!(clipped != clamped);
        assert_eq!(&clipped[..832 * 70], &clamped[..832 * 70]);
        assert!(interior == pixels(60, options));
    }
}
