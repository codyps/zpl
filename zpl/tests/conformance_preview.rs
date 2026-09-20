//! Complete comparison corpus, with aligned native barcode replacements.
//! See fixtures/conformance-zd621-v1/README.md for provenance and field regions.
use std::{collections::BTreeSet, fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;

#[test]
fn complete_corpus_preserves_non_text_pixels_and_each_text_field_floor() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conformance-zd621-v1");
    let reset = include_bytes!("fixtures/conformance-zd621-v1/reset.zpl");
    let regions: Vec<_> = include_str!("fixtures/conformance-zd621-v1/text-regions.tsv")
        .lines()
        .skip(1)
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .collect();
    assert_eq!(regions.len(), 51);
    let (mut exact, mut text, mut blank, mut invalid, mut aligned, mut checked_regions) =
        (0, 0, 0, 0, 0, 0);
    let mut names = BTreeSet::new();
    for line in include_str!("fixtures/conformance-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = line.split('\t').collect();
        assert_eq!(c.len(), 9);
        let name = c[0];
        assert!(names.insert(name), "duplicate {name}");
        aligned += usize::from(c[1].starts_with("../barcode-aligned-zd621-v1/"));
        let source = fs::read(root.join(format!("{}.zpl", c[1]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[1]))).unwrap();
        assert_eq!(digest::sha256(&source), c[2], "{name} source");
        assert_eq!(digest::sha256(&png), c[3], "{name} printer PNG");
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let mut input = reset.to_vec();
        input.extend_from_slice(&source);
        let rendered = zpl::render(&input, zpl::render::profiles::ZD621_203_DPI);
        if c[4] == "invalid" {
            assert_eq!(name, "symbol-databar_upce");
            assert!(reference.pixels.iter().all(|&p| p == 255));
            assert_eq!(rendered.unwrap_err().message, c[8]);
            invalid += 1;
            continue;
        }
        let doc = rendered.unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(doc.labels.len(), 2, "reset plus candidate: {name}");
        let candidate = zpl::output::raster::rasterize(&doc.labels[1]).unwrap();
        let diff = raster_diff::compare(&reference, &candidate, false).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (c[5].parse().unwrap(), c[6].parse().unwrap()),
            "{name} paint"
        );
        assert_eq!(digest::sha256(&candidate.pixels), c[7], "{name} raster");
        match c[4] {
            "exact" => {
                assert_eq!((diff.reference_only, diff.candidate_only), (0, 0));
                assert!(reference.pixels.contains(&0));
                exact += 1;
            }
            "blank" => {
                assert!(reference.pixels.iter().all(|&p| p == 255));
                assert_eq!(candidate.pixels, reference.pixels);
                blank += 1;
            }
            "text" => {
                let mut text_mask = vec![false; reference.pixels.len()];
                let mut fields = 0;
                for region in regions.iter().filter(|r| r[0] == name) {
                    let n = |i: usize| region[i].parse::<usize>().unwrap();
                    let (left, top, right, bottom) = (n(1), n(2), n(3), n(4));
                    let white = n(5) == 1;
                    assert!(
                        left < right
                            && top < bottom
                            && right <= reference.width as usize
                            && bottom <= reference.height as usize
                    );
                    let (mut intersection, mut union) = (0, 0);
                    for y in top..bottom {
                        for x in left..right {
                            let i = y * reference.width as usize + x;
                            assert!(!text_mask[i], "overlapping text fields: {name}");
                            text_mask[i] = true;
                            let a = (reference.pixels[i] == 0) != white;
                            let b = (candidate.pixels[i] == 0) != white;
                            intersection += usize::from(a && b);
                            union += usize::from(a || b);
                        }
                    }
                    assert!(
                        union > 0 && intersection * 100 >= union * 80,
                        "{name} field ({left},{top}) IoU {intersection}/{union}"
                    );
                    fields += 1;
                }
                assert!(fields > 0, "missing text regions: {name}");
                checked_regions += fields;
                for (i, is_text) in text_mask.into_iter().enumerate() {
                    if !is_text {
                        assert_eq!(
                            candidate.pixels[i], reference.pixels[i],
                            "{name} non-text pixel {i}"
                        );
                    }
                }
                text += 1;
            }
            status => panic!("unknown status {status}"),
        }
    }
    assert_eq!(names.len(), 512, "do not drop corpus cases");
    assert_eq!(
        (exact, text, blank, invalid, aligned, checked_regions),
        (490, 12, 9, 1, 59, 51)
    );
}

#[test]
fn corpus_provenance_and_reset_are_pinned() {
    // The original manifest records the native firmware, source corpus digest,
    // complete capture status, reset ZPL and repeated-frame equality check.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conformance-zd621-v1");
    for (name, expected) in [
        (
            "original-capture.json",
            "b3f74c1d9766bf7a027d89f90a1ecef74b4797146e4022f9f925254135cc7de1",
        ),
        (
            "corpus.json",
            "c658da2be0b9d3df865fc56a1ee6a5719f595966539e81a08abc58ec5ab938aa",
        ),
        (
            "reset.zpl",
            "4965884100f529b45c9d32c55970d767054f1940a983ece6d89dc6ef8c505a9c",
        ),
    ] {
        assert_eq!(
            digest::sha256(&fs::read(root.join(name)).unwrap()),
            expected,
            "{name}"
        );
    }
}
