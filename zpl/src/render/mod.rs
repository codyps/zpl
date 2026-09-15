//! Local, deterministic ZPL previews. Unsupported rendering semantics are errors.
mod barcode;
use crate::compression;
mod font;
mod graphics;
use crate::{
    output::{Draw, Paint, Path, Point, Scene},
    parse::{Element, ParseContext},
};
use std::{collections::HashMap, error::Error, fmt};
#[derive(Debug, Clone)]
pub struct RenderError {
    pub offset: usize,
    pub message: String,
}
impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ZPL byte {}: {}", self.offset, self.message)
    }
}
impl Error for RenderError {}
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            width: 812,
            height: 1218,
            dpi: 203,
        }
    }
}
#[derive(Debug)]
pub struct Document {
    pub labels: Vec<Scene>,
    pub warnings: Vec<String>,
}
#[derive(Clone)]
struct Field {
    x: f64,
    y: f64,
    baseline: bool,
    rotation: u8,
    reverse: bool,
    hex: Option<u8>,
    barcode: Option<(f64, bool, bool)>,
    path: Option<Path>,
    block: Option<(f64, usize, f64, u8)>,
    baseline_height: f64,
    text_size: Option<(f64, f64)>,
}
impl Default for Field {
    fn default() -> Self {
        Self {
            x: 0.,
            y: 0.,
            baseline: false,
            rotation: b'N',
            reverse: false,
            hex: None,
            barcode: None,
            path: None,
            block: None,
            baseline_height: 0.,
            text_size: None,
        }
    }
}
fn number(p: &[&str], i: usize, default: f64) -> Result<f64, String> {
    match p.get(i).filter(|s| !s.is_empty()) {
        None => Ok(default),
        Some(s) => {
            let n = s
                .parse::<f64>()
                .map_err(|_| format!("invalid number {s:?}"))?;
            if !n.is_finite() || n.abs() > 1_000_000. {
                return Err("number out of range".into());
            }
            Ok(n)
        }
    }
}
fn rotation(s: &str) -> Result<u8, String> {
    match s.as_bytes() {
        [] => Ok(b'N'),
        [c @ (b'N' | b'R' | b'I' | b'B')] => Ok(*c),
        _ => Err("unsupported orientation".into()),
    }
}
fn code39(s: &str, module: f64, ratio: f64, h: f64) -> Result<Path, String> {
    const CHARS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%*";
    const PATTERNS: [u16; 44] = [
        0x034, 0x121, 0x061, 0x160, 0x031, 0x130, 0x070, 0x025, 0x124, 0x064, 0x109, 0x049, 0x148,
        0x019, 0x118, 0x058, 0x00d, 0x10c, 0x04c, 0x01c, 0x103, 0x043, 0x142, 0x013, 0x112, 0x052,
        0x007, 0x106, 0x046, 0x016, 0x181, 0x0c1, 0x1c0, 0x091, 0x190, 0x0d0, 0x085, 0x184, 0x0c4,
        0x0a8, 0x0a2, 0x08a, 0x02a, 0x094,
    ];
    let mut p = Path::default();
    let mut x = 0.;
    for c in std::iter::once('*')
        .chain(s.chars())
        .chain(std::iter::once('*'))
    {
        let idx = CHARS.find(c).ok_or("unsupported Code 39 character")?;
        let mask = PATTERNS[idx];
        for i in 0..9 {
            let w = module
                * if mask & (1 << (8 - i)) != 0 {
                    ratio
                } else {
                    1.
                };
            if i % 2 == 0 {
                p.rect(x, 0., w, h)
            }
            x += w;
        }
        x += module;
    }
    Ok(p)
}
/// Convert a command stream to printer-dot paths. No printer or network access occurs.
pub fn render(input: &[u8], options: Options) -> Result<Document, RenderError> {
    if input.len() > 1_048_576 {
        return Err(RenderError {
            offset: 0,
            message: "input exceeds 1 MiB renderer limit".into(),
        });
    }
    let mut parser = ParseContext::from_bytes(input);
    let mut labels = Vec::new();
    let mut total_segments = 0usize;
    let mut scene = None;
    let mut field = Field::default();
    let (mut width, mut height) = (options.width, options.height);
    let (mut home_x, mut home_y) = (0., 0.);
    let (mut font_w, mut font_h) = (20., 20.);
    let (mut default_w, mut default_h) = (20., 20.);
    let (mut module, mut ratio, mut bar_h) = (2., 3., 100.);
    let mut graphics = HashMap::<String, Path>::new();
    let mut warnings = Vec::new();
    let (mut shift, mut top) = (0., 0.);
    let mut default_rotation = b'N';
    let mut reverse = false;
    let mut upside_down = false;
    loop {
        let syntax = parser.syntax();
        let offset = parser.position();
        let Some(item) = parser.next() else { break };
        let item = item.map_err(|e| RenderError {
            offset,
            message: e.to_string(),
        })?;
        let result = (|| -> Result<(), String> {
            let raw = item.as_bytes();
            if field.path.is_some()
                && matches!(&item,Element::FormatCommand(b) if b.get(1..3)==Some(b"GF"))
            {
                return Err("multiple drawings in one field".into());
            }
            let (name, data) = match item {
                Element::BeforeFirstCommand(b) => {
                    if b.iter().all(u8::is_ascii_whitespace) {
                        return Ok(());
                    }
                    return Err("non-command data".into());
                }
                Element::ControlCharacter(b) => match b[0] {
                    2 => ("XA", &b""[..]),
                    3 => ("XZ", &b""[..]),
                    15 => ("FS", &b""[..]),
                    _ => return Err("unsupported control character".into()),
                },
                _ => {
                    if raw.len() < 3 {
                        return Err("incomplete command".into());
                    }
                    (
                        std::str::from_utf8(&raw[1..3]).map_err(|_| "invalid command")?,
                        &raw[3..],
                    )
                }
            };
            if matches!(item, Element::ControlCommand(_))
                && !matches!(name, "DG" | "CC" | "CT" | "CD")
            {
                return Err(format!("unsupported control command {name}"));
            }
            if scene.is_none()
                && !matches!(
                    name,
                    "XA" | "DG"
                        | "CC"
                        | "CT"
                        | "CD"
                        | "FX"
                        | "PW"
                        | "LL"
                        | "CF"
                        | "LH"
                        | "LS"
                        | "LT"
                        | "LR"
                        | "PO"
                        | "FW"
                        | "BY"
                        | "CI"
                )
            {
                return Err("field or label terminator outside label".into());
            }
            if name == "DG" && !matches!(item, Element::ControlCommand(_)) {
                return Err("DG requires control prefix".into());
            }
            if name == "GF" && data.first() == Some(&b'B') {
                let mut pieces = data.splitn(5, |&b| b == syntax.delimiter);
                pieces.next();
                let parse = |v: Option<&[u8]>| -> Result<usize, String> {
                    std::str::from_utf8(v.ok_or("missing graphic header")?)
                        .map_err(|_| "invalid graphic header")?
                        .parse()
                        .map_err(|_| "invalid graphic count".into())
                };
                let sent = parse(pieces.next())?;
                let count = parse(pieces.next())?;
                let row = parse(pieces.next())?;
                if sent != count {
                    return Err("binary GF transmitted and graphic counts must match".into());
                }
                field.path = Some(graphics::decode(
                    pieces.next().ok_or("missing graphic payload")?,
                    count,
                    row,
                    true,
                )?);
                return Ok(());
            }
            if !syntax.delimiter.is_ascii() {
                return Err("non-ASCII parameter delimiters unsupported".into());
            }
            let s = std::str::from_utf8(data)
                .map_err(|_| "binary data is unsupported by this renderer")?
                .trim_end_matches(['\r', '\n']);
            let delim = syntax.delimiter as char;
            let p: Vec<_> = s.split(delim).collect();
            if field.path.is_some() && !matches!(name, "FS" | "FR" | "FX" | "CC" | "CT" | "CD") {
                return Err("drawing must end with FS before another command".into());
            }
            let max = match name {
                "XA" | "XZ" | "FS" | "FR" => Some(0),
                "CI" | "PW" | "LL" | "LS" | "LT" | "LR" | "PO" | "FH" => Some(1),
                "LH" | "FW" => Some(2),
                "FO" | "FT" | "CF" | "BY" | "XG" => Some(3),
                "GB" | "B3" | "FB" => Some(5),
                "BC" => Some(6),
                "GE" => Some(4),
                "GC" => Some(3),
                n if n.starts_with('A') => Some(3),
                _ => None,
            };
            if max.is_some_and(|n| if n == 0 { !s.is_empty() } else { p.len() > n }) {
                return Err("unexpected command parameters".into());
            }

            match name {
                "CC" | "CT" | "CD" | "FX" => {}
                "CI" => {
                    if s != "0" && s != "27" && s != "28" {
                        return Err("character encoding unsupported".into());
                    }
                }
                "XA" => {
                    if scene.is_some() {
                        return Err("nested label".into());
                    }
                    scene =
                        Some(Scene::new(width, height, options.dpi).map_err(|e| e.to_string())?);
                    font_w = default_w;
                    font_h = default_h;
                    field = Field {
                        rotation: default_rotation,
                        ..Field::default()
                    };
                }
                "XZ" => {
                    if field.path.is_some() {
                        return Err("field must end with FS".into());
                    }
                    let mut sc = scene.take().ok_or("XZ without XA")?;
                    if upside_down {
                        let (w, h) = (sc.width as f64, sc.height as f64);
                        for d in &mut sc.draws {
                            d.path.transform(|p| Point::new(w - p.x, h - p.y));
                        }
                    }
                    labels.push(sc);
                    if labels.len() > 64 {
                        return Err("too many labels (maximum 64)".into());
                    }
                }
                "PW" | "LL" => {
                    let n = number(&p, 0, 0.)?;
                    if n < 1. || n.fract() != 0. {
                        return Err("label dimension must be a positive integer".into());
                    }
                    if name == "PW" {
                        width = n as u32
                    } else {
                        height = n as u32
                    }
                    let fresh =
                        Scene::new(width, height, options.dpi).map_err(|e| e.to_string())?;
                    if let Some(sc) = scene.as_mut() {
                        sc.width = fresh.width;
                        sc.height = fresh.height;
                    }
                }
                "LS" => shift = number(&p, 0, 0.)?,
                "LT" => top = number(&p, 0, 0.)?,
                "LR" => {
                    reverse = match p[0] {
                        "Y" => true,
                        "N" | "" => false,
                        _ => return Err("invalid reverse setting".into()),
                    }
                }
                "PO" => {
                    upside_down = match p[0] {
                        "I" => true,
                        "N" | "" => false,
                        _ => return Err("invalid print orientation".into()),
                    }
                }
                "FW" => {
                    default_rotation = rotation(p[0])?;
                    field.rotation = default_rotation;
                    if number(&p, 1, 0.)? != 0. {
                        return Err("default justification unsupported".into());
                    }
                }
                "FB" => {
                    let width = number(&p, 0, 0.)?;
                    let lines = number(&p, 1, 1.)?;
                    let spacing = number(&p, 2, 0.)?;
                    let align = p.get(3).copied().unwrap_or("L");
                    if width <= 0.
                        || !(1. ..=1000.).contains(&lines)
                        || lines.fract() != 0.
                        || !matches!(align, "" | "L" | "C" | "R")
                        || number(&p, 4, 0.)? != 0.
                    {
                        return Err("unsupported or invalid field block parameters".into());
                    }
                    field.block = Some((
                        width,
                        lines as usize,
                        spacing,
                        align.as_bytes().first().copied().unwrap_or(b'L'),
                    ));
                }
                "LH" => {
                    home_x = number(&p, 0, 0.)?;
                    home_y = number(&p, 1, 0.)?;
                }
                "FO" | "FT" => {
                    field.x = number(&p, 0, 0.)?;
                    field.y = number(&p, 1, 0.)?;
                    field.baseline = name == "FT";
                    if field.baseline && (p[0].is_empty() || p.get(1).is_none_or(|v| v.is_empty()))
                    {
                        return Err("FT requires explicit coordinates".into());
                    }
                    if number(&p, 2, 0.)? != 0. {
                        return Err("field justification unsupported".into());
                    }
                }
                "CF" => {
                    if !matches!(p[0], "0" | "") {
                        return Err("only resident font 0 is embedded; other font selections are unsupported".into());
                    }
                    (font_w, font_h) = font_dimensions(&p, default_w, default_h)?;
                    default_w = font_w;
                    default_h = font_h;
                }
                n if n.starts_with('A') => {
                    if n != "A0" {
                        return Err("only resident font 0 is embedded; other font selections are unsupported".into());
                    }
                    field.rotation = if p[0].is_empty() {
                        default_rotation
                    } else {
                        rotation(p[0])?
                    };
                    (font_w, font_h) = font_dimensions(&p, default_w, default_h)?;
                }
                "FH" => {
                    if s.len() > 1 {
                        return Err("invalid hex indicator".into());
                    }
                    field.hex = Some(s.as_bytes().first().copied().unwrap_or(b'_'));
                }
                "FR" => field.reverse = true,
                "BY" => {
                    module = number(&p, 0, 2.)?;
                    ratio = number(&p, 1, 3.)?;
                    bar_h = number(&p, 2, 100.)?;
                    if !(1. ..=10.).contains(&module) || !(2. ..=3.).contains(&ratio) || bar_h <= 0.
                    {
                        return Err("invalid barcode dimensions".into());
                    }
                }
                "BC" => {
                    field.rotation = rotation(p[0])?;
                    let h = number(&p, 1, bar_h)?;
                    if p.get(2).is_some_and(|v| !matches!(*v, "" | "Y" | "N")) {
                        return Err("invalid barcode text flag".into());
                    }
                    let show = p.get(2).is_none_or(|v| matches!(*v, "" | "Y"));
                    if p.get(3).is_some_and(|v| !matches!(*v, "" | "N"))
                        || p.get(4).is_some_and(|v| !matches!(*v, "" | "N"))
                        || p.get(5).is_some_and(|v| !matches!(*v, "" | "N"))
                    {
                        return Err(
                            "Code 128 above-text, UCC and automatic modes unsupported".into()
                        );
                    }
                    field.barcode = Some((h, show, true));
                }
                "B3" => {
                    field.rotation = rotation(p[0])?;
                    if p.get(1).is_some_and(|v| !matches!(*v, "" | "N")) {
                        return Err("Code 39 checksum unsupported".into());
                    }
                    let h = number(&p, 2, bar_h)?;
                    if p.get(3).is_some_and(|v| !matches!(*v, "" | "Y" | "N")) {
                        return Err("invalid barcode text flag".into());
                    }
                    let show = p.get(3).is_none_or(|v| matches!(*v, "" | "Y"));
                    if p.get(4).is_some_and(|v| !matches!(*v, "" | "N")) {
                        return Err("barcode text above unsupported".into());
                    }
                    field.barcode = Some((h, show, false));
                }
                "FD" | "FV" => {
                    if field.path.is_some() {
                        return Err("multiple drawing commands in one field".into());
                    }
                    let mut bytes = Vec::new();
                    let mut i = 0;
                    while i < data.len() {
                        if field.hex == Some(data[i]) {
                            if i + 2 >= data.len() {
                                return Err("truncated field hex escape".into());
                            }
                            let v = std::str::from_utf8(&data[i + 1..i + 3])
                                .map_err(|_| "invalid hex escape")?;
                            bytes
                                .push(u8::from_str_radix(v, 16).map_err(|_| "invalid hex escape")?);
                            i += 3;
                        } else {
                            bytes.push(data[i]);
                            i += 1;
                        }
                    }
                    if field.barcode.is_none_or(|(_, show, _)| show)
                        && (font_w != 32.
                            || font_h != 32.
                            || options.dpi != 203
                            || field.rotation != b'N')
                        && warnings.is_empty()
                    {
                        warnings.push("Font 0 uses a captured 32-dot, 203-DPI, normal-orientation bitmap strike; other sizes, resolutions or rotations can differ from printer rasterization.".into());
                    }
                    if bytes.len() > 4096 {
                        return Err("field data exceeds 4096-byte renderer limit".into());
                    }
                    let value = std::str::from_utf8(&bytes)
                        .map_err(|_| "only ASCII preview text supported")?;
                    let path = if let Some((h, show, code128)) = field.barcode {
                        if (!code128 && value.contains('*')) || h <= 0. {
                            return Err("invalid Code 39 data or height".into());
                        }
                        field.baseline_height = h;
                        let mut b = if code128 {
                            barcode::code128(value, module, h)?
                        } else {
                            code39(value, module, ratio, h)?
                        };
                        if show {
                            let mut t = font::text(value, font_w, font_h)?;
                            t.transform(|p| Point::new(p.x, p.y + h + 3.));
                            b.segments.extend(t.segments);
                        }
                        b
                    } else {
                        if font_w <= 0. || font_h <= 0. {
                            return Err("font dimensions must be positive".into());
                        }
                        let (path, baseline) = text_block(value, font_w, font_h, field.block)?;
                        field.baseline_height = baseline;
                        field.text_size =
                            Some(if let Some((width, lines, spacing, _)) = field.block {
                                (width, lines as f64 * font_h + (lines - 1) as f64 * spacing)
                            } else {
                                (font::width(value, font_w)?, font_h)
                            });
                        path
                    };
                    field.path = Some(path);
                }
                "GB" | "GE" | "GC" => {
                    let w = number(&p, 0, 1.)?;
                    let h = if name == "GC" { w } else { number(&p, 1, 1.)? };
                    let ti = if name == "GC" { 1 } else { 2 };
                    let t = number(&p, ti, 1.)?;
                    if w <= 0. || h <= 0. || t <= 0. {
                        return Err("invalid shape dimensions".into());
                    }
                    if p.get(ti + 1).is_some_and(|v| !matches!(*v, "" | "B")) {
                        return Err("white shapes unsupported".into());
                    }
                    let mut path = Path::default();
                    if name == "GB" {
                        let r = number(&p, 4, 0.)?;
                        if !(0. ..=8.).contains(&r) {
                            return Err("invalid corner rounding".into());
                        }
                        let radius = r / 8. * w.min(h) / 2.;
                        path.rounded_rect(0., 0., w, h, radius);
                        path.rounded_rect(t, t, w - 2. * t, h - 2. * t, (radius - t).max(0.));
                    } else {
                        path.ellipse(0., 0., w, h);
                        if w > 2. * t && h > 2. * t {
                            path.ellipse(t, t, w - 2. * t, h - 2. * t)
                        }
                    }
                    field.path = Some(path);
                }
                "DG" => {
                    if p.len() < 4 {
                        return Err("invalid graphic download".into());
                    }
                    let n = count(&p, 1)?;
                    let row = count(&p, 2)?;
                    graphics.insert(
                        p[0].to_string(),
                        graphics::decode(
                            p[3..].join(&delim.to_string()).as_bytes(),
                            n,
                            row,
                            false,
                        )?,
                    );
                }
                "GF" => {
                    if p.first() != Some(&"A") || p.len() < 5 {
                        return Err("unsupported GF mode (use A or B)".into());
                    }
                    let n = count(&p, 2)?;
                    let row = count(&p, 3)?;
                    field.path = Some(graphics::decode(
                        p[4..].join(&delim.to_string()).as_bytes(),
                        n,
                        row,
                        false,
                    )?);
                }
                "XG" => {
                    let mut path = graphics
                        .get(p[0])
                        .ok_or("unknown downloaded graphic")?
                        .clone();
                    let x = number(&p, 1, 1.)?;
                    let y = number(&p, 2, 1.)?;
                    if !(1. ..=10.).contains(&x) || !(1. ..=10.).contains(&y) {
                        return Err("invalid graphic magnification".into());
                    }
                    path.transform(|p| Point::new(p.x * x, p.y * y));
                    field.path = Some(path);
                }
                "FS" => {
                    if let Some(mut path) = field.path.take() {
                        let sc = scene.as_mut().ok_or("field outside label")?;
                        let (x, y) = (field.x + home_x - shift, field.y + home_y + top);
                        let (_, _, w, h) = bounds(&path);
                        let base = if field.baseline {
                            if field.baseline_height > 0. {
                                field.baseline_height
                            } else {
                                h
                            }
                        } else {
                            0.
                        };
                        path.transform(|p| {
                            let (xp, yp) = (p.x, p.y - base);
                            let (a, b) = match field.rotation {
                                b'R' => (-yp, xp),
                                b'I' => (-xp, -yp),
                                b'B' => (yp, -xp),
                                _ => (xp, yp),
                            };
                            let (dx, dy) = if field.baseline {
                                (0., 0.)
                            } else {
                                let (w, h) = field
                                    .text_size
                                    .map(|(w, h)| ((w - 1.).max(0.), (h - 1.).max(0.)))
                                    .unwrap_or((w, h));
                                match field.rotation {
                                    b'R' => (h, 0.),
                                    b'I' => (w, h),
                                    b'B' => (0., w),
                                    _ => (0., 0.),
                                }
                            };
                            Point::new(x + a + dx, y + b + dy)
                        });
                        total_segments += path.segments.len();
                        if total_segments > crate::output::MAX_SEGMENTS {
                            return Err("document path limit exceeded".into());
                        }
                        sc.draws.push(Draw {
                            path,
                            paint: if field.reverse || reverse {
                                Paint::Invert
                            } else {
                                Paint::Black
                            },
                        });
                        sc.validate().map_err(|e| e.to_string())?;
                    }
                    font_w = default_w;
                    font_h = default_h;
                    field = Field {
                        rotation: default_rotation,
                        ..Field::default()
                    };
                }
                _ => return Err(format!("unsupported command {name}")),
            }
            if graphics.values().map(|p| p.segments.len()).sum::<usize>()
                > crate::output::MAX_SEGMENTS
            {
                return Err("downloaded graphics path limit exceeded".into());
            }
            Ok(())
        })();
        result.map_err(|message| RenderError { offset, message })?;
    }
    if scene.is_some() {
        return Err(RenderError {
            offset: input.len(),
            message: "unterminated label (missing XZ)".into(),
        });
    }
    if labels.is_empty() {
        return Err(RenderError {
            offset: input.len(),
            message: "no labels".into(),
        });
    }
    Ok(Document { labels, warnings })
}
fn bounds(path: &Path) -> (f64, f64, f64, f64) {
    let (mut x, mut y, mut right, mut bottom) = (0f64, 0f64, 0f64, 0f64);
    let mut add = |p: &Point| {
        x = x.min(p.x);
        y = y.min(p.y);
        right = right.max(p.x);
        bottom = bottom.max(p.y);
    };
    for s in &path.segments {
        match s {
            crate::output::Segment::Move(p) | crate::output::Segment::Line(p) => add(p),
            crate::output::Segment::Cubic(a, b, c) => {
                add(a);
                add(b);
                add(c)
            }
            _ => {}
        }
    }
    (x, y, right - x, bottom - y)
}
fn text_block(
    value: &str,
    w: f64,
    h: f64,
    block: Option<(f64, usize, f64, u8)>,
) -> Result<(Path, f64), String> {
    let Some((width, max_lines, spacing, align)) = block else {
        return Ok((font::text(value, w, h)?, font::baseline(h)));
    };
    if spacing < 0. {
        return Err("overlapping field block lines unsupported".into());
    }
    let mut lines = Vec::new();
    for paragraph in value.split("\\&") {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if font::width(word, w)? > width {
                return Err("field block word hyphenation unsupported".into());
            }
            let next = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && font::width(&next, w)? > width {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = next;
            }
        }
        lines.push(line);
    }
    if lines.len() > max_lines {
        return Err("field block overflow unsupported".into());
    }
    let mut path = Path::default();
    for (i, line) in lines.iter().enumerate() {
        let mut p = font::text(line, w, h)?;
        let slack = width - font::width(line, w)?;
        let x = match align {
            b'C' => slack / 2.,
            b'R' => slack,
            _ => 0.,
        };
        p.transform(|p| Point::new(p.x + x, p.y + i as f64 * (h + spacing)));
        path.segments.extend(p.segments);
    }
    Ok((
        if lines.len() > 1 {
            font::union_lines(path)
        } else {
            path
        },
        (max_lines - 1) as f64 * (h + spacing) + font::baseline(h),
    ))
}
fn font_dimensions(p: &[&str], default_w: f64, default_h: f64) -> Result<(f64, f64), String> {
    let h = number(p, 1, 0.)?;
    let w = number(p, 2, 0.)?;
    let (w, h) = match (w, h) {
        (0., 0.) => (default_w, default_h),
        (0., h) => (h, h),
        (w, 0.) => (w, w),
        (w, h) => (w, h),
    };
    if w <= 0. || h <= 0. {
        return Err("font dimensions must be positive".into());
    }
    Ok((w, h))
}

fn count(p: &[&str], i: usize) -> Result<usize, String> {
    let n = number(p, i, 0.)?;
    if n < 1. || n.fract() != 0. {
        return Err("invalid graphic count".into());
    }
    Ok(n as usize)
}
