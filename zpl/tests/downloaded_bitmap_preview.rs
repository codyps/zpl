//! Native, unaligned printer comparisons. Capture identities, sessions and source
//! hashes: fixtures/downloaded-bitmap-zd621-v1/README.md and provenance.json.
use std::{fs, path::Path};
use zpl::{
    bitmap_font::{Glyph, Settings},
    fonts::Fonts,
    render::{
        profiles::{ZD621_203_DPI, ZQ610_PLUS_203_DPI},
        render_with_fonts,
    },
};
#[path = "support/digest.rs"]
mod digest;

#[test]
fn downloaded_bitmap_faces_match_saved_printer_canvases() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/downloaded-bitmap-zd621-v1");
    let bundle = fs::read(root.join("bitmap-download.zpl")).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("bundle.json")).unwrap()).unwrap();
    assert_eq!(
        digest::sha256(&bundle),
        manifest["sha256"]["bitmap-download.zpl"]
    );
    // Preserve the exact saved submissions, including their unused font-0 alias.
    // These fixtures explicitly select bitmap text. A sentinel face makes any
    // accidental font-0 text an error instead of importing an unrelated TTF.
    let mut fonts = Fonts::new();
    fonts
        .insert_named_bitmap(
            "R:FC0.TTF",
            Settings {
                font: '0',
                width: 1,
                height: 1,
                dpi: 203,
            },
            vec![Glyph {
                codepoint: 0x10ffff,
                advance: 1,
                left: 0,
                top: 0,
                width: 0,
                height: 0,
                bitmap: vec![],
            }],
            0.,
        )
        .unwrap();
    let mut failures = Vec::new();
    let mut count = 0;
    for row in include_str!("fixtures/downloaded-bitmap-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let source = root.join(c[0]);
        let submission = fs::read(&source).unwrap();
        let png = fs::read(source.with_extension("png")).unwrap();
        assert_eq!(digest::sha256(&submission), c[2], "{} submission", c[0]);
        assert_eq!(digest::sha256(&png), c[3], "{} printer PNG", c[0]);
        let mut input = bundle.clone();
        input.extend(submission);
        let profile = match c[1] {
            "zd621" => ZD621_203_DPI,
            "zq610" => ZQ610_PLUS_203_DPI,
            _ => panic!("unknown printer"),
        };
        let doc = render_with_fonts(&input, profile, &fonts).unwrap();
        assert_eq!(doc.labels.len(), 1);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        if !diff.matches() {
            failures.push(format!(
                "{}: under {} over {}",
                c[0], diff.reference_only, diff.candidate_only
            ));
        }
        count += 1;
    }
    assert_eq!(count, 100);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
