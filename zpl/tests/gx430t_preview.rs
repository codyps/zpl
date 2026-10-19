//! GX430t V56.17.17Z 300-DPI native HTTP previews, offline in CI.
//! Original canvases are compared without registration, cropping or rescaling.
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
use zpl::render::profiles::{GX430T_300_DPI, SPECIFICATION, ZD621_203_DPI};
#[path = "support/digest.rs"]
mod digest;
mod support;
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gx430t-300-v1")
}
fn manifest(root: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(root.join("provenance.json")).unwrap()).unwrap()
}
fn native(root: &Path, case: &serde_json::Value) -> (Vec<u8>, raster_diff::Raster) {
    let name = case["name"].as_str().unwrap();
    assert_eq!(case["status"], "captured", "{name}");
    let source = fs::read(root.join(format!("{name}.zpl"))).unwrap();
    let png = fs::read(root.join(format!("{name}.png"))).unwrap();
    assert_eq!(
        digest::sha256(&source),
        case["source_sha256"],
        "{name} source"
    );
    assert_eq!(
        digest::sha256(&png),
        case["png_sha256"],
        "{name} native PNG"
    );
    let raster = raster_diff::Raster::decode_png(&png).unwrap();
    assert_eq!(
        serde_json::json!([raster.width, raster.height]),
        case["dimensions"],
        "{name}"
    );
    (source, raster)
}
#[test]
fn main_and_independent_density_controls_are_pixel_exact() {
    for (campaign, count) in [
        ("main", 166),
        ("dpi-controls", 159),
        ("boundary-controls", 24),
        ("bitmap-edge-holdouts", 50),
        ("ocr-justification", 34),
        ("retail-captions", 50),
        ("canvas-holdouts", 11),
        ("latin-holdouts", 14),
        ("maxicode-a-runs", 16),
    ] {
        let root = root().join(campaign);
        let m = manifest(&root);
        let cases = m["cases"].as_array().unwrap();
        let baseline = fs::read_to_string(root.join("baseline.tsv")).unwrap();
        let rows: Vec<_> = baseline.lines().skip(1).collect();
        assert_eq!(cases.len(), count, "{campaign}");
        assert_eq!(rows.len(), count);
        support::map(cases, |case| {
            let name = case["name"].as_str().unwrap();
            let row: Vec<_> = rows
                .iter()
                .find(|r| r.split('\t').next() == Some(name))
                .unwrap()
                .split('\t')
                .collect();
            let n = |i: usize| row[i].parse::<usize>().unwrap();
            let (source, reference) = native(&root, case);
            let doc = zpl::render(&source, GX430T_300_DPI)
                .unwrap_or_else(|e| panic!("{campaign}/{name}: {e}"));
            assert_eq!(doc.labels.len(), 1);
            let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
            assert_eq!(
                (actual.width as usize, actual.height as usize),
                (n(1), n(2)),
                "{name}"
            );
            let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
            assert_eq!(
                (diff.reference_only, diff.candidate_only, diff.both_black),
                (0, 0, n(3)),
                "{campaign}/{name}"
            );
            assert!(diff.both_black > 0, "blank frames are diagnostics: {name}");
            assert_eq!(digest::sha256(&actual.pixels), row[4], "{name} raster");
        });
    }
}
#[test]
fn complete_conformance_preserves_geometry_and_each_text_field() {
    let root = root().join("conformance");
    let m = manifest(&root);
    let cases = m["cases"].as_array().unwrap();
    let baseline = include_str!("fixtures/gx430t-300-v1/conformance/baseline.tsv");
    let rows: Vec<_> = baseline.lines().skip(1).collect();
    let regions: Vec<_> = include_str!("fixtures/gx430t-300-v1/conformance/text-regions.tsv")
        .lines()
        .skip(1)
        .map(|s| s.split('\t').collect::<Vec<_>>())
        .collect();
    let names: BTreeSet<_> = cases.iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names.len(), 514);
    assert_eq!(rows.len(), 514);
    assert_eq!(regions.len(), 28);
    let counts = support::map(cases, |case| {
        let name = case["name"].as_str().unwrap();
        let c: Vec<_> = rows
            .iter()
            .find(|r| r.split('\t').next() == Some(name))
            .unwrap()
            .split('\t')
            .collect();
        let n = |i: usize| c[i].parse::<usize>().unwrap();
        let (source, reference) = native(&root, case);
        let result = zpl::render(&source, GX430T_300_DPI);
        let mut counts = [0usize; 6];
        if matches!(c[1], "invalid" | "unsupported") {
            let error = result.unwrap_err();
            assert_eq!(error.message, c[6], "{name}");
            assert_eq!(error.offset, n(7), "{name}");
            assert_eq!(
                reference.pixels.contains(&0),
                c[1] == "unsupported",
                "{name}"
            );
            counts[if c[1] == "invalid" { 3 } else { 4 }] = 1;
            return counts;
        }
        let doc = result.unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(doc.labels.len(), 1);
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only, diff.both_black),
            (n(2), n(3), n(4)),
            "{name}"
        );
        assert_eq!(digest::sha256(&actual.pixels), c[5], "{name} raster");
        match c[1] {
            "exact" => {
                assert_eq!((n(2), n(3)), (0, 0));
                assert!(diff.both_black > 0);
                counts[0] = 1;
            }
            "blank" => {
                assert!(!reference.pixels.contains(&0));
                assert_eq!(reference.pixels, actual.pixels);
                counts[2] = 1;
            }
            "text" => {
                let mut mask = vec![false; reference.pixels.len()];
                for r in regions.iter().filter(|r| r[0] == name) {
                    let v = |i: usize| r[i].parse::<usize>().unwrap();
                    let (left, top, right, bottom) = (v(1), v(2), v(3), v(4));
                    assert!(
                        left < right
                            && top < bottom
                            && right <= reference.width as usize
                            && bottom <= reference.height as usize
                    );
                    let (mut intersection, mut union) = (0usize, 0usize);
                    for y in top..bottom {
                        for x in left..right {
                            let i = y * reference.width as usize + x;
                            assert!(!mask[i], "overlapping fields: {name}");
                            mask[i] = true;
                            let a = (reference.pixels[i] == 0) != (v(5) == 1);
                            let b = (actual.pixels[i] == 0) != (v(5) == 1);
                            intersection += usize::from(a && b);
                            union += usize::from(a || b);
                        }
                    }
                    assert_eq!(
                        (intersection, union),
                        (v(6), v(7)),
                        "{name} field ({left},{top})"
                    );
                    // Preserve the measured stronger baseline as well as ZD621's
                    // minimum per-field foreground IoU of 14/15 (93.33%).
                    assert!(
                        union > 0 && intersection * 15 >= union * 14,
                        "{name}: {intersection}/{union}"
                    );
                    counts[5] += 1;
                }
                assert!(counts[5] > 0);
                for (i, is_text) in mask.into_iter().enumerate() {
                    if !is_text {
                        assert_eq!(
                            actual.pixels[i], reference.pixels[i],
                            "{name} non-text pixel {i}"
                        );
                    }
                }
                counts[1] = 1;
            }
            s => panic!("unknown status {s}"),
        }
        counts
    });
    let mut totals = [0; 6];
    for c in counts {
        for (a, b) in totals.iter_mut().zip(c) {
            *a += b;
        }
    }
    assert_eq!(totals, [475, 24, 9, 1, 5, 28]);
}
#[test]
fn native_campaigns_retain_identity_hashes_and_repeated_controls() {
    for entry in fs::read_dir(root()).unwrap() {
        let root = entry.unwrap().path();
        if !root.is_dir() {
            continue;
        }
        let m = manifest(&root);
        assert_eq!(m["printer"]["model"], "GX430t");
        assert_eq!(m["printer"]["serial"], "32J193600198");
        assert_eq!(m["printer"]["firmware"], "V56.17.17Z");
        assert_eq!(m["printer"]["dpi"], 300);
        assert!(m["finished"].is_string());
        let cases = m["cases"].as_array().unwrap();
        let names: BTreeSet<_> = cases.iter().map(|c| c["name"].as_str().unwrap()).collect();
        assert_eq!(names.len(), cases.len());
        for c in cases {
            native(&root, c);
        }
        if let Some(files) = m["files"].as_object() {
            for (name, hash) in files {
                assert!(Path::new(name)
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_))));
                assert_eq!(
                    digest::sha256(&fs::read(root.join(name)).unwrap()),
                    *hash,
                    "{name}"
                );
            }
        }
        assert_eq!(
            fs::read(root.join("control-start.png")).unwrap(),
            fs::read(root.join("control-end.png")).unwrap(),
            "{}",
            root.display()
        );
    }
}
#[test]
fn gx430t_strikes_are_independent_of_density_and_other_profiles() {
    let source = b"^XA^PW832^LL300^FO20,20^A0N,32,32^FDHello 123^FS^XZ";
    let raster = |options| {
        zpl::output::raster::rasterize(&zpl::render(source, options).unwrap().labels[0]).unwrap()
    };
    let mut generic = GX430T_300_DPI;
    generic.compatibility.gx430t_scalable_strikes = false;
    assert_ne!(raster(GX430T_300_DPI).pixels, raster(generic).pixels);
    assert_eq!(zpl::Options::default(), ZD621_203_DPI);
    generic = GX430T_300_DPI;
    generic.dpi = 203;
    assert_eq!(raster(GX430T_300_DPI).pixels, raster(generic).pixels);
}

#[test]
fn gx430t_departures_are_independent_options_and_clipping_obeys_limits() {
    let raster = |source: &[u8], options| {
        zpl::output::raster::rasterize(&zpl::render(source, options).unwrap().labels[0]).unwrap()
    };
    for (source, flag) in [
        (b"^XA^PW65^FO0,20^GB200,10,10^FS^XZ".as_slice(), 0),
        (
            b"^XA^PW832^FT300,300^AEI,60,38^FDABC123^FS^XZ".as_slice(),
            1,
        ),
    ] {
        let mut options = GX430T_300_DPI;
        if flag == 0 {
            options.compatibility.preview_clips_logical_width = false;
        } else {
            options.compatibility.ocr_text_uses_padded_cells = false;
        }
        assert_ne!(
            raster(source, options).pixels,
            raster(source, GX430T_300_DPI).pixels
        );
    }
    for profile in [SPECIFICATION, ZD621_203_DPI] {
        assert!(!profile.compatibility.gx430t_scalable_strikes);
        assert!(!profile.compatibility.preview_clips_logical_width);
        assert!(!profile.compatibility.ocr_text_uses_padded_cells);
        assert!(!profile.compatibility.reject_field_concatenation);
        assert!(!profile.compatibility.maxicode_a_runs_stop_at_space);
    }
    let source = b"^XA^PW65^FO0,20^GB65,10,10^FS^XZ";
    let mut limits = zpl::render::Limits::unlimited();
    limits.segments = 14;
    assert!(
        zpl::render::render_with_limits(source, GX430T_300_DPI, limits)
            .unwrap_err()
            .message
            .contains("path limit")
    );
    limits.segments = 15;
    assert!(zpl::render::render_with_limits(source, GX430T_300_DPI, limits).is_ok());
}
