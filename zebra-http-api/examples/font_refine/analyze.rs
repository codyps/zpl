//! Offline measurements; never reads the sealed directory.
use super::campaign::{self, Page, Probe};
use eyre::{ensure, eyre, Result};
use raster_diff::{compare, Raster};
use serde_json::{json, Value};
use std::{fs, path::Path};
fn tile(root: &Path, page: &Page, p: &Probe) -> Result<Raster> {
    tile_in(&root.join("development"), page, p)
}
fn tile_in(root: &Path, page: &Page, p: &Probe) -> Result<Raster> {
    let path = root.join(format!("{}.png", page.name));
    ensure!(
        fs::metadata(&path)?.len() <= 16 * 1024 * 1024,
        "capture too large"
    );
    let data = fs::read(&path)?;
    let hash = fs::read_to_string(path.with_extension("sha256"))?;
    ensure!(
        super::super::font_support::sha256(&data) == hash,
        "capture hash differs"
    );
    let r = Raster::decode_png_with_threshold(&data, None).map_err(|e| eyre!(e))?;
    campaign::validate(&r, page)?;
    Ok(campaign::normalize(&r, p))
}
fn metric(m: &Value) -> i64 {
    let r = m["column_runs"].as_array().unwrap();
    if r.len() < 2 {
        return 0;
    }
    r.last().unwrap()[0].as_i64().unwrap() - r[0][0].as_i64().unwrap()
}
pub fn run(root: &Path) -> Result<Value> {
    let pages = campaign::plan();
    let mut mapping = vec![];
    let mut calibration = vec![];
    let mut metrics = vec![];
    let mut scan_samples = vec![];
    for page in pages
        .iter()
        .filter(|p| p.group == "mapping" && p.probes[0].font != "0")
    {
        let source = pages
            .iter()
            .find(|s| {
                s.group == "mapping"
                    && s.probes[0].font == "0"
                    && s.probes[0].h == page.probes[0].h
                    && s.probes[0].w == page.probes[0].w
                    && s.probes[0].turns == page.probes[0].turns
            })
            .unwrap();
        let mut total = 0;
        for (p, q) in page.probes.iter().zip(&source.probes) {
            total += compare(&tile(root, source, q)?, &tile(root, page, p)?, false)
                .map_err(|e| eyre!(e))?
                .different_pixels();
        }
        mapping.push(json!({"page":page.name,"font":page.probes[0].font,"xor_against_font0":total,"caution":"matches alone cannot exclude silent missing-font fallback"}));
    }
    for page in pages
        .iter()
        .filter(|p| p.group == "calibration" && p.probes[0].font == "R:ZRFN.TTF")
    {
        let shifted = pages
            .iter()
            .find(|s| {
                s.group == "calibration"
                    && s.probes[0].font == "R:ZRFH.TTF"
                    && s.probes[0].h == page.probes[0].h
                    && s.probes[0].w == page.probes[0].w
                    && s.probes[0].turns == page.probes[0].turns
            })
            .unwrap();
        let mut differences = 0;
        let mut after_shift = 0;
        let mut measurements = vec![];
        for (p, q) in page.probes.iter().zip(&shifted.probes) {
            let plain = tile(root, page, p)?;
            let actual = tile(root, shifted, q)?;
            scan_samples.push((page.name.clone(), p.clone(), plain.clone()));
            differences += compare(&plain, &actual, false)
                .map_err(|e| eyre!(e))?
                .different_pixels();
            let mut candidate = plain.clone();
            candidate.pixels.fill(255);
            for y in 0..plain.height {
                for x in 1..plain.width {
                    candidate.pixels[(y * plain.width + x) as usize] =
                        plain.pixels[(y * plain.width + x - 1) as usize];
                }
            }
            after_shift += compare(&actual, &candidate, false)
                .map_err(|e| eyre!(e))?
                .different_pixels();
            measurements.push(json!({"glyph":p.text,"plain":campaign::measurement(&plain),"shifted":campaign::measurement(&actual)}));
        }
        calibration.push(json!({"page":page.name,"h":page.probes[0].h,"w":page.probes[0].w,"rotation":page.probes[0].turns,"plain_vs_instructed_xor":differences,"one_pixel_x_shift_xor":after_shift,"glyphs":measurements}));
    }
    for page in pages.iter().filter(|p| p.group == "metrics-development") {
        for p in &page.probes {
            let m = campaign::measurement(&tile(root, page, p)?);
            metrics.push(json!({"h":p.h,"origin":p.origin,"text":p.text,"sentinel_start_distance":metric(&m),"measurement":m}));
        }
    }
    let mut advances = vec![];
    for glyph in ["l", "H", " "] {
        let mut observations = vec![];
        let mut repetition_errors = vec![];
        for h in [16, 24, 32, 48, 64] {
            let get = |text: &str, origin: &str| {
                metrics
                    .iter()
                    .find(|v| v["h"] == h && v["text"] == text && v["origin"] == origin)
                    .unwrap()["sentinel_start_distance"]
                    .as_i64()
                    .unwrap()
            };
            let base = get("||", "FT");
            let advance = get(&format!("|{glyph}|"), "FT") - base;
            ensure!(advance >= 0, "negative measured advance");
            observations.push((h, advance as u32));
            for n in if glyph == " " {
                vec![1, 2]
            } else {
                vec![1, 2, 4]
            } {
                for origin in ["FT", "FO"] {
                    let text = format!("|{}|", glyph.repeat(n));
                    let error = get(&text, origin) - base - advance * n as i64;
                    repetition_errors.push(json!({"h":h,"origin":origin,"length":n,"error":error}));
                }
            }
        }
        if root.join("diagnostic/report.json").exists() && glyph != " " {
            let pages = super::diagnostic::pages();
            let page = &pages[if glyph == "l" { 0 } else { 1 }];
            let base = metric(&campaign::measurement(&tile_in(
                &root.join("diagnostic"),
                page,
                &page.probes[0],
            )?));
            let p = page
                .probes
                .iter()
                .find(|p| p.text == format!("|{glyph}|"))
                .unwrap();
            let actual = metric(&campaign::measurement(&tile_in(
                &root.join("diagnostic"),
                page,
                p,
            )?)) - base;
            ensure!(actual >= 0, "negative diagnostic advance");
            observations.push((p.h, actual as u32));
        }
        advances.push(json!({"glyph":glyph,"rounding":super::constraints::hypotheses(&observations,0),"repetition_errors":repetition_errors}));
    }
    let mut stems = vec![];
    for glyph in ["H", "I", "l", "|"] {
        for side in ["left", "right"] {
            let mut observations = vec![];
            for page in pages.iter().filter(|p| p.group == "development") {
                let p = page.probes.iter().find(|p| p.text == glyph).unwrap();
                let r = tile(root, page, p)?;
                let mut widths = std::collections::BTreeMap::<u32, usize>::new();
                for row in r.pixels.chunks_exact(r.width as usize) {
                    let mut runs = vec![];
                    let mut start = None;
                    for (x, black) in row.iter().map(|&p| p == 0).chain([false]).enumerate() {
                        match (start, black) {
                            (None, true) => start = Some(x),
                            (Some(a), false) => {
                                runs.push((x - a) as u32);
                                start = None;
                            }
                            _ => {}
                        }
                    }
                    if (glyph == "H" && runs.len() == 2) || (glyph != "H" && runs.len() == 1) {
                        let width = if side == "left" {
                            runs[0]
                        } else {
                            *runs.last().unwrap()
                        };
                        *widths.entry(width).or_default() += 1;
                    }
                }
                if let Some((&w, _)) = widths
                    .iter()
                    .max_by_key(|&(w, n)| (*n, std::cmp::Reverse(*w)))
                {
                    observations.push((p.h, w));
                }
            }
            stems.push(json!({"glyph":glyph,"side":side,"measurement":"modal scanline run width; not assumed equal to a hinted design stem","rounding":super::constraints::hypotheses(&observations,1)}));
        }
    }
    let first = pages.iter().find(|p| p.group == "repeat-start").unwrap();
    let last = pages.iter().find(|p| p.group == "repeat-end").unwrap();
    let mut repeat = 0;
    for (p, q) in first.probes.iter().zip(&last.probes) {
        repeat += compare(&tile(root, first, p)?, &tile(root, last, q)?, false)
            .map_err(|e| eyre!(e))?
            .different_pixels();
    }
    let mut scan_rules = vec![];
    for nonzero in [false, true] {
        for inclusive in [false, true] {
            for dropout in [false, true] {
                for fixed in [false, true] {
                    for tie_shift in [false, true] {
                        let rules = super::scan::Rules {
                            nonzero,
                            inclusive,
                            dropout,
                            fixed,
                            tie_shift,
                        };
                        let mut total = 0;
                        let mut exact = 0;
                        let mut cases = vec![];
                        for (name, p, reference) in &scan_samples {
                            let candidate = super::scan::render(p, rules);
                            let diff =
                                compare(reference, &candidate, false).map_err(|e| eyre!(e))?;
                            total += diff.different_pixels();
                            exact += usize::from(diff.matches());
                            cases.push(json!({"page":name,"glyph":p.text,"xor":diff.different_pixels(),"missing":diff.reference_only,"extra":diff.candidate_only}));
                        }
                        scan_rules.push(json!({"nonzero":nonzero,"inclusive_edges":inclusive,"horizontal_dropout":dropout,"fixed_26_6":fixed,"positive_edge_tie":tie_shift,"xor":total,"exact_cases":exact,"cases":cases}));
                    }
                }
            }
        }
    }
    scan_rules.sort_by_key(|v| v["xor"].as_u64().unwrap());
    Ok(
        json!({"mapping":mapping,"calibration":calibration,"metrics":metrics,"scan_rules":scan_rules,"advances":advances,"stem_constraints":stems,"repeat_xor":repeat,"sealed_pixels_read":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixtures() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/font-refinement-v1")
    }
    #[test]
    fn captured_advances_do_not_scale_from_the_32_dot_strike() {
        let pages = campaign::plan();
        let measure = |h, text: &str| {
            let page = pages
                .iter()
                .find(|page| {
                    page.group == "metrics-development"
                        && page
                            .probes
                            .iter()
                            .any(|p| p.h == h && p.origin == "FT" && p.text == text)
                })
                .unwrap();
            let p = page.probes.iter().find(|p| p.text == text).unwrap();
            metric(&campaign::measurement(&tile(&fixtures(), page, p).unwrap()))
        };
        for (text, at32, at64) in [("|l|", 8, 17), ("|H|", 20, 39), ("| |", 9, 19)] {
            assert_eq!(measure(32, text) - measure(32, "||"), at32);
            assert_eq!(measure(64, text) - measure(64, "||"), at64);
            assert_ne!(2 * at32, at64);
        }
    }
    #[test]
    fn printer_executes_a_one_pixel_shift_in_normal_and_rotated_fonts() {
        let pages = campaign::plan();
        for turns in [0, 1] {
            let plain = pages
                .iter()
                .find(|p| {
                    p.group == "calibration"
                        && p.probes[0].font == "R:ZRFN.TTF"
                        && p.probes[0].h == 32
                        && p.probes[0].w == 0
                        && p.probes[0].turns == turns
                })
                .unwrap();
            let moved = pages
                .iter()
                .find(|p| {
                    p.group == "calibration"
                        && p.probes[0].font == "R:ZRFH.TTF"
                        && p.probes[0].h == 32
                        && p.probes[0].w == 0
                        && p.probes[0].turns == turns
                })
                .unwrap();
            // Includes overlaps, a quadratic, diagonals, holes and thin strokes.
            for (p, q) in plain.probes.iter().zip(&moved.probes) {
                let a = tile(&fixtures(), plain, p).unwrap();
                let b = tile(&fixtures(), moved, q).unwrap();
                for y in 0..a.height {
                    for x in 1..a.width {
                        assert_eq!(
                            a.pixels[(y * a.width + x - 1) as usize],
                            b.pixels[(y * b.width + x) as usize]
                        );
                    }
                }
            }
        }
    }
}
