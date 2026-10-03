//! Full-canvas barcode accuracy against saved model-specific HTTP previews.
//! See docs/barcode-accuracy.md for the corpus and known-gap contract.
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};
use zpl::parse::{Element, ParseContext};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
mod support;

// Zebra ZPL Programming Guide, barcode commands ^B0–^BZ, pp. 64–150.
// https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
const COMMANDS: &[&str] = &[
    "B0", "B1", "B2", "B3", "B4", "B5", "B7", "B8", "B9", "BA", "BB", "BC", "BD", "BE", "BF", "BI",
    "BJ", "BK", "BL", "BM", "BO", "BP", "BQ", "BR", "BS", "BT", "BU", "BX", "BZ",
];

fn sources(root: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "zpl") {
            out.push(path);
        }
    }
}

fn commands(input: &[u8]) -> BTreeSet<String> {
    ParseContext::from_bytes(input)
        .filter_map(|element| match element.unwrap() {
            Element::FormatCommand(bytes) if bytes.len() >= 3 => {
                let name = std::str::from_utf8(&bytes[1..3]).ok()?;
                COMMANDS.contains(&name).then(|| name.to_owned())
            }
            _ => None,
        })
        .collect()
}

// Foreground IoU must be strictly above 80%; an empty union proves nothing.
fn within_budget(both: usize, under: usize, over: usize) -> bool {
    5 * (under + over) < both + under + over
}

struct Observation {
    name: String,
    codes: BTreeSet<String>,
    reference_ink: usize,
    profile_name: &'static str,
    status: &'static str,
    both: usize,
    under: usize,
    over: usize,
    diagnostic: String,
    source_hash: String,
    printer_hash: String,
    width: u32,
    height: u32,
    local_size: String,
    local_hash: String,
}

#[test]
fn barcode_reference_coverage_and_error_budget() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut paths = Vec::new();
    for directory in ["zpl/tests/fixtures", "zebra-http-api/tests/fixtures"] {
        sources(&root.join(directory), &mut paths);
    }
    paths.sort();
    // Exceptions are individual immutable observations, never a whole family.
    // Strict mode reports the unmet target even for these pinned known gaps.
    let strict = std::env::var_os("ZPL_BARCODE_STRICT").is_some();
    let mut exceptions = BTreeMap::new();
    for row in include_str!("fixtures/barcode-accuracy-gaps.tsv")
        .lines()
        .filter(|row| !row.starts_with('#') && !row.is_empty())
    {
        let mut columns = row.splitn(4, '\t');
        let name = columns.next().unwrap();
        let kind = columns.next().unwrap();
        let reason = columns.next().unwrap();
        let observation = columns.next().unwrap();
        assert!(matches!(kind, "diagnostic" | "text" | "gap"));
        assert!(!reason.is_empty());
        assert!(
            exceptions
                .insert(name, (kind, reason, observation))
                .is_none(),
            "duplicate exception: {name}"
        );
    }
    let mut used_exceptions = BTreeSet::new();
    let mut covered: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut failures = Vec::new();
    let artifacts = std::env::var_os("ZPL_BARCODE_ARTIFACTS").map(PathBuf::from);
    if let Some(dir) = &artifacts {
        fs::create_dir_all(dir).unwrap();
    }
    let report_path = std::env::var_os("ZPL_BARCODE_REPORT");
    let mut report = report_path.as_ref().map(|_| String::from("path\tcommands\tinput_sha256\tprinter_sha256\tstatus\tintersection\tunderpaint\toverpaint\terror_percent\tdiagnostic\tprofile\treference_size\tlocal_size\tlocal_pixel_sha256\tclassification\treason\n"));
    let (mut count, mut positive, mut over_budget, mut blank, mut diagnostics) = (0, 0, 0, 0, 0);
    let observations = support::map(&paths, |path| {
        let input = fs::read(path).unwrap();
        let codes = commands(&input);
        if codes.is_empty() {
            return None;
        }
        let name = path.strip_prefix(root).unwrap().to_str().unwrap();
        // Exact comparisons use bytes. Hash only persisted observations: a
        // requested report or an immutable exception in the gaps manifest.
        let needs_hashes = report_path.is_some() || exceptions.contains_key(name);
        let png_path = path.with_extension("png");
        assert!(png_path.is_file(), "{name}: missing printer comparison");
        let png = fs::read(&png_path).unwrap();
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let reference_ink = reference.pixels.iter().filter(|&&p| p < 128).count();
        let (profile_name, profile) = if name.starts_with("zpl/tests/fixtures/zq610-plus-v1/") {
            (
                "ZQ610_PLUS_203_DPI",
                zpl::render::profiles::ZQ610_PLUS_203_DPI,
            )
        } else {
            assert!(
                name.contains("-zd621-v1/")
                    || name.starts_with("zpl/tests/fixtures/printer-accuracy/"),
                "{name}: assign an explicit capture profile before adding this corpus"
            );
            ("ZD621_203_DPI", zpl::render::profiles::ZD621_203_DPI)
        };
        // The comparison campaign submitted a separate reset before each frame.
        let mut replay = Vec::new();
        let reset = name.starts_with("zpl/tests/fixtures/conformance-zd621-v1/")
            || name.starts_with("zebra-http-api/tests/fixtures/barcodes-zd621-v1/");
        if reset {
            replay.extend_from_slice(include_bytes!("fixtures/conformance-zd621-v1/reset.zpl"));
        }
        replay.extend_from_slice(&input);
        let rendered = zpl::render(&replay, profile);
        let (mut local_size, mut local_hash) = ("-".to_owned(), "-".to_owned());
        let (status, both, under, over, diagnostic) = match rendered {
            Err(error) => ("render-error", 0, reference_ink, 0, error.to_string()),
            Ok(doc) => {
                assert_eq!(
                    doc.labels.len(),
                    1 + usize::from(reset),
                    "{name}: expected one candidate label"
                );
                let actual = zpl::output::raster::rasterize(doc.labels.last().unwrap()).unwrap();
                local_size = format!("{}x{}", actual.width, actual.height);
                if needs_hashes {
                    local_hash = digest::sha256(&actual.pixels);
                }
                if (reference.width, reference.height) != (actual.width, actual.height) {
                    (
                        "size-mismatch",
                        0,
                        reference_ink,
                        actual.pixels.iter().filter(|&&p| p < 128).count(),
                        format!(
                            "reference {}x{}; local {}x{}",
                            reference.width, reference.height, actual.width, actual.height
                        ),
                    )
                } else if reference.pixels == actual.pixels {
                    // Most controls are exact. Avoid allocating a full RGB diff
                    // for an identical canvas; the foreground counts are exact.
                    (
                        if reference_ink == 0 {
                            "blank-reference"
                        } else {
                            "rendered"
                        },
                        reference_ink,
                        0,
                        0,
                        String::new(),
                    )
                } else {
                    let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
                    if !diff.matches() {
                        if let Some(dir) = &artifacts {
                            let stem = name.replace('/', "__").replace(".zpl", "");
                            fs::write(
                                dir.join(format!("{stem}-render.png")),
                                raster_diff::Png::encode_gray(&actual, 203).unwrap(),
                            )
                            .unwrap();
                            fs::write(
                                dir.join(format!("{stem}-diff.png")),
                                raster_diff::compare(&reference, &actual, false)
                                    .unwrap()
                                    .png(1)
                                    .unwrap(),
                            )
                            .unwrap();
                        }
                    }
                    (
                        if reference_ink == 0 {
                            "blank-reference"
                        } else {
                            "rendered"
                        },
                        diff.both_black,
                        diff.reference_only,
                        diff.candidate_only,
                        String::new(),
                    )
                }
            }
        };
        Some(Observation {
            name: name.to_owned(),
            codes,
            reference_ink,
            profile_name,
            status,
            both,
            under,
            over,
            diagnostic,
            source_hash: if needs_hashes {
                digest::sha256(&input)
            } else {
                String::new()
            },
            printer_hash: if needs_hashes {
                digest::sha256(&png)
            } else {
                String::new()
            },
            width: reference.width,
            height: reference.height,
            local_size,
            local_hash,
        })
    });
    for observation in observations.into_iter().flatten() {
        let Observation {
            name,
            codes,
            reference_ink,
            profile_name,
            status,
            both,
            under,
            over,
            diagnostic,
            source_hash,
            printer_hash,
            width,
            height,
            local_size,
            local_hash,
        } = observation;
        let name = name.as_str();
        count += 1;
        let union = both + under + over;
        // Strictly below 20% of foreground union; integer comparison keeps the
        // exact boundary independent of rounded report percentages. Blank
        // references and renderer errors cannot satisfy a positive control.
        let is_diagnostic = exceptions
            .get(name)
            .is_some_and(|(kind, _, _)| *kind == "diagnostic");
        let meets_target =
            !is_diagnostic && status == "rendered" && within_budget(both, under, over);
        blank += usize::from(reference_ink == 0);
        diagnostics += usize::from(is_diagnostic);
        if meets_target {
            positive += 1;
            covered
                .entry(profile_name)
                .or_default()
                .extend(codes.iter().cloned());
        } else if !is_diagnostic {
            over_budget += 1;
        }
        let error = if status == "rendered" {
            format!("{:.6}", 100. * (under + over) as f64 / union as f64)
        } else {
            "NA".to_owned()
        };
        let observation = format!(
            "{}\t{}\t{status}\t{both}\t{under}\t{over}\t{error}\t{}\t{profile_name}\t{}x{}\t{local_size}\t{local_hash}",
            source_hash,
            printer_hash,
            diagnostic.replace(['\t', '\n', '\r'], " "),
            width, height
        );
        if let Some(report) = &mut report {
            let (classification, reason) = exceptions
                .get(name)
                .map(|(kind, reason, _)| (*kind, *reason))
                .unwrap_or((
                    if meets_target {
                        "positive"
                    } else {
                        "unreviewed"
                    },
                    "",
                ));
            writeln!(
                report,
                "{name}\t{}\t{observation}\t{classification}\t{reason}",
                codes.into_iter().collect::<Vec<_>>().join(",")
            )
            .unwrap();
        }
        if let Some((kind, _, expected)) = exceptions.get(name) {
            used_exceptions.insert(name.to_owned());
            if observation != *expected {
                failures.push(format!("{name}: known observation changed\nexpected: {expected}\nactual: {observation}"));
            }
            if (*kind == "gap" && meets_target) || (*kind == "text" && under + over == 0) {
                failures.push(format!("{name}: remove resolved exception"));
            }
            if *kind == "text" && !meets_target {
                failures.push(format!(
                    "{name}: text comparison must meet the foreground target"
                ));
            }
        } else if !meets_target || under + over != 0 {
            failures.push(format!("{name}: {status}, error={error}% {diagnostic}"));
        }
        if strict && !meets_target && !is_diagnostic {
            failures.push(format!("{name}: target unmet: {status}, error={error}%"));
        }
    }
    for name in exceptions.keys() {
        assert!(
            used_exceptions.contains(*name),
            "stale or missing comparison: {name}"
        );
    }
    if let Some(output) = report_path {
        fs::write(output, report.unwrap()).unwrap();
    }
    assert_eq!(
        // Original 1,614 frames plus eleven SurePost barcode captures:
        // two full labels, two isolated symbols, seven compaction controls;
        // plus 22 public-document previews, each pinned independently.
        count,
        1647,
        "review inventory changes; do not silently drop captures"
    );
    for model in ["ZD621_203_DPI", "ZQ610_PLUS_203_DPI"] {
        assert_eq!(
            covered.get(model),
            Some(&COMMANDS.iter().map(|s| s.to_string()).collect()),
            "{model}: every barcode command needs a nonblank comparison meeting the target"
        );
    }
    println!("Barcode audit: {count} frames; {positive} positive comparisons; {over_budget} unmet targets; {diagnostics} diagnostic controls ({blank} blank references).");
    assert!(
        failures.is_empty(),
        "barcode accuracy:\n{}",
        failures.join("\n")
    );
}

#[test]
fn coverage_uses_commands_not_payload_text_or_defaults() {
    assert!(commands(b"^XA^BY2^FDliteral BQ and BC^FS^XZ").is_empty());
    assert!(commands(b"^XA^GFB,3,3,3,^BQ^FS^XZ").is_empty());
    assert_eq!(
        commands(b"^XA^CC!!BQN,2,3!FDLA,ABC!FS!XZ"),
        BTreeSet::from(["BQ".into()])
    );
}

#[test]
fn foreground_budget_is_strict_and_excludes_empty_controls() {
    assert!(!within_budget(0, 0, 0));
    assert!(within_budget(1, 0, 0));
    assert!(within_budget(81, 9, 10));
    assert!(!within_budget(80, 10, 10));
    assert!(!within_budget(79, 11, 10));
    assert!(!within_budget(0, 0, 1));
}
