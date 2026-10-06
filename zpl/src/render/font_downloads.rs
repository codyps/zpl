//! Font downloads: Zebra Programming Guide ~DB pp. 169–170, ~DT/~DU pp. 179–180,
//! ~DY pp. 181–183 and B64/Z64 appendix.
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use super::{fonts, graphics, Limits};
use crate::{
    bitmap_font::{Glyph, Settings},
    parse::{Element, ParseContext},
};
use std::collections::BTreeMap;

pub(super) enum Download {
    TrueType {
        name: String,
        data: Vec<u8>,
    },
    Bitmap {
        name: String,
        settings: Settings,
        glyphs: Vec<Glyph>,
        baseline: f64,
        space: u32,
    },
}

// Decode into an arena before borrowing TrueType bytes. Entries become visible only
// when the interpreter reaches their command. Retain errors at their original offset,
// so an earlier render error still wins. No leaked/self-referential font allocations.
pub(super) fn prepare(input: &[u8], limits: Limits) -> BTreeMap<usize, Result<Download, String>> {
    let mut parser = ParseContext::from_bytes(input);
    let mut downloads = BTreeMap::new();
    let mut remaining = limits.font_bytes;
    loop {
        let offset = parser.position();
        let delimiter = parser.syntax().delimiter;
        let Some(Ok(item)) = parser.next() else { break };
        if let Element::ControlCommand(raw) = item {
            if matches!(raw.get(1..3), Some(b"DB" | b"DT" | b"DU" | b"DY")) {
                let result = decode(&raw[1..3], &raw[3..], delimiter, &mut remaining);
                let failed = result.is_err();
                downloads.insert(offset, result);
                if failed {
                    break;
                }
            }
        }
    }
    downloads
}

fn header<const N: usize>(data: &[u8], delimiter: u8) -> Result<([&str; N], &[u8]), String> {
    let mut parts = data.splitn(N + 1, |b| *b == delimiter);
    let mut fields = [""; N];
    for field in &mut fields {
        *field = std::str::from_utf8(parts.next().ok_or("missing font header")?)
            .map_err(|_| "invalid font header")?
            .trim();
    }
    Ok((fields, parts.next().ok_or("missing font payload")?))
}
fn number(value: &str, max: u32) -> Result<u32, String> {
    let value: u32 = value.parse().map_err(|_| "invalid font count or metric")?;
    if value > max {
        return Err("font count or metric exceeds supported range".into());
    }
    Ok(value)
}
fn charge(count: usize, remaining: &mut usize) -> Result<(), String> {
    *remaining = remaining
        .checked_sub(count)
        .ok_or("font bytes exceed configured renderer limit")?;
    Ok(())
}
fn name(value: &str, extension: &str) -> Result<String, String> {
    let value = if value.is_empty() { "UNKNOWN" } else { value };
    let value = if value.ends_with(':') {
        format!("{value}UNKNOWN")
    } else {
        value.to_owned()
    };
    fonts::font_name(&if value.contains('.') {
        value
    } else {
        format!("{value}.{extension}")
    })
}
fn bytes(data: &[u8], count: usize, binary: bool) -> Result<Vec<u8>, String> {
    let decoded = if binary {
        data.to_vec()
    } else {
        let data = data.trim_ascii();
        if data.starts_with(b":B64:") || data.starts_with(b":Z64:") {
            graphics::decode_envelope(data, count).map_err(|e| e.replace("graphic", "font"))?
        } else {
            let mut out = Vec::new();
            let mut high = None;
            for &b in data.iter().filter(|b| !b.is_ascii_whitespace()) {
                let v = (b as char).to_digit(16).ok_or("invalid font hex digit")? as u8;
                if let Some(h) = high.take() {
                    if out.len() == count {
                        return Err("font byte count mismatch".into());
                    }
                    out.push(h * 16 + v);
                } else {
                    high = Some(v);
                }
            }
            if high.is_some() {
                return Err("incomplete font hex byte".into());
            }
            out
        }
    };
    if decoded.len() != count {
        return Err("font byte count mismatch".into());
    }
    Ok(decoded)
}
fn decode(
    code: &[u8],
    data: &[u8],
    delimiter: u8,
    remaining: &mut usize,
) -> Result<Download, String> {
    if code == b"DB" {
        return bitmap(data, delimiter, remaining);
    }
    let (name, count, data, binary) = if code == b"DY" {
        let (p, data) = header::<5>(data, delimiter)?;
        if !matches!(p[1], "A" | "B") {
            return Err("unsupported DY font encoding (expected A or B)".into());
        }
        let extension = match p[2] {
            "T" => "TTF",
            "E" => "TTE",
            _ => return Err("unsupported DY font object type".into()),
        };
        let name = name(p[0], extension)?;
        if !(name.ends_with(extension) || p[2] == "T" && name.ends_with(".OTF")) {
            return Err("DY font filename and object type disagree".into());
        }
        (
            name,
            number(p[3], 16 * 1024 * 1024)? as usize,
            data,
            p[1] == "B",
        )
    } else {
        let (p, data) = header::<2>(data, delimiter)?;
        (
            name(p[0], if code == b"DT" { "DAT" } else { "FNT" })?,
            number(p[1], 16 * 1024 * 1024)? as usize,
            data,
            false,
        )
    };
    if count == 0 {
        return Err("empty font download".into());
    }
    charge(count, remaining)?;
    let data = bytes(data, count, binary)?;
    // The engine accepts quadratic SFNT. Legacy ZTools-only wrappers are not
    // assumed to be SFNT and must fail explicitly rather than become a fallback.
    if !data.starts_with(&[0, 1, 0, 0]) {
        return Err("unsupported downloaded font format: expected quadratic TrueType SFNT; legacy ZTools containers are unsupported".into());
    }
    Ok(Download::TrueType { name, data })
}
fn bitmap(data: &[u8], delimiter: u8, remaining: &mut usize) -> Result<Download, String> {
    let (p, data) = header::<8>(data, delimiter)?;
    let name = name(p[0], "FNT")?;
    if !name.ends_with(".FNT") || p[1] != "N" {
        return Err("DB requires FNT and normal orientation".into());
    }
    let height = number(p[2], 32000)?;
    let width = number(p[3], 32000)?;
    let baseline = number(p[4], height)?;
    let space = number(p[5], 4096)?;
    let count = number(p[6], 256)?;
    if [height, width, baseline, space, count].contains(&0) || p[7].is_empty() || p[7].len() > 63 {
        return Err("invalid DB font header".into());
    }
    let mut chunks = data.split(|b| *b == b'#');
    if !chunks.next().unwrap().iter().all(u8::is_ascii_whitespace) {
        return Err("invalid DB glyph marker".into());
    }
    let mut glyphs = Vec::new();
    for chunk in chunks {
        if glyphs.len() == count as usize {
            return Err("DB glyph count mismatch".into());
        }
        let (g, data) = header::<6>(chunk, b'.')?;
        if g[0].is_empty() || g[0].len() > 4 {
            return Err("invalid DB character code".into());
        }
        let codepoint = u32::from_str_radix(g[0], 16).map_err(|_| "invalid DB character code")?;
        let h = number(g[1], 4096)?;
        let w = number(g[2], 4096)?;
        let x = g[3].parse::<i32>().map_err(|_| "invalid DB x offset")?;
        let y = g[4].parse::<i32>().map_err(|_| "invalid DB y offset")?;
        let advance = number(g[5], 4096)?;
        let row = w.div_ceil(8) as usize;
        let size = row * h as usize;
        // Also account for per-glyph/row storage for empty or very thin bitmaps.
        charge(
            size + h as usize * std::mem::size_of::<Vec<u8>>() + std::mem::size_of::<Glyph>(),
            remaining,
        )?;
        let decoded = bytes(data, size, false)?;
        glyphs.push(Glyph {
            codepoint,
            advance,
            left: x,
            top: y.checked_neg().ok_or("invalid DB y offset")?,
            width: w,
            height: h,
            bitmap: if row == 0 {
                vec![Vec::new(); h as usize]
            } else {
                decoded.chunks_exact(row).map(<[u8]>::to_vec).collect()
            },
        });
    }
    if glyphs.len() != count as usize {
        return Err("DB glyph count mismatch".into());
    }
    Ok(Download::Bitmap {
        name,
        settings: Settings {
            font: '0',
            width,
            height,
            dpi: 203,
        },
        glyphs,
        baseline: baseline as f64,
        space,
    })
}
