//! Native ESC/DEL rendering; see the fixture README for command references.
use std::{fs, path::Path};
use zpl::{
    output::raster::rasterize,
    render::profiles::{SPECIFICATION, ZD621_203_DPI},
};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn native_legacy_control_frames_are_pixel_exact() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/legacy-controls-zd621-v1");
    let mut count = 0;
    for line in include_str!("fixtures/legacy-controls-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = line.split('\t').collect();
        let source = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&source), c[1]);
        assert_eq!(digest::sha256(&png), c[2]);
        assert_eq!(&c[3..5], &["0", "0"]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&source, ZD621_203_DPI).unwrap();
        assert_eq!(doc.labels.len(), 1);
        let actual = rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (0, 0),
            "{}",
            c[0]
        );
        assert_eq!(digest::sha256(&actual.pixels), c[5]);
        assert!(reference.pixels.contains(&0));
        count += 1;
    }
    assert_eq!(count, 8);
}

#[test]
fn control_option_is_independent_and_keeps_unicode_and_barcode_lookup_separate() {
    let source = b"^XA^PW832^LL300^CI13^FO20,20^A0N,32,0^FH^FDAB_1B_7FCD^FS^XZ";
    assert!(zpl::render(source, SPECIFICATION).is_err());
    let mut options = SPECIFICATION;
    options.compatibility.text_esc_del_processing = true;
    assert!(zpl::render(source, options).is_ok());
    let mut disabled = ZD621_203_DPI;
    disabled.compatibility.text_esc_del_processing = false;
    assert!(zpl::render(source, disabled).is_err());
    // C0 source bytes select legacy-only glyph keys. They must not make the
    // same Unicode scalars acquire legacy glyphs under CI28 as a side effect.
    for body in ["^CI28^A0N,32,0^FD←⌂", "^BQN,2,3^FH^FDQA,AB_1B_7FCD"] {
        let input = format!("^XA^PW832^LL300^FO20,20{body}^FS^XZ");
        let draw = |options| {
            zpl::render(input.as_bytes(), options)
                .map(|d| rasterize(&d.labels[0]).unwrap())
                .map_err(|e| e.message)
        };
        assert_eq!(draw(disabled), draw(ZD621_203_DPI));
    }
    // The captured default glyph changes missing ESC in font 0, but not the
    // independently available DEL house or bitmap-A control glyphs.
    for (font, same) in [("A,9,5", true), ("0,32,0", false)] {
        let (id, size) = font.split_once(',').unwrap();
        let input = format!("^XA^PW832^LL300^CI0^PA0,0,0,0^FO20,20^A{id}N,{size}^FH^FD_1B^FS^XZ");
        let a = zpl::render(input.as_bytes(), ZD621_203_DPI).unwrap();
        let b = zpl::render(input.replace("^PA0", "^PA1").as_bytes(), ZD621_203_DPI).unwrap();
        assert_eq!(
            rasterize(&a.labels[0]).unwrap() == rasterize(&b.labels[0]).unwrap(),
            same
        );
    }
}
