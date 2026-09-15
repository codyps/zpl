//! Offline parameter-fit experiment. Does not change the embedded renderer font.
use clap::Parser;
use eyre::{ensure, eyre, Result};
use image_diff::{compare, Raster};
use serde_json::json;
use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};
mod font_fit;
mod font_support;
use font_fit::{Model, Sample, CELL};
const GLYPHS: &[u8] = b" AgjQMWil1O0@%&|";
const TRAIN: &[(usize, u32)] = &[
    (0, 12),
    (1, 16),
    (3, 24),
    (5, 32),
    (7, 48),
    (8, 64),
    (9, 96),
    (10, 128),
];
#[derive(Parser)]
#[command(about = "Fit vector outlines to saved font-study captures without network access")]
struct Args {
    #[arg(long,default_value=concat!(env!("CARGO_MANIFEST_DIR"),"/tests/fixtures/font-study"))]
    captures: PathBuf,
    #[arg(long, default_value_t = 1)]
    passes: usize,
    /// Captured glyph subset for a shorter experiment.
    #[arg(long, default_value = "AgjQMWil1O0@%&|")]
    characters: String,
    /// Fit detected stems with size-dependent width/position rounding.
    #[arg(long)]
    strokes: bool,
    output: PathBuf,
}
fn load(root: &Path, name: &str) -> Result<Raster> {
    let path = root.join(format!("{name}.png"));
    ensure!(
        fs::metadata(&path)?.len() <= 16 * 1024 * 1024,
        "PNG too large"
    );
    let r = Raster::decode_png_with_threshold(&fs::read(path)?, None).map_err(|e| eyre!(e))?;
    ensure!((r.width, r.height) == (768, 768), "unexpected atlas size");
    Ok(r)
}
fn tile(r: &Raster, index: usize, turns: usize) -> Raster {
    let mut out = Raster {
        width: CELL as u32,
        height: CELL as u32,
        pixels: vec![255; CELL * CELL],
    };
    for y in 0..CELL {
        for x in 0..CELL {
            let (rx, ry) = match turns {
                0 => (x, y),
                1 => (CELL - 1 - y, x),
                2 => (CELL - 1 - x, CELL - 1 - y),
                3 => (y, CELL - 1 - x),
                _ => unreachable!(),
            };
            out.pixels[y * CELL + x] =
                r.pixels[(index / 4 * CELL + ry) * 768 + index % 4 * CELL + rx];
        }
    }
    out
}
fn raster_json(reference: &Raster, model: &Model, h: u32, w: u32) -> serde_json::Value {
    let d = compare(reference, &font_fit::render(model, h, w), false).unwrap();
    json!({"different_pixels":d.different_pixels(),"ink_iou":d.ink_iou(),"missing_ink":d.reference_only,"extra_ink":d.candidate_only})
}
fn svg(models: &[(u8, Model)]) -> String {
    let mut out=String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 768 768\"><rect width=\"768\" height=\"768\" fill=\"white\"/>");
    for (code, m) in models {
        let index = GLYPHS.iter().position(|c| c == code).unwrap();
        let mut d = String::new();
        for c in &m.contours {
            for (i, p) in c.iter().enumerate() {
                write!(
                    d,
                    "{}{} {} ",
                    if i == 0 { 'M' } else { 'L' },
                    p[0] + 32. + (index % 4 * CELL) as f64,
                    p[1] + 144. + (index / 4 * CELL) as f64
                )
                .unwrap();
            }
            d.push_str("Z ");
        }
        write!(
            out,
            "<path fill=\"black\" fill-rule=\"evenodd\" d=\"{d}\"/>"
        )
        .unwrap();
    }
    out.push_str("</svg>");
    out
}
fn write(path: &Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(data)?;
    Ok(())
}
fn main() -> Result<()> {
    let args = Args::parse();
    ensure!((1..=4).contains(&args.passes), "passes must be 1..4");
    let mut codes = args.characters.as_bytes().to_vec();
    codes.sort_unstable();
    codes.dedup();
    ensure!(
        !codes.is_empty() && codes.iter().all(|c| GLYPHS.contains(c) && *c != b' '),
        "characters must be nonblank study glyphs"
    );
    ensure!(!args.output.exists(), "output must be new");
    fs::create_dir_all(&args.output)?;
    // Training pixels are loaded separately. Validation captures are first read after fitting.
    let training: Vec<_> = TRAIN
        .iter()
        .map(|&(i, h)| Ok((h, load(&args.captures, &format!("{i:02}-{h}x0-N"))?)))
        .collect::<Result<_>>()?;
    let mut initial = Vec::new();
    let mut outlines = Vec::new();
    let mut fitted = Vec::new();
    let mut records = Vec::new();
    let mut stroke_models = Vec::new();
    let mut stroke_records = Vec::new();
    for code in codes {
        let index = GLYPHS.iter().position(|&c| c == code).unwrap();
        let samples: Vec<_> = training
            .iter()
            .map(|(h, r)| Sample {
                height: *h,
                width: *h,
                image: tile(r, index, 0),
            })
            .collect();
        let base = font_fit::trace(&samples.last().unwrap().image, 0.65);
        ensure!(!base.contours.is_empty(), "empty glyph");
        ensure!(
            base.contours.iter().map(Vec::len).sum::<usize>() <= 2048,
            "excessive outline complexity"
        );
        let mut model = base.clone();
        let history = font_fit::fit(&mut model, &samples, args.passes);
        let outline = model.clone();
        let mut stroke_model = outline.clone();
        if args.strokes {
            let stroke_loss =
                font_fit::strokes::fit(&mut stroke_model, &samples, &samples.last().unwrap().image);
            let axes: Vec<_> = stroke_model
                .strokes
                .as_ref()
                .unwrap()
                .axes
                .iter()
                .map(|a| json!({"bands":a.bands,"params":a.params}))
                .collect();
            stroke_records.push(json!({"codepoint":code,"contours":stroke_model.contours,"hints":stroke_model.hints,"axes":axes,"stroke_loss":stroke_loss}));
            eprintln!(
                "{}: stroke rounding {:.4} -> {:.4}",
                char::from(code),
                stroke_loss[0],
                stroke_loss.last().unwrap()
            );
        }
        stroke_models.push((code, stroke_model));
        let hints = font_fit::fit_hints(&mut model, &samples);
        eprintln!(
            "{}: {} vertices, training {:.4} -> {:.4} -> {:.4}",
            char::from(code),
            model.contours.iter().map(Vec::len).sum::<usize>(),
            history[0],
            history.last().unwrap(),
            hints.last().unwrap()
        );
        records.push(json!({"codepoint":code,"contours":model.contours,"hints":model.hints,"outline_loss":history,"hint_loss":hints}));
        initial.push((code, base));
        outlines.push((code, outline));
        fitted.push((code, model));
    }
    write(&args.output.join("outlines.svg"), svg(&fitted).as_bytes())?;
    let mut provenance = Vec::new();
    for &(i, h) in TRAIN {
        let name = format!("{i:02}-{h}x0-N");
        provenance.push(json!({"name":name,"sha256":font_support::sha256(&fs::read(args.captures.join(format!("{name}.png")))?)}));
    }
    let mut document = json!({"schema":"zpl-outline-fit-v1","source_height":128,"fill_rule":"evenodd","curve_type":"piecewise-linear","coordinates":"relative to FT baseline, source dots","training":provenance,"passes":args.passes,"simplification_tolerance":0.65,"max_vertex_displacement":1.5,"objective":"mean per-size binary XOR divided by reference ink","hint_parameters":["x_pixel_phase","y_pixel_phase","x_bbox_snap_strength","y_bbox_snap_strength"],"glyphs":records,"limitations":["no advance widths","no recovered TrueType bytecode","no exact-font claim"]});
    document["stage"] = json!("outline-and-hints");
    write(
        &args.output.join("model.json"),
        &serde_json::to_vec_pretty(&document)?,
    )?;
    document["stage"] = json!("outline-only");
    for glyph in document["glyphs"].as_array_mut().unwrap() {
        glyph["hints"] = json!([0., 0., 0., 0.]);
        glyph.as_object_mut().unwrap().remove("hint_loss");
    }
    write(
        &args.output.join("outline-model.json"),
        &serde_json::to_vec_pretty(&document)?,
    )?;
    if args.strokes {
        document["schema"] = json!("zpl-stroke-fit-v1");
        document["stage"] = json!("outline-and-stroke-rounding");
        document["glyphs"] = json!(stroke_records);
        document["stroke_parameters"] = json!([
            "source_width_delta",
            "width_rounding_bias",
            "inverse_size_bias",
            "position_rounding_bias",
            "warp_strength",
            "pixel_phase"
        ]);
        document["stroke_detection"] = json!({"source_height":128,"width_range":[2,24],"minimum_support":6,"maximum_bands_per_axis":3});
        document["stroke_parameter_bounds"] =
            json!([[-2, 2], [0, 1], [-16, 16], [0, 1], [0, 1], [-0.5, 0.5]]);
        document["stroke_search_selection"]=json!("refine zero-inverse seed grid and expanded inverse seed grid separately; retain lower training objective, prefer zero-seed trial on ties");
        document["stroke_coarse_search"] = json!({"strength":[0.5,1],"source_delta":[0,-1,1],"width_bias":[0.25,0.5,0.75],"position_bias":[0,0.5,1],"inverse_size_bias":[0,-8,8]});
        document["stroke_width_rule"] = json!(
            "max(1,floor((source_width+delta)*axis_size/128+bias+inverse_size_bias/axis_size))"
        );
        document["stroke_position_rule"] =
            json!("floor(source_low*axis_size/128+position_rounding_bias)+pixel_phase");
        document["stroke_interpolation"]=json!("blend source and rounded edges by warp_strength; clamp target knots to nondecreasing; interpolate between knots, translate outside");
        write(
            &args.output.join("stroke-model.json"),
            &serde_json::to_vec_pretty(&document)?,
        )?;
    }
    let mut reports = Vec::new();
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
    for (i, (h, w)) in sizes.into_iter().enumerate() {
        for turns in 0..4 {
            let name = format!("{i:02}-{h}x{w}-{}", ['N', 'R', 'I', 'B'][turns]);
            let actual = load(&args.captures, &name)?;
            let split = if turns == 0 && TRAIN.iter().any(|&(j, _)| j == i) {
                "train"
            } else {
                "validation"
            };
            let mut glyphs = Vec::new();
            let mut sums = [0u64; 4];
            let mut canvas = Raster {
                width: 768,
                height: 768,
                pixels: vec![255; 768 * 768],
            };
            let mut expected = canvas.clone();
            for (j, (code, model)) in fitted.iter().enumerate() {
                let index = GLYPHS.iter().position(|c| c == code).unwrap();
                let reference = tile(&actual, index, turns);
                let width = if w == 0 { h } else { w };
                let a = raster_json(&reference, &initial[j].1, h, width);
                let b = raster_json(&reference, &outlines[j].1, h, width);
                let c = raster_json(&reference, model, h, width);
                for (sum, value) in sums.iter_mut().zip([&a, &b, &c]) {
                    *sum += value["different_pixels"].as_u64().unwrap();
                }
                let mut record = json!({"codepoint":code,"initial":a,"outline":b,"hinted":c});
                if args.strokes {
                    let d = raster_json(&reference, &stroke_models[j].1, h, width);
                    sums[3] += d["different_pixels"].as_u64().unwrap();
                    record["strokes"] = d;
                }
                glyphs.push(record);
                let rendered = font_fit::render(
                    if args.strokes {
                        &stroke_models[j].1
                    } else {
                        model
                    },
                    h,
                    width,
                );
                for y in 0..CELL {
                    let offset = (index / 4 * CELL + y) * 768 + index % 4 * CELL;
                    canvas.pixels[offset..offset + CELL]
                        .copy_from_slice(&rendered.pixels[y * CELL..(y + 1) * CELL]);
                    expected.pixels[offset..offset + CELL]
                        .copy_from_slice(&reference.pixels[y * CELL..(y + 1) * CELL]);
                }
            }
            if (turns == 0 && [20, 31, 32, 33, 64, 128].contains(&h) && w == 0)
                || (i == 5 && turns == 1)
            {
                let d = compare(&expected, &canvas, false).map_err(|e| eyre!(e))?;
                write(
                    &args.output.join(format!("{name}-diff.png")),
                    &d.png(1).map_err(|e| eyre!(e))?,
                )?;
            }
            let mut record = json!({"name":name,"split":split,"initial_mismatch":sums[0],"outline_mismatch":sums[1],"hinted_mismatch":sums[2],"glyphs":glyphs});
            if args.strokes {
                record["stroke_mismatch"] = json!(sums[3]);
            }
            reports.push(record);
        }
    }
    write(
        &args.output.join("report.json"),
        &serde_json::to_vec_pretty(&reports)?,
    )?;
    for split in ["train", "validation"] {
        let mut sums = [0u64; 4];
        for r in reports.iter().filter(|r| r["split"] == split) {
            for (sum, key) in sums.iter_mut().zip([
                "initial_mismatch",
                "outline_mismatch",
                "hinted_mismatch",
                "stroke_mismatch",
            ]) {
                *sum += r[key].as_u64().unwrap_or(0);
            }
        }
        eprintln!(
            "{split}: initial={} outline={} hinted={}",
            sums[0], sums[1], sums[2]
        );
        if args.strokes {
            eprintln!("{split}: strokes={}", sums[3]);
        }
    }
    Ok(())
}
