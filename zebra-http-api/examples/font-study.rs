//! Repeatable preview-only study of font scaling and rotation.
use clap::Parser;
use eyre::{ensure, eyre, Result};
use raster_diff::{compare, Raster};
use serde_json::json;
use std::{fmt::Write as _, fs, io::Read, path::PathBuf, time::Duration};
mod font_support;
const CELL: u32 = 192;
const SIDE: u32 = CELL * 4;
const GLYPHS: &[u8] = b" AgjQMWil1O0@%&|";
const LIMIT: u64 = 16 * 1024 * 1024;
#[derive(Parser)]
#[command(about = "Compare resident font 0 across sizes and rotations using previews only")]
struct Args {
    #[arg(long)]
    host: String,
    #[arg(long)]
    offline: bool,
    /// Reuse saved requests with identical ZPL; fetch missing previews only.
    #[arg(long)]
    resume: bool,
    output: PathBuf,
}
fn rotate(x: u32, y: u32, turns: usize) -> (u32, u32) {
    match turns {
        0 => (x, y),
        1 => (CELL - 1 - y, x),
        2 => (CELL - 1 - x, CELL - 1 - y),
        3 => (y, CELL - 1 - x),
        _ => unreachable!(),
    }
}
fn plan(h: u32, w: u32, turns: usize) -> String {
    let orientation = ['N', 'R', 'I', 'B'][turns];
    let mut z = format!("^XA^PW{SIDE}^LL{SIDE}^LH0,0^LS0^LT0^PON^LRN^CI27");
    let (mut x, mut y) = rotate(32, 144, turns);
    // FT is an edge coordinate: rotating pixel centers uses C-1, but anchors use C.
    if turns == 1 || turns == 2 {
        x += 1;
    }
    if turns == 2 || turns == 3 {
        y += 1;
    }
    for (i, code) in GLYPHS.iter().enumerate() {
        write!(
            z,
            "\n^FT{},{}^A0{orientation},{h},{w}^FH^FD_{code:02X}^FS",
            i as u32 % 4 * CELL + x,
            i as u32 / 4 * CELL + y
        )
        .unwrap();
    }
    z.push_str("\n^XZ");
    z
}
/// Rotate each tile back to normal, preserving absolute bearings (no alignment).
fn normalize(r: &Raster, turns: usize) -> Result<Raster> {
    ensure!(
        (r.width, r.height) == (SIDE, SIDE),
        "preview canvas changed"
    );
    let mut out = Raster {
        width: SIDE,
        height: SIDE,
        pixels: vec![255; (SIDE * SIDE) as usize],
    };
    for ty in 0..4 {
        for tx in 0..4 {
            for y in 0..CELL {
                for x in 0..CELL {
                    let (rx, ry) = rotate(x, y, turns);
                    out.pixels[((ty * CELL + y) * SIDE + tx * CELL + x) as usize] =
                        r.pixels[((ty * CELL + ry) * SIDE + tx * CELL + rx) as usize];
                }
            }
        }
    }
    Ok(out)
}
/// Sample a larger strike at the target size about the same FT baseline origin.
/// This is a bitmap baseline for outline fitting, not an inferred vector font.
fn rescale_samples(r: &Raster, source: u32, height: u32, width: u32) -> Raster {
    let mut out = Raster {
        width: SIDE,
        height: SIDE,
        pixels: vec![255; (SIDE * SIDE) as usize],
    };
    let width = if width == 0 { height } else { width };
    for ty in 0..4 {
        for tx in 0..4 {
            for y in 0..CELL {
                for x in 0..CELL {
                    let sx = (32. + (x as f64 + 0.5 - 32.) * source as f64 / width as f64).floor()
                        as i32;
                    let sy = (144. + (y as f64 + 0.5 - 144.) * source as f64 / height as f64)
                        .floor() as i32;
                    if sx >= 0 && sy >= 0 && sx < CELL as i32 && sy < CELL as i32 {
                        out.pixels[((ty * CELL + y) * SIDE + tx * CELL + x) as usize] = r.pixels
                            [((ty * CELL + sy as u32) * SIDE + tx * CELL + sx as u32) as usize];
                    }
                }
            }
        }
    }
    out
}
fn read(path: &std::path::Path) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    fs::File::open(path)?
        .take(LIMIT + 1)
        .read_to_end(&mut data)?;
    ensure!(data.len() as u64 <= LIMIT, "capture too large");
    Ok(data)
}
fn save(path: &std::path::Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(data)?;
    Ok(())
}
fn derived(path: &std::path::Path, data: &[u8]) -> Result<()> {
    if path.exists() {
        ensure!(
            read(path)? == data,
            "derived output changed: {}",
            path.display()
        );
    } else {
        save(path, data)?;
    }
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let host = reqwest::Url::parse(&args.host)?;
    ensure!(
        matches!(host.scheme(), "http" | "https")
            && host.host_str().is_some()
            && host.username().is_empty()
            && host.password().is_none()
            && host.query().is_none()
            && host.fragment().is_none(),
        "invalid host URL"
    );
    let sizes = [
        (12, 0),
        (16, 0),
        (20, 0),
        (24, 0),
        (31, 0),
        (32, 0),
        (33, 0),
        (48, 0),
        (64, 0),
        (96, 0),
        (128, 0),
        (32, 16),
        (32, 48),
        (64, 32),
    ];
    let config = serde_json::to_vec_pretty(&json!({"schema": "font-study-v1", "host": args.host,
        "sizes":sizes,"glyphs":String::from_utf8_lossy(GLYPHS),"cell":CELL,"anchor":[32,144],"repeat":[32,0]}))?;
    if args.output.exists() {
        ensure!(
            args.resume || args.offline,
            "output exists; use --resume or --offline"
        );
        ensure!(
            read(&args.output.join("capture.json"))? == config,
            "capture configuration differs"
        );
    } else {
        ensure!(!args.offline, "offline directory missing");
        fs::create_dir_all(&args.output)?;
        save(&args.output.join("capture.json"), &config)?;
    }
    let client = reqwest::Client::builder()
        .http1_title_case_headers()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let username = std::env::var("ZPL_USERNAME").unwrap_or_default();
    let password = std::env::var("ZPL_PASSWORD").unwrap_or_default();
    let mut reports = Vec::new();
    let mut baseline = None;
    for (iteration, (h, w)) in sizes.into_iter().chain([(32, 0)]).enumerate() {
        let mut normal = None;
        for turns in 0..4 {
            let name = format!("{iteration:02}-{h}x{w}-{}", ['N', 'R', 'I', 'B'][turns]);
            let z = plan(h, w, turns);
            ensure!(
                !args.output.join(format!("{name}.png")).exists()
                    || args.output.join(format!("{name}.zpl")).exists(),
                "cached PNG has no request"
            );
            derived(&args.output.join(format!("{name}.zpl")), z.as_bytes())?;
            let path = args.output.join(format!("{name}.png"));
            let data = if path.exists() {
                read(&path)?
            } else {
                ensure!(!args.offline, "missing offline page {name}");
                tokio::time::sleep(Duration::from_millis(500)).await;
                let data = zebra_http_api::zpl_to_png_with_credentials(
                    client.clone(),
                    host.clone(),
                    &z,
                    &username,
                    &password,
                )
                .await?;
                save(&path, &data)?;
                data
            };
            let actual = Raster::decode_png_with_threshold(&data, None).map_err(|e| eyre!(e))?;
            let normalized = normalize(&actual, turns)?;
            // Guard against accidentally clipped samples or an empty/incorrect response.
            for (i, &code) in GLYPHS.iter().enumerate() {
                let (tx, ty) = (i as u32 % 4 * CELL, i as u32 / 4 * CELL);
                let mut ink = 0;
                for y in 0..CELL {
                    for x in 0..CELL {
                        if normalized.pixels[((ty + y) * SIDE + tx + x) as usize] == 0 {
                            ensure!(
                                x > 1 && y > 1 && x < CELL - 2 && y < CELL - 2,
                                "glyph touches tile edge"
                            );
                            ink += 1;
                        }
                    }
                }
                ensure!(code == b' ' || ink > 0, "missing glyph {code}");
                ensure!(code != b' ' || ink == 0, "blank tile has unexpected ink");
            }
            if turns == 0 {
                normal = Some(normalized.clone());
            }
            let rotation =
                compare(normal.as_ref().unwrap(), &normalized, false).map_err(|e| eyre!(e))?;
            let doc = zpl::render::render(z.as_bytes(), zpl::render::Options::default())?;
            let local = zpl::output::rasterize(&doc.labels[0])?;
            let diff = compare(&actual, &local, false).map_err(|e| eyre!(e))?;
            derived(
                &args.output.join(format!("{name}-local-diff.png")),
                &diff.png(1).map_err(|e| eyre!(e))?,
            )?;
            derived(
                &args.output.join(format!("{name}-rotation-diff.png")),
                &rotation.png(1).map_err(|e| eyre!(e))?,
            )?;
            let mut per_glyph = Vec::new();
            for (i, code) in GLYPHS.iter().enumerate() {
                let (tx, ty) = (i as u32 % 4 * CELL, i as u32 / 4 * CELL);
                let count = (ty..ty + CELL)
                    .flat_map(|y| (tx..tx + CELL).map(move |x| (y * SIDE + x) as usize))
                    .filter(|&p| normal.as_ref().unwrap().pixels[p] != normalized.pixels[p])
                    .count();
                per_glyph.push(json!({"codepoint":code,"different_pixels":count}));
            }
            let repeat = if h == 32 && w == 0 && turns == 0 {
                let delta = baseline
                    .as_ref()
                    .map(|b| compare(b, &actual, false).unwrap().different_pixels());
                baseline = Some(actual.clone());
                delta
            } else {
                None
            };
            reports.push(json!({"name":name,"height":h,"width":w,"orientation":(['N','R','I','B'][turns].to_string()),
                "sha256":font_support::sha256(&data),"local_different_pixels":diff.different_pixels(),"local_ink_iou":diff.ink_iou(),
                "rotation_different_pixels":rotation.different_pixels(),"rotation_ink_iou":rotation.ink_iou(),"rotation_glyphs":per_glyph,"repeat_different_pixels":repeat}));
            eprintln!(
                "{name}: local={} rotated-printer={}",
                diff.different_pixels(),
                rotation.different_pixels()
            );
        }
    }
    derived(
        &args.output.join("report.json"),
        &serde_json::to_vec_pretty(&reports)?,
    )?;
    let mut scaling = Vec::new();
    for (source, source_name) in [(96, "09-96x0-N"), (128, "10-128x0-N")] {
        let large = Raster::decode_png_with_threshold(
            &read(&args.output.join(format!("{source_name}.png")))?,
            None,
        )
        .map_err(|e| eyre!(e))?;
        for report in &reports {
            if report["orientation"] != "N" {
                continue;
            }
            let name = report["name"].as_str().unwrap();
            let h = report["height"].as_u64().unwrap() as u32;
            let w = report["width"].as_u64().unwrap() as u32;
            let reference = Raster::decode_png_with_threshold(
                &read(&args.output.join(format!("{name}.png")))?,
                None,
            )
            .map_err(|e| eyre!(e))?;
            let d = compare(&reference, &rescale_samples(&large, source, h, w), false)
                .map_err(|e| eyre!(e))?;
            scaling.push(json!({"source_height":source,"target":name,"different_pixels":d.different_pixels(),"ink_iou":d.ink_iou()}));
        }
    }
    derived(
        &args.output.join("scaling.json"),
        &serde_json::to_vec_pretty(&scaling)?,
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn independent_size_strikes_match_held_out_text() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/font-study");
        for (h, text) in [(20, "AVATAR Agj Wavy 123 _^~|!"), (64, "AVATAR Agj 123")] {
            let dir = root.join(format!("strike-{h}"));
            let (settings, glyphs) =
                zpl::font_extract::unpack(&std::fs::read(dir.join("font.zbf")).unwrap()).unwrap();
            assert_eq!(glyphs.len(), 95);
            let (request, expected) =
                zpl::font_extract::verification_plan(&glyphs, settings, text).unwrap();
            assert_eq!(
                request.as_bytes(),
                std::fs::read(dir.join("verification.zpl")).unwrap()
            );
            let actual = Raster::decode_png_with_threshold(
                &std::fs::read(dir.join("verification.png")).unwrap(),
                None,
            )
            .unwrap();
            assert!(compare(&expected, &actual, false).unwrap().matches());
        }
    }
    #[test]
    fn printer_rotation_residuals_are_real_and_repeatable() {
        let load = |name: &str| {
            Raster::decode_png_with_threshold(
                &std::fs::read(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/fixtures/font-study")
                        .join(name),
                )
                .unwrap(),
                None,
            )
            .unwrap()
        };
        let normal = load("05-32x0-N.png");
        assert_eq!(normal, load("14-32x0-N.png"));
        for (turns, suffix, expected) in [(1, "R", 2), (2, "I", 3), (3, "B", 4)] {
            let rotated = normalize(&load(&format!("05-32x0-{suffix}.png")), turns).unwrap();
            assert_eq!(
                compare(&normal, &rotated, false)
                    .unwrap()
                    .different_pixels(),
                expected
            );
        }
        let large = load("10-128x0-N.png");
        let rotated = normalize(&load("10-128x0-R.png"), 1).unwrap();
        assert_eq!(
            compare(&large, &rotated, false).unwrap().different_pixels(),
            31
        );
    }

    use super::*;
    #[test]
    fn native_scaling_preserves_samples() {
        let mut r = Raster {
            width: SIDE,
            height: SIDE,
            pixels: vec![255; (SIDE * SIDE) as usize],
        };
        r.pixels[(120 * SIDE + 48) as usize] = 0;
        r.pixels[(500 * SIDE + 401) as usize] = 0;
        assert_eq!(rescale_samples(&r, 128, 128, 0), r);
    }
    #[test]
    fn rotations_preserve_asymmetric_pixels_and_tiles() {
        let mut normal = Raster {
            width: SIDE,
            height: SIDE,
            pixels: vec![255; (SIDE * SIDE) as usize],
        };
        for &(x, y) in &[(2, 3), (25, 40), (200, 399), (767, 767)] {
            normal.pixels[(y * SIDE + x) as usize] = 0;
        }
        for turns in 0..4 {
            let mut rotated = normal.clone();
            rotated.pixels.fill(255);
            for y in 0..SIDE {
                for x in 0..SIDE {
                    let (rx, ry) = rotate(x % CELL, y % CELL, turns);
                    rotated.pixels
                        [((y / CELL * CELL + ry) * SIDE + x / CELL * CELL + rx) as usize] =
                        normal.pixels[(y * SIDE + x) as usize];
                }
            }
            assert_eq!(normalize(&rotated, turns).unwrap(), normal);
        }
    }
}
