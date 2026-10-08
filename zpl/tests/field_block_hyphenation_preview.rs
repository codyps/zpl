//! ZD621 preview regressions. Raw captures and provenance are in the fixture README.
#[path = "support/compact_font.rs"]
mod compact_font;
use std::{fs, path::Path};
#[path = "support/digest.rs"]
mod digest;
#[test]
fn printer_controls_pin_every_painted_pixel() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/field-block-hyphenation-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/field-block-hyphenation-zd621-v1/manifest.tsv")
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
        if c[0].starts_with("layout-0-") && c[0].ends_with('B') {
            assert!(diff.ink_iou() >= 0.8, "{} text IoU", c[0]);
        } else {
            assert!(diff.matches(), "{} must match the printer exactly", c[0]);
        }
        if c[0].starts_with("layout-") {
            // These four independent fields are separated vertically by more
            // than 50 white rows. Keep the full-canvas pins above and require
            // each field to pass too; neighboring good fields cannot hide one.
            let width = reference.width as usize;
            let mut fields: Vec<(usize, usize)> = Vec::new();
            for (y, (a, b)) in reference
                .pixels
                .chunks(width)
                .zip(actual.pixels.chunks(width))
                .enumerate()
            {
                if a.iter().zip(b).any(|(&a, &b)| a == 0 || b == 0) {
                    match fields.last_mut() {
                        Some((_, end)) if y <= *end + 50 => *end = y,
                        _ => fields.push((y, y)),
                    }
                }
            }
            assert_eq!(fields.len(), 4, "{} field inventory", c[0]);
            for ((start, end), align) in fields.into_iter().zip(['L', 'C', 'R', 'J']) {
                let pixels = start * width..(end + 1) * width;
                let mut both = 0;
                let mut union = 0;
                for (&a, &b) in reference.pixels[pixels.clone()]
                    .iter()
                    .zip(&actual.pixels[pixels])
                {
                    both += usize::from(a == 0 && b == 0);
                    union += usize::from(a == 0 || b == 0);
                }
                assert!(
                    union > 0 && both * 5 >= union * 4,
                    "{} {align} field IoU",
                    c[0]
                );
            }
        }
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
        if c[0] == "font-source/E-28-15/verification" || c[0] == "font-source/H-21-13/verification"
        {
            assert_eq!(diff.both_black, 0, "{} blank glyph verification", c[0]);
        } else {
            assert!(diff.both_black > 0, "{} positive control", c[0]);
        }
        count += 1;
    }
    assert_eq!(count, 61);
}

#[test]
fn native_hyphen_supplements_are_pinned_and_c_d_are_identical() {
    let mut count = 0;
    for row in include_str!("fixtures/field-block-hyphenation-zd621-v1/assets.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let bytes = compact_font::asset(c[0]).unwrap();
        assert_eq!(digest::sha256(&bytes), c[1], "{} asset", c[0]);
        let (settings, glyphs) = compact_font::decoded(c[0]).unwrap();
        assert_eq!(
            (settings.font, settings.height, settings.width, settings.dpi),
            (
                c[2].chars().next().unwrap(),
                c[3].parse().unwrap(),
                c[4].parse().unwrap(),
                203
            )
        );
        assert_eq!(
            glyphs.iter().map(|g| g.codepoint).collect::<Vec<_>>(),
            vec![173, 240]
        );
        assert!(glyphs.iter().all(|g| g.advance > 0));
        if matches!(settings.font, 'E' | 'H') {
            assert!(glyphs.iter().all(|g| g.bitmap.is_empty()));
        } else {
            assert!(glyphs.iter().all(|g| !g.bitmap.is_empty()));
        }
        count += 1;
    }
    assert_eq!(count, 15);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for name in ["page-000", "verification"] {
        let read = |font| {
            let path = root.join(format!("tests/fixtures/field-block-hyphenation-zd621-v1/font-source/{font}-18-10/{name}.png"));
            raster_diff::Raster::decode_png(&fs::read(path).unwrap()).unwrap()
        };
        assert_eq!(read('C').pixels, read('D').pixels);
    }
}
