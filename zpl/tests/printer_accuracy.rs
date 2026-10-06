//! Offline ZD621 preview regressions. Capture provenance and source hashes are in
//! fixtures/printer-accuracy/provenance.json; see docs/printer-accuracy.md.
use std::{fs, path::Path};
#[path = "support/digest.rs"]
mod digest;
mod support;

#[test]
fn printer_accuracy() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/printer-accuracy");
    let artifacts = std::env::var_os("ZPL_ACCURACY_ARTIFACTS").map(std::path::PathBuf::from);
    if let Some(dir) = &artifacts {
        fs::create_dir_all(dir).unwrap();
    }
    let rows: Vec<_> = include_str!("fixtures/printer-accuracy/baseline.tsv")
        .lines()
        .filter(|row| !row.starts_with('#'))
        .collect();
    let results = support::map(&rows, |row| {
        let mut failures = Vec::new();
        let summary;
        let cols: Vec<_> = row.split('\t').collect();
        assert_eq!(cols.len(), 12);
        let name = cols[0];
        let n = |i: usize| cols[i].parse::<usize>().unwrap();
        let fixture = if let Some(barcode) = name.strip_prefix("barcode-") {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../zebra-http-api/tests/fixtures/barcodes-zd621-v1")
                .join(barcode)
        } else {
            root.join(name)
        };
        let input = fs::read(fixture.with_extension("zpl")).unwrap();
        let png = fs::read(fixture.with_extension("png")).unwrap();
        assert_eq!(digest::sha256(&input), cols[8], "{name}: source changed");
        assert_eq!(digest::sha256(&png), cols[9], "{name}: capture changed");
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let document = zpl::render(
            &input,
            zpl::Options {
                width: n(1) as u32,
                height: n(2) as u32,
                ..zpl::render::profiles::ZD621_203_DPI
            },
        );
        match document {
            Err(error) => {
                summary = format!("{name}: ERROR {error}");
                if cols[3] != "error" || error.to_string() != cols[11] {
                    failures.push(format!("{name}: unexpected render error: {error}"));
                }
            }
            Ok(document) => {
                assert_eq!(document.labels.len(), 1, "{name}");
                let candidate = zpl::output::raster::rasterize(&document.labels[0]).unwrap();
                // Same origin, threshold 128, no padding, alignment or cropping.
                let diff = raster_diff::compare_stats(&reference, &candidate, false).unwrap();
                if let Some(dir) = &artifacts {
                    if !diff.matches() {
                        fs::write(
                            dir.join(format!("{name}-render.png")),
                            raster_diff::Png::encode_gray(&candidate, 203).unwrap(),
                        )
                        .unwrap();
                        fs::write(
                            dir.join(format!("{name}-diff.png")),
                            raster_diff::compare(&reference, &candidate, false)
                                .unwrap()
                                .png(1)
                                .unwrap(),
                        )
                        .unwrap();
                    }
                }
                let actual = (
                    diff.reference_only,
                    diff.candidate_only,
                    candidate.width as usize,
                    candidate.height as usize,
                );
                summary = format!(
                    "{name}: under={} over={} size={}x{} IoU={:.6}",
                    actual.0,
                    actual.1,
                    actual.2,
                    actual.3,
                    diff.ink_iou()
                );
                let hash = digest::sha256(&candidate.pixels);
                if cols[3] != "rendered" || actual != (n(4), n(5), n(6), n(7)) || hash != cols[10] {
                    failures.push(format!(
                        "{name}: expected under/over/width/height {:?}, got {actual:?} (status {}); expected pixel hash {}, got {hash}",
                        (n(4), n(5), n(6), n(7)),
                        cols[3], cols[10]
                    ));
                }
            }
        }
        (summary, failures)
    });
    let mut failures = Vec::new();
    for (summary, errors) in results {
        println!("{summary}");
        failures.extend(errors);
    }
    assert_eq!(rows.len(), 164, "do not silently drop printer cases");
    assert!(
        failures.is_empty(),
        "accuracy baseline changed:\n{}",
        failures.join("\n")
    );
}
