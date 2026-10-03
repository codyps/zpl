//! Offline audit and differential-render driver for the original TrueType engine.
//! No printer access, font downloads, or production assets are implicit.
//! Instruction framing: OpenType TrueType Instruction Set, "Pushing data":
//! https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, BufRead, Write},
    process::ExitCode,
};
use zpl::{
    output::raster::truetype::{self, ScanMode},
    truetype::{Environment, Font, Hinting, Size},
};

fn quote(s: &str) -> String {
    let mut result = String::from("\"");
    for c in s.chars() {
        match c as u32 {
            34 | 92 => {
                result.push(char::from(92));
                result.push(c);
            }
            0..=31 => result.push_str(&format!("{}u{:04x}", char::from(92), c as u32)),
            _ => result.push(c),
        }
    }
    result.push('"');
    result
}
fn audit(font: &Font<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let mut counts = BTreeMap::new();
    let mut programs = 0;
    let mut bytes = 0;
    let mut inspect = |code: &[u8]| -> Result<(), String> {
        if !code.is_empty() {
            programs += 1;
        }
        bytes += code.len();
        let mut pc = 0;
        while pc < code.len() {
            let op = code[pc];
            *counts.entry(op).or_insert(0_usize) += 1;
            pc += 1;
            let length = match op {
                0x40 | 0x41 => {
                    let n = *code.get(pc).ok_or("truncated push count")? as usize;
                    pc += 1;
                    n * if op == 0x41 { 2 } else { 1 }
                }
                0xb0..=0xb7 => (op - 0xb0 + 1) as usize,
                0xb8..=0xbf => (op - 0xb8 + 1) as usize * 2,
                _ => 0,
            };
            pc += length;
            if pc > code.len() {
                return Err("truncated push payload".into());
            }
        }
        Ok(())
    };
    for tag in [b"fpgm", b"prep"] {
        inspect(font.table(tag).unwrap_or_default())?;
    }
    for glyph in 0..font.glyph_count() {
        inspect(font.glyph_program(glyph)?)?;
    }
    println!("{{\"units_per_em\":{},\"glyph_count\":{},\"programs\":{programs},\"program_bytes\":{bytes},\"opcodes\":{{{}}}}}",font.units_per_em(),font.glyph_count(),counts.iter().map(|(op,n)|format!("\"{op:02x}\":{n}")).collect::<Vec<_>>().join(","));
    Ok(())
}

/// A local visual smoke check, using hinted advances at an integer baseline.
/// This is Unicode text without ZPL mapping, shaping, kerning, or line layout.
fn specimen(font: &Font<'_>, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use zpl::output::{Adapter, Draw, Paint, Path, Png, Scene};
    if !(6..=7).contains(&args.len()) {
        return Err(
            "usage: ttf-examine specimen FONT OUTPUT.png X_PPEM Y_PPEM TEXT [center|zd621]".into(),
        );
    }
    let size = Size::new(args[3].parse()?, args[4].parse()?)?;
    if args[5].chars().count() > 256 {
        return Err("specimen exceeds 256 characters".into());
    }
    let (environment, scan) = match args.get(6).map(String::as_str).unwrap_or("center") {
        "center" => (Environment::Standard, ScanMode::Center),
        "zd621" => (
            Environment::Zd621V93 { quarter_turns: 0 },
            ScanMode::Zd621V93,
        ),
        _ => return Err("unknown specimen rendering profile".into()),
    };
    let instance = font.instance(size, Hinting::Native, environment)?;
    let mut draws = Vec::new();
    let mut pen = 0_i32;
    let (mut left, mut top, mut right, mut bottom) = (0, 0, 0, 0);
    let mut segments = 0;
    for character in args[5].chars() {
        let outline = instance.glyph(character)?;
        let advance = if scan == ScanMode::Zd621V93 {
            instance.layout_advance(font.glyph_index(character).ok_or("missing glyph")?)?
        } else {
            u32::try_from((outline.advance + 32).div_euclid(64))?
        };
        let glyph = truetype::rasterize(&outline, character as u32, advance, 0, scan)?;
        let mut path = Path::default();
        let x = pen + glyph.left;
        left = left.min(x);
        right = right.max(x + glyph.width as i32);
        top = top.min(glyph.top);
        bottom = bottom.max(glyph.top + glyph.height as i32);
        for (row, bits) in glyph.bitmap.iter().enumerate() {
            let mut col = 0;
            while col < glyph.width as usize {
                if bits[col / 8] & (128 >> (col % 8)) == 0 {
                    col += 1;
                    continue;
                }
                let start = col;
                while col < glyph.width as usize && bits[col / 8] & (128 >> (col % 8)) != 0 {
                    col += 1;
                }
                segments += 5;
                if segments > zpl::output::MAX_SEGMENTS {
                    return Err("specimen path budget exceeded".into());
                }
                path.rect(
                    (x + start as i32) as f64,
                    (glyph.top + row as i32) as f64,
                    (col - start) as f64,
                    1.,
                );
            }
        }
        // Separate draws preserve black union if adjacent glyph ink overlaps.
        draws.push(Draw {
            path,
            paint: Paint::Black,
        });
        pen += advance as i32;
        if pen > 4096 {
            return Err("specimen exceeds 4096 dots".into());
        }
    }
    right = right.max(pen);
    let mut scene = Scene::new((right - left + 32) as u32, (bottom - top + 32) as u32, 203)?;
    for draw in &mut draws {
        draw.path.transform(|p| {
            zpl::output::Point::new(p.x + (16 - left) as f64, p.y + (16 - top) as f64)
        });
    }
    scene.draws = draws;
    fs::write(&args[2], Png.encode(&scene)?)?;
    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 2 {
        return Err("usage: ttf-examine audit FONT | glyphs FONT [center|zebra] [native|none]; stdin: x_ppem y_ppem codepoint quarter_turns".into());
    }
    let bytes = fs::read(&args[1])?;
    let font = Font::parse(&bytes)?;
    if args[0] == "audit" {
        return audit(&font);
    }
    if args[0] == "specimen" {
        return specimen(&font, &args);
    }
    if args[0] != "glyphs" {
        return Err("unknown command".into());
    }
    let mode = match args.get(2).map(String::as_str).unwrap_or("center") {
        "center" => ScanMode::Center,
        "zebra" => ScanMode::ZebraExperimental,
        "zd621" => ScanMode::Zd621V93,
        _ => return Err("unknown scan mode".into()),
    };
    let hinting = match args.get(3).map(String::as_str).unwrap_or("native") {
        "native" => Hinting::Native,
        "none" => Hinting::None,
        _ => return Err("unknown hint mode".into()),
    };
    let mut output = io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let line = line?;
        let result = (|| -> Result<String, Box<dyn std::error::Error>> {
            if line.len() > 128 {
                return Err("request exceeds 128 bytes".into());
            }
            let parts = line
                .split_whitespace()
                .map(str::parse::<u32>)
                .collect::<Result<Vec<_>, _>>()?;
            if parts.len() != 4 || parts[0] > 4096 || parts[1] > 4096 || parts[3] > 3 {
                return Err("invalid glyph request".into());
            }
            let character = char::from_u32(parts[2]).ok_or("invalid character")?;
            let turns = parts[3] as u8;
            let environment = if args.get(2).is_some_and(|s| s == "zd621") {
                Environment::Zd621V93 {
                    quarter_turns: turns,
                }
            } else if mode == ScanMode::ZebraExperimental {
                Environment::Zebra203 {
                    quarter_turns: turns,
                }
            } else {
                Environment::Standard
            };
            let instance = font.instance(
                Size::new(parts[0] as u16, parts[1] as u16)?,
                hinting,
                environment,
            )?;
            let glyph = font.glyph_index(character).ok_or("missing glyph")?;
            let outline = instance.outline(glyph)?;
            let layout_advance = instance.layout_advance(glyph)?;
            let advance = outline.advance.max(0).div_euclid(64) as u32;
            let bitmap = truetype::rasterize(&outline, parts[2], advance, turns, mode)?;
            let contours = outline
                .contours
                .iter()
                .map(|c| {
                    format!(
                        "[{}]",
                        c.iter()
                            .map(|p| format!("[{},{},{}]", p.x, p.y, u8::from(p.on_curve)))
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let rows = bitmap
                .bitmap
                .iter()
                .map(|r| {
                    format!(
                        "\"{}\"",
                        r.iter().map(|b| format!("{b:02x}")).collect::<String>()
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            Ok(format!("{{\"glyph\":{glyph},\"layout_advance\":{layout_advance},\"advance\":{},\"linear_advance\":{},\"contours\":[{contours}],\"left\":{},\"top\":{},\"width\":{},\"height\":{},\"rows\":[{rows}]}}",outline.advance,outline.linear_advance,bitmap.left,bitmap.top,bitmap.width,bitmap.height))
        })();
        match result {
            Ok(line) => writeln!(output, "{line}")?,
            Err(error) => writeln!(output, "{{\"error\":{}}}", quote(&error.to_string()))?,
        }
    }
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
