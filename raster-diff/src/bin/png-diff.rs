//! Compare reference and candidate binary PNGs; see docs/raster-diff.md.
use raster_diff::Raster;
use std::{
    env,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    process::ExitCode,
};
fn read(path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut data = Vec::new();
    fs::File::open(path)?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut data)?;
    if data.len() > 16 * 1024 * 1024 {
        return Err("PNG exceeds 16 MiB limit".into());
    }
    Ok(data)
}
fn run() -> Result<bool, Box<dyn std::error::Error>> {
    let (mut pad, mut check, mut threshold, mut scale) = (false, false, None, 1u32);
    let mut paths = Vec::new();
    let mut args = env::args().skip(1);
    let mut positional = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--" if !positional => positional = true,
            "--help" | "-h" if !positional => {
                println!("Usage: png-diff [--pad] [--threshold 1..255] [--scale N] [--check] REFERENCE.png CANDIDATE.png OUTPUT.png\n\nMagenta: reference only (missing ink). Cyan: candidate only (extra ink).\nDark gray: both black. White: both white.\nDefault: require binary pixels and equal dimensions. --pad adds white at right/bottom.\n--threshold explicitly binarizes non-binary images. --scale magnifies with nearest neighbors.\nOutput must not exist. Exit codes: 0 success, 1 differences with --check, 2 error.");
                return Ok(false);
            }
            "--pad" if !positional => pad = true,
            "--check" if !positional => check = true,
            "--scale" if !positional => {
                scale = args.next().ok_or("--scale needs a value")?.parse()?
            }
            "--threshold" if !positional => {
                let t: u8 = args.next().ok_or("--threshold needs a value")?.parse()?;
                if t == 0 {
                    return Err("threshold must be 1..255".into());
                }
                threshold = Some(t)
            }
            _ if !positional && arg.starts_with('-') => {
                return Err(format!("unknown option {arg}").into())
            }
            _ => paths.push(arg),
        }
    }
    if paths.len() != 3 {
        return Err(
            "usage: png-diff [OPTIONS] REFERENCE.png CANDIDATE.png OUTPUT.png (see --help)".into(),
        );
    }
    if scale == 0 {
        return Err("scale must be positive".into());
    }
    if Path::new(&paths[2]).exists() {
        return Err("output exists; choose a new filename".into());
    }
    let reference = Raster::decode_png_with_threshold(&read(&paths[0])?, threshold)?;
    let candidate = Raster::decode_png_with_threshold(&read(&paths[1])?, threshold)?;
    let diff = raster_diff::compare(&reference, &candidate, pad)?;
    let png = diff.png(scale)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&paths[2])?;
    file.write_all(&png)?;
    println!(
        "Reference: {}x{}; candidate: {}x{}; dimensions match: {}",
        reference.width, reference.height, candidate.width, candidate.height, diff.dimensions_match
    );
    println!("Magenta (reference only / missing ink): {}\nCyan (candidate only / extra ink): {}\nShared black: {}; shared white: {}",diff.reference_only,diff.candidate_only,diff.both_black,diff.both_white);
    println!(
        "Different pixels: {} / {} ({:.6}%); ink IoU: {:.6}",
        diff.different_pixels(),
        u64::from(diff.width) * u64::from(diff.height),
        diff.different_pixels() as f64 / (f64::from(diff.width) * f64::from(diff.height)) * 100.,
        diff.ink_iou()
    );
    if let Some(b) = diff.bounds {
        println!(
            "Difference bounds: x={}, y={}, width={}, height={}",
            b.x, b.y, b.width, b.height
        )
    } else {
        println!("Difference bounds: none")
    }
    println!("Wrote {}", paths[2]);
    Ok(check && !diff.matches())
}
fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::from(1),
        Ok(false) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("png-diff: {e}");
            ExitCode::from(2)
        }
    }
}
