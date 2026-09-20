//! Local, deterministic ZPL previews. Unsupported rendering semantics are errors.
mod barcode;
mod barcode_edges;
pub mod compatibility;
pub mod profiles;
use raster_diff::compression;
mod font;
mod graphics;
mod printer_shapes;
mod validation;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    /// Explicit printer departures and quantization choices. See [`profiles`].
    pub compatibility: compatibility::Compatibility,
}
/// Defaults to the captured ZD621 profile. Use [`profiles::SPECIFICATION`]
/// explicitly to disable all printer compatibility overrides.
impl Default for Options {
    fn default() -> Self {
        profiles::ZD621_203_DPI
    }
}
#[derive(Debug)]
pub struct Document {
    pub labels: Vec<Scene>,
    pub warnings: Vec<String>,
}
struct TextLayout {
    path: Path,
    baseline: f64,
    parts: Vec<Path>,
}
#[derive(Clone)]
struct Field {
    x: f64,
    y: f64,
    baseline: bool,
    rotation: u8,
    justification: u8,
    reverse: bool,
    white: bool,
    explicit_font: bool,
    hex: Option<u8>,
    barcode: Option<barcode::Barcode>,
    barcode_error: Option<String>,
    path: Option<Path>,
    origins: Option<Vec<Option<(f64, f64)>>>,
    multiple_paths: Option<Vec<(f64, f64, Path)>>,
    block: Option<(f64, usize, f64, u8, f64)>,
    barcode_split: Vec<barcode_edges::PartBoundary>,
    barcode_width: f64,
    baseline_height: f64,
    inverted_margin: f64,
    text_size: Option<(f64, f64)>,
    text_parts: Vec<Path>,
    graphic_size: Option<(f64, f64)>,
    graphic_bitmap: bool,
}
impl Default for Field {
    fn default() -> Self {
        Self {
            x: 0.,
            y: 0.,
            baseline: false,
            rotation: b'N',
            justification: 0,
            reverse: false,
            white: false,
            explicit_font: false,
            hex: None,
            barcode: None,
            barcode_error: None,
            path: None,
            origins: None,
            multiple_paths: None,
            block: None,
            barcode_split: Vec::new(),
            barcode_width: 0.,
            baseline_height: 0.,
            inverted_margin: 0.,
            text_size: None,
            text_parts: Vec::new(),
            graphic_size: None,
            graphic_bitmap: false,
        }
    }
}
fn justification(p: &[&str], i: usize, default: u8) -> Result<u8, String> {
    let n = number(p, i, default as f64)?;
    if !matches!(n, 0. | 1. | 2.) {
        return Err("invalid field justification".into());
    }
    Ok(n as u8)
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
    let (mut font_id, mut default_font_id) = ('0', '0');
    let (mut font_w, mut font_h) = (20., 20.);
    let (mut default_w, mut default_h) = (20., 20.);
    // Retain CF's requested dimensions as well as its resolved font size.
    // In particular, a zero axis means proportional sizing for another font.
    let (mut default_requested_w, mut default_requested_h) = (0., 20.);
    // Zebra Programming Guide ^BY, p. 148: initial module width 2 dots,
    // ratio 3, and height 10 dots. Later omitted operands retain their values.
    let (mut module, mut ratio, mut bar_h) = (2., 3., 10.);
    let mut graphics = HashMap::<String, (Path, (f64, f64))>::new();
    let mut warnings = Vec::new();
    let (mut shift, mut top) = (0., 0.);
    let mut encoding = 0;
    let mut default_rotation = b'N';
    let mut default_justification = 0;
    let mut reverse = false;
    let mut code_validation = false;
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
                field.graphic_size = Some(((row * 8) as f64, (count / row) as f64));
                field.graphic_bitmap = true;
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
                "GB" | "GD" | "B3" | "FB" => Some(5),
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
                    encoding = s.parse::<u8>().unwrap();
                }
                "XA" => {
                    if scene.is_some() {
                        return Err("nested label".into());
                    }
                    scene =
                        Some(Scene::new(width, height, options.dpi).map_err(|e| e.to_string())?);
                    font_id = default_font_id;
                    font_w = default_w;
                    font_h = default_h;
                    field = Field {
                        rotation: default_rotation,
                        justification: default_justification,
                        ..Field::default()
                    };
                }
                "XZ" => {
                    if field.path.is_some() {
                        return Err("field must end with FS".into());
                    }
                    let mut sc = scene.take().ok_or("XZ without XA")?;
                    if upside_down && !options.compatibility.preview_ignores_print_orientation {
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
                    default_justification = justification(&p, 1, 0)?;
                    field.justification = default_justification;
                }
                "FB" => {
                    // ^FB selects text layout, replacing a preceding ^GS symbol field.
                    // ZD621 controls in graphic-symbols-zd621-v1 retain GS dimensions.
                    if font_id == 'S' {
                        font_id = default_font_id;
                    }
                    let width = number(&p, 0, 0.)?;
                    let lines = number(&p, 1, 1.)?;
                    let spacing = number(&p, 2, 0.)?;
                    let align = p.get(3).copied().unwrap_or("L");
                    if width <= 0.
                        || !(1. ..=1000.).contains(&lines)
                        || lines.fract() != 0.
                        || !matches!(align, "" | "L" | "C" | "R" | "J")
                        || !(0. ..width).contains(&number(&p, 4, 0.)?)
                    {
                        return Err("unsupported or invalid field block parameters".into());
                    }
                    field.block = Some((
                        width,
                        lines as usize,
                        spacing,
                        align.as_bytes().first().copied().unwrap_or(b'L'),
                        number(&p, 4, 0.)?,
                    ));
                }
                "LH" => {
                    home_x = number(&p, 0, 0.)?;
                    home_y = number(&p, 1, 0.)?;
                }
                "CV" => {
                    // Zebra guide p. 167: persists across formats within this
                    // render call, until ^CVN. No process-global printer state.
                    code_validation = match p.as_slice() {
                        ["Y"] => true,
                        ["N" | ""] => false,
                        _ => return Err("invalid code validation setting".into()),
                    };
                }
                "FM" => {
                    if p.len() % 2 != 0 || p.is_empty() || p.len() > 120 {
                        return Err("FM requires 1 through 60 coordinate pairs".into());
                    }
                    let mut origins = Vec::new();
                    for pair in p.as_chunks::<2>().0 {
                        let mut values = [0.; 2];
                        let mut excluded = false;
                        for i in 0..2 {
                            if pair[i] == "e" {
                                excluded = true;
                                continue;
                            }
                            if pair[i].is_empty() {
                                return Err("FM requires explicit coordinates".into());
                            }
                            values[i] = number(pair, i, 0.)?;
                            if !(0. ..=32000.).contains(&values[i]) || values[i].fract() != 0. {
                                return Err("invalid FM coordinate".into());
                            }
                        }
                        origins.push((!excluded).then_some((values[0], values[1])));
                    }
                    field.origins = Some(origins);
                }
                "FO" | "FT" => {
                    field.origins = None;
                    field.x = number(&p, 0, 0.)?;
                    field.y = number(&p, 1, 0.)?;
                    field.baseline = name == "FT";
                    if field.baseline && (p[0].is_empty() || p.get(1).is_none_or(|v| v.is_empty()))
                    {
                        return Err("FT requires explicit coordinates".into());
                    }
                    field.justification = justification(&p, 2, default_justification)?;
                }
                "CF" => {
                    font_id = match p[0] {
                        "" => default_font_id,
                        "0" => '0',
                        "A" => 'A',
                        "B" => 'B',
                        "C" => 'C',
                        "D" => 'D',
                        "E" => 'E',
                        "F" => 'F',
                        "G" => 'G',
                        "H" => 'H',
                        _ => return Err("unsupported resident font".into()),
                    };
                    default_font_id = font_id;
                    // ^CF p. 154: omitted dimensions retain the last CF request;
                    // explicit zero is a supplied value, not omission.
                    let supplied_size = p.get(1).is_some_and(|v| !v.is_empty())
                        || p.get(2).is_some_and(|v| !v.is_empty());
                    if options.compatibility.bitmap_cf_font_only_resets_size
                        && font_id != '0'
                        && !p[0].is_empty()
                        && !supplied_size
                    {
                        default_requested_h = 0.;
                        default_requested_w = 0.;
                    } else if supplied_size {
                        default_requested_h = number(&p, 1, 0.)?;
                        default_requested_w = number(&p, 2, 0.)?;
                    }
                    let (dw, dh) = if font_id == '0' {
                        (default_w, default_h)
                    } else {
                        (default_requested_w, default_requested_h)
                    };
                    (font_w, font_h) = font_dimensions(&p, dw, dh, font_id)?;
                    default_w = font_w;
                    default_h = font_h;
                }
                n if n.starts_with('A') => {
                    field.explicit_font = true;
                    font_id = match n {
                        "A0" => '0',
                        "AA" => 'A',
                        "AB" => 'B',
                        "AC" => 'C',
                        "AD" => 'D',
                        "AE" => 'E',
                        "AF" => 'F',
                        "AG" => 'G',
                        "AH" => 'H',
                        _ => return Err("unsupported resident font".into()),
                    };
                    field.rotation = if p[0].is_empty() {
                        default_rotation
                    } else {
                        rotation(p[0])?
                    };
                    let (dw, dh) = if font_id == '0' {
                        (default_w, default_h)
                    } else {
                        (default_requested_w, default_requested_h)
                    };
                    (font_w, font_h) = font_dimensions(&p, dw, dh, font_id)?;
                }
                "GS" => {
                    // ^GS p. 217 selects a separate symbol face, not ^AS.
                    font_id = 'S';
                    field.barcode = None;
                    field.explicit_font = false;
                    field.rotation = if p[0].is_empty() {
                        default_rotation
                    } else {
                        rotation(p[0])?
                    };
                    (font_w, font_h) =
                        font_dimensions(&p, default_requested_w, default_requested_h, font_id)?;
                }
                "FH" => {
                    if s.len() > 1 {
                        return Err("invalid hex indicator".into());
                    }
                    field.hex = Some(s.as_bytes().first().copied().unwrap_or(b'_'));
                }
                "FR" => field.reverse = true,
                "BY" => {
                    module = number(&p, 0, module)?;
                    ratio = number(&p, 1, ratio)?;
                    bar_h = number(&p, 2, bar_h)?;
                    if !(1. ..=10.).contains(&module) || !(2. ..=3.).contains(&ratio) || bar_h <= 0.
                    {
                        return Err("invalid barcode dimensions".into());
                    }
                }
                n if barcode::supported(n) => {
                    field.rotation = if n == "BD" || n == "BQ" {
                        b'N'
                    } else if p[0].is_empty() {
                        if n == "BR" {
                            b'R'
                        } else if n == "BB" {
                            b'N'
                        } else {
                            default_rotation
                        }
                    } else {
                        rotation(p[0])?
                    };
                    let barcode = barcode::Barcode::new(
                        n,
                        &p,
                        module,
                        ratio,
                        bar_h,
                        options.dpi,
                        options.compatibility,
                    );
                    match barcode {
                        Ok(barcode) => {
                            field.barcode = Some(barcode);
                            field.barcode_error = None;
                        }
                        Err(error) if code_validation && validation::classify(&error).is_some() => {
                            field.barcode = None;
                            field.barcode_error = Some(error);
                        }
                        Err(error) => return Err(error),
                    }
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
                    if field.barcode.as_ref().is_none_or(|b| b.show)
                        && (font_w != 32.
                            || font_h != 32.
                            || options.dpi != 203
                            || field.rotation != b'N')
                        && warnings.is_empty()
                    {
                        warnings.push("Resident fonts use captured bitmap strikes; unsampled sizes, resolutions or rotations can differ from printer rasterization.".into());
                    }
                    if bytes.len() > 4096 {
                        return Err("field data exceeds 4096-byte renderer limit".into());
                    }
                    let decoded;
                    let value = if field.barcode.is_some() || field.barcode_error.is_some() {
                        ""
                    } else if encoding == 27 {
                        // ^CI27 uses Windows-1252. ASCII and U+00A0–00FF map
                        // directly; C1 mappings are rejected until glyphs exist.
                        if bytes.iter().any(|b| (128..160).contains(b)) {
                            return Err("unsupported Windows-1252 glyph".into());
                        }
                        decoded = bytes.iter().map(|&b| char::from(b)).collect::<String>();
                        &decoded
                    } else {
                        if encoding == 0 && !bytes.is_ascii() {
                            return Err(
                                "unsupported legacy text byte; select ^CI28 for UTF-8".into()
                            );
                        }
                        std::str::from_utf8(&bytes).map_err(|_| "invalid UTF-8 text")?
                    };
                    let rendered = (|| -> Result<Path, String> {
                        if let Some(error) = &field.barcode_error {
                            return Err(error.clone());
                        }
                        let path = if let Some(b) = &field.barcode {
                            if let Some(origins) =
                                field.origins.as_ref().filter(|_| b.uses_multiple_origins())
                            {
                                field.baseline = false;
                                let paths = b.render_multiple(&bytes, origins.len())?;
                                let (dx, dy) = b.multiple_origin_offset(field.rotation);
                                field.multiple_paths = Some(
                                    paths
                                        .into_iter()
                                        .zip(origins)
                                        .filter_map(|(path, origin)| {
                                            origin.map(|(x, y)| (x + dx, y + dy, path))
                                        })
                                        .collect(),
                                );
                                return Ok(Path::default());
                            }
                            let rendered = b.render(
                                &bytes,
                                field.explicit_font.then_some((font_id, font_w, font_h)),
                                field.rotation,
                            )?;
                            let mut path = rendered.path;
                            if !field.baseline {
                                let offset = b.field_origin_y();
                                path.transform(|p| Point::new(p.x, p.y + offset));
                            }
                            field.barcode_split = rendered.split;
                            field.barcode_width = rendered.width;
                            field.baseline_height = rendered.baseline;
                            path
                        } else {
                            if font_w <= 0. || font_h <= 0. {
                                return Err("font dimensions must be positive".into());
                            }
                            let TextLayout {
                                path,
                                baseline,
                                parts,
                            } = text_block(
                                font_id,
                                value,
                                font_w,
                                font_h,
                                field.block,
                                options.compatibility,
                                encoding,
                            )?;
                            field.text_parts = parts;
                            // Table 29 p. 1582 gives GS a 3/4-height baseline.
                            // Printer controls instead use native row 23 of 24.
                            field.baseline_height = if font_id == 'S'
                                && !options.compatibility.graphic_symbol_last_row_baseline
                            {
                                baseline - font_h * 5. / 24.
                            } else {
                                baseline
                            };
                            if options
                                .compatibility
                                .right_justified_inverted_text_uses_ink_margin
                            {
                                field.inverted_margin =
                                    font::inverted_margin(font_id, value, font_w, font_h)?;
                            }
                            field.text_size =
                                Some(if let Some((width, lines, spacing, _, _)) = field.block {
                                    (width, lines as f64 * font_h + (lines - 1) as f64 * spacing)
                                } else {
                                    (font::width_for(font_id, value, font_w, font_h)?, font_h)
                                });
                            path
                        };
                        Ok(path)
                    })();
                    let path = match rendered {
                        Ok(path) => path,
                        Err(error)
                            if code_validation
                                && (field.barcode.is_some() || field.barcode_error.is_some()) =>
                        {
                            let mut code =
                                validation::classify(&error).ok_or_else(|| error.clone())?;
                            if code == b'L'
                                && ((options.compatibility.validation_retail_long_is_short
                                    && error.starts_with("retail"))
                                    || (options.compatibility.validation_legacy_small_is_parameter
                                        && error.starts_with("legacy Data Matrix")
                                        && error.contains("does not fit")))
                            {
                                code = if error.starts_with("retail") {
                                    b'S'
                                } else {
                                    b'P'
                                };
                            }
                            field.baseline_height = 30. * options.dpi as f64 / 203.;
                            validation::render(code, options.dpi)
                        }
                        Err(error) => return Err(error),
                    };
                    field.path = Some(path);
                }
                "GD" => {
                    // Zebra Programming Guide, ^GD, p. 213. The ZD621 draws
                    // horizontal runs, with the x accumulator advanced before
                    // each row. Integer-slope and fractional-slope captures are
                    // in tests/fixtures/printer-accuracy (see its provenance).
                    let t = number(&p, 2, 1.)?;
                    let w = number(&p, 0, t.max(3.))?;
                    let h = number(&p, 1, t.max(3.))?;
                    if !(3. ..=32000.).contains(&w)
                        || !(3. ..=32000.).contains(&h)
                        || !(1. ..=32000.).contains(&t)
                        || [w, h, t].iter().any(|v| v.fract() != 0.)
                    {
                        return Err("invalid diagonal dimensions".into());
                    }
                    if p.get(3).is_some_and(|v| !matches!(*v, "" | "B" | "W")) {
                        return Err("invalid shape color".into());
                    }
                    field.white = p.get(3) == Some(&"W");
                    let left = match p.get(4).copied().unwrap_or("") {
                        "" | "R" | "/" => false,
                        "L" | "\\" => true,
                        _ => return Err("invalid diagonal orientation".into()),
                    };
                    let mut path = Path::default();
                    if options.compatibility.diagonal_dot_runs {
                        let step = ((w as u64) << 16) / h as u64;
                        for y in 0..h as u64 {
                            let x = (((y + 1) * step) >> 16) as f64;
                            path.rect(
                                if left { x } else { w - x },
                                y as f64,
                                t.max((w / h).floor()),
                                1.,
                            );
                        }
                    } else {
                        path = graphics::diagonal(w, h, t, left);
                    }
                    field.path = Some(path);
                    field.graphic_size = Some((w, h));
                }
                "GB" | "GE" | "GC" => {
                    let ti = if name == "GC" { 1 } else { 2 };
                    let t = number(&p, ti, 1.)?;
                    // Zebra Programming Guide, ^GB, p. 210: omitted dimensions
                    // default to thickness; adjust dimensions to at least t
                    // before calculating rounding (including the zero-width example).
                    let default_dimension = if name == "GB" { t } else { 1. };
                    let w = number(&p, 0, default_dimension)?;
                    let h = if name == "GC" {
                        w
                    } else {
                        number(&p, 1, default_dimension)?
                    };
                    if t <= 0. || w < 0. || h < 0. || (name != "GB" && (w == 0. || h == 0.)) {
                        return Err("invalid shape dimensions".into());
                    }
                    if p.get(ti + 1).is_some_and(|v| !matches!(*v, "" | "B" | "W")) {
                        return Err("invalid shape color".into());
                    }
                    // Zebra Programming Guide, ^GB/^GC/^GE, pp. 210–214:
                    // W paints white; it does not toggle the pixels underneath.
                    field.white = p.get(ti + 1) == Some(&"W");
                    let mut path = Path::default();
                    field.graphic_size = Some((w, h));
                    if name == "GB" {
                        let r = number(&p, 4, 0.)?;
                        if !(0. ..=8.).contains(&r) {
                            return Err("invalid corner rounding".into());
                        }
                        let printer_geometry =
                            r > 0. && options.compatibility.rounded_box_printer_geometry;
                        let t = if printer_geometry { t.max(2.) } else { t };
                        let (w, h) = (w.max(t), h.max(t));
                        field.graphic_size = Some((w, h));
                        let radius = r / 8. * w.min(h) / 2.;
                        let inner_radius = if printer_geometry {
                            (r / 16. * (w - 2. * t).min(h - 2. * t)).floor().max(0.)
                        } else {
                            (radius - t).max(0.)
                        };
                        if r > 0. && options.compatibility.rounded_box_printer_curve {
                            let quantize = |radius: f64| {
                                if printer_geometry {
                                    radius.floor().max(2.)
                                } else {
                                    radius.floor()
                                }
                            };
                            printer_shapes::rounded_rect(
                                &mut path,
                                0.,
                                0.,
                                w,
                                h,
                                quantize(radius),
                            )?;
                            printer_shapes::rounded_rect(
                                &mut path,
                                t,
                                t,
                                w - 2. * t,
                                h - 2. * t,
                                quantize(inner_radius),
                            )?;
                        } else {
                            path.rounded_rect(0., 0., w, h, radius);
                            path.rounded_rect(t, t, w - 2. * t, h - 2. * t, inner_radius);
                        }
                    } else if w == h && options.compatibility.circle_printer_curve {
                        printer_shapes::circle(&mut path, w, t)?;
                    } else if name == "GE" && w != h && options.compatibility.ellipse_printer_curve
                    {
                        printer_shapes::ellipse(&mut path, w, h, t)?;
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
                        (
                            graphics::decode(
                                p[3..].join(&delim.to_string()).as_bytes(),
                                n,
                                row,
                                false,
                            )?,
                            ((row * 8) as f64, (n / row) as f64),
                        ),
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
                    field.graphic_size = Some(((row * 8) as f64, (n / row) as f64));
                    field.graphic_bitmap = true;
                }
                "XG" => {
                    let (mut path, graphic_size) = graphics
                        .get(p[0])
                        .ok_or("unknown downloaded graphic")?
                        .clone();
                    let x = number(&p, 1, 1.)?;
                    let y = number(&p, 2, 1.)?;
                    if !(1. ..=10.).contains(&x) || !(1. ..=10.).contains(&y) {
                        return Err("invalid graphic magnification".into());
                    }
                    path.transform(|p| Point::new(p.x * x, p.y * y));
                    field.graphic_size = Some((graphic_size.0 * x, graphic_size.1 * y));
                    field.graphic_bitmap = true;
                    field.path = Some(path);
                }
                "FS" => {
                    // ^FW p. 208 applies rotation to commands with an orientation
                    // parameter. GB/GC/GE/GD/GF/XG have no such parameter.
                    if field.graphic_size.is_some() {
                        field.rotation = b'N';
                    }
                    let paths = field.multiple_paths.take().unwrap_or_else(|| {
                        field
                            .path
                            .take()
                            .map(|path| vec![(field.x, field.y, path)])
                            .unwrap_or_default()
                    });
                    for (origin_x, origin_y, mut path) in paths {
                        let sc = scene.as_mut().ok_or("field outside label")?;
                        let (x, y) = (
                            origin_x + home_x - shift,
                            origin_y
                                + home_y
                                + if options.compatibility.preview_ignores_label_top {
                                    0.
                                } else {
                                    top
                                },
                        );
                        let (x, y) = if options.compatibility.text_clamps_negative_origins
                            && field.text_size.is_some()
                        {
                            (x.max(0.), y.max(0.))
                        } else {
                            (x, y)
                        };
                        let (_, _, w, h) = bounds(&path);
                        let left = path
                            .segments
                            .iter()
                            .filter_map(|s| match s {
                                crate::output::Segment::Move(p)
                                | crate::output::Segment::Line(p) => Some(p.x),
                                _ => None,
                            })
                            .reduce(f64::min)
                            .unwrap_or(0.);
                        let base = if field.baseline {
                            if field.baseline_height > 0. {
                                field.baseline_height
                            } else {
                                // ^FT p. 205 Table 7 anchors the graphic area,
                                // including blank bitmap rows, rather than ink.
                                field.graphic_size.map_or(h, |(_, h)| h)
                            }
                        } else {
                            0.
                        };
                        let base = if field.baseline
                            && options.compatibility.graphic_ft_last_row_baseline
                        {
                            field.graphic_size.map_or(base, |(_, h)| (h - 1.).max(0.))
                        } else {
                            base
                        };
                        let base = if field.baseline {
                            field.barcode.as_ref().map_or(base, |barcode| {
                                barcode.field_baseline_height(base, field.rotation)
                            })
                        } else {
                            base
                        };
                        let (ft_dx, ft_dy) = if options.compatibility.bitmap_font_ft_dot_origin
                            && field.baseline
                            && field.text_size.is_some()
                        {
                            font::printer_ft_offset(font_id, font_h, field.rotation)
                        } else {
                            (0., 0.)
                        };
                        let field_justification = if font_id == 'S'
                            && field.text_size.is_some()
                            && options.compatibility.graphic_symbol_ignores_justification
                        {
                            0
                        } else {
                            field.justification
                        };
                        let transform = |p: Point| {
                            // ^FO/^FT pp. 201/205: right justification changes
                            // the origin, not the character order. Auto (2) is
                            // left for the supported Latin scripts.
                            let advance = field.text_size.or(field.graphic_size).map_or(w, |s| s.0);
                            let xp = p.x
                                - if field.baseline && field_justification == 1 {
                                    advance
                                } else {
                                    0.
                                };
                            let yp = p.y - base;
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
                                    .map(|(w, h)| {
                                        if font_id == '0' {
                                            ((w - 1.).max(0.), (h - 1.).max(0.))
                                        } else {
                                            (w, h)
                                        }
                                    })
                                    .unwrap_or((w, h));
                                // ZD621 ^FO rotations pivot about the bar height,
                                // not the combined bars/interpretation extent.
                                // ^FM places its component paths separately.
                                let h = if options.compatibility.barcode_fo_uses_bar_height
                                    && field.barcode.is_some()
                                    && field.origins.is_none()
                                {
                                    field.baseline_height
                                } else {
                                    h
                                };
                                let w = field.barcode.as_ref().map_or(w, |barcode| {
                                    barcode.field_origin_width(w, field.barcode_width)
                                });
                                match field.rotation {
                                    b'R' => (h, 0.),
                                    b'I' => (w, h),
                                    b'B' => (0., w),
                                    _ => (0., 0.),
                                }
                            };
                            let (jx, jy) = if !field.baseline && field_justification == 1 {
                                if let Some((tw, th)) = field.text_size {
                                    match field.rotation {
                                        b'B' if field.block.is_some()
                                            && options
                                                .compatibility
                                                .block_fo_right_justification_printer_layout =>
                                        {
                                            (-th + font_h, 0.)
                                        }
                                        b'I' if field.block.is_some()
                                            && options
                                                .compatibility
                                                .block_fo_right_justification_printer_layout =>
                                        {
                                            (1. - tw, 0.)
                                        }
                                        b'R' => (-th, -left),
                                        b'I' => (field.inverted_margin - dx + left, 0.),
                                        b'B' => (-th, 0.),
                                        _ => (-tw, 0.),
                                    }
                                } else {
                                    (
                                        if matches!(field.rotation, b'R' | b'B') {
                                            -h
                                        } else {
                                            -field.graphic_size.map_or(w, |s| s.0)
                                        },
                                        0.,
                                    )
                                }
                            } else {
                                (0., 0.)
                            };
                            let (tx, ty) = (x + a + dx + jx + ft_dx, y + b + dy + jy + ft_dy);
                            if field.graphic_size.is_some()
                                && options.compatibility.graphic_clamps_negative_origin
                            {
                                // Clamp the complete field origin, not each dot;
                                // preserve the graphic at the edge (^FO/^FT/^LS).
                                let gx = if field.graphic_bitmap && !field.baseline {
                                    // FO bitmap justification follows origin clamping.
                                    x.max(0.) + a - p.x + dx + jx
                                } else {
                                    (tx - p.x).max(0.)
                                };
                                let gy = ty - p.y;
                                let gy = if field.baseline
                                    && options.compatibility.graphic_ft_last_row_baseline
                                    && gy <= 1.
                                {
                                    0.
                                } else {
                                    gy.max(0.)
                                };
                                Point::new(gx + p.x, gy + p.y)
                            } else {
                                Point::new(tx, ty)
                            }
                        };
                        if options.compatibility.text_clamps_negative_origins
                            && !field.text_parts.is_empty()
                        {
                            let shifts: Vec<_> = field
                                .text_parts
                                .iter()
                                .map(|part| {
                                    let (min_x, min_y) = part
                                        .segments
                                        .iter()
                                        .filter_map(|s| match s {
                                            crate::output::Segment::Move(p)
                                            | crate::output::Segment::Line(p) => {
                                                Some(transform(*p))
                                            }
                                            _ => None,
                                        })
                                        .fold((f64::INFINITY, f64::INFINITY), |(x, y), p| {
                                            (x.min(p.x), y.min(p.y))
                                        });
                                    ((-min_x).max(0.), (-min_y).max(0.))
                                })
                                .collect();
                            if shifts.iter().any(|&(dx, dy)| dx > 0. || dy > 0.) {
                                let mut shifted = Path::default();
                                for (part, (dx, dy)) in field.text_parts.iter().zip(shifts) {
                                    // Clamp glyph ink after anchoring the field. Move
                                    // back in unrotated coordinates so overlapping
                                    // glyphs can be unioned before the common transform.
                                    let (dx, dy) = match field.rotation {
                                        b'R' => (dy, -dx),
                                        b'I' => (-dx, -dy),
                                        b'B' => (-dy, dx),
                                        _ => (dx, dy),
                                    };
                                    let mut component = part.clone();
                                    component.transform(|p| Point::new(p.x + dx, p.y + dy));
                                    shifted.segments.extend(component.segments);
                                }
                                path = font::union_lines(shifted);
                            }
                        }
                        path.transform(transform);
                        if options.compatibility.linear_barcode_rotated_edge_loses_dot {
                            if let Some(part) = field.barcode_split.get(1) {
                                barcode_edges::trim_rotated_boundary(
                                    &mut path,
                                    part.end,
                                    field.rotation,
                                );
                            }
                        }
                        if options.compatibility.linear_barcode_clamps_negative_ink {
                            barcode_edges::clamp(
                                &mut path,
                                &field.barcode_split,
                                field.reverse || reverse,
                                field.rotation,
                            )?;
                        }
                        total_segments += path.segments.len();
                        if total_segments > crate::output::MAX_SEGMENTS {
                            return Err("document path limit exceeded".into());
                        }
                        sc.draws.push(Draw {
                            path,
                            paint: if field.reverse || reverse {
                                Paint::Invert
                            } else if field.white {
                                Paint::White
                            } else {
                                Paint::Black
                            },
                        });
                        sc.validate().map_err(|e| e.to_string())?;
                    }
                    font_id = default_font_id;
                    font_w = default_w;
                    font_h = default_h;
                    field = Field {
                        rotation: default_rotation,
                        justification: default_justification,
                        ..Field::default()
                    };
                }
                _ => return Err(format!("unsupported command {name}")),
            }
            if graphics
                .values()
                .map(|(p, _)| p.segments.len())
                .sum::<usize>()
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
    font_id: char,
    value: &str,
    w: f64,
    h: f64,
    block: Option<(f64, usize, f64, u8, f64)>,
    compatibility: compatibility::Compatibility,
    encoding: u8,
) -> Result<TextLayout, String> {
    let center_space = compatibility.block_center_includes_trailing_space;
    let Some((width, max_lines, spacing, align, indent)) = block else {
        let path = font::text_for(font_id, value, w, h)?;
        let parts = if compatibility.text_clamps_negative_origins {
            font::text_parts_for(font_id, value, w, h)?
        } else {
            Vec::new()
        };
        return Ok(TextLayout {
            path,
            baseline: font::baseline_for(font_id, h),
            parts,
        });
    };
    if spacing < 0. {
        return Err("overlapping field block lines unsupported".into());
    }
    // ^FB p. 186 specifies no printing below the selected font width.
    // Captured ZD621 previews instead emit individual characters.
    if !compatibility.block_narrow_printer_layout && width < w {
        return Ok(TextLayout {
            path: Path::default(),
            baseline: font::baseline_for(font_id, h),
            parts: Vec::new(),
        });
    }
    let mut lines = Vec::new();
    // Explicit paragraph ends affect centering and terminate justification.
    // Preserve them separately from automatic wraps (^FB pp. 185–187).
    // ^FB p. 187 defines \\ as an escaped backslash. Consume pairs before
    // recognizing \& so a literal backslash followed by '&' stays on its line.
    // Keep other escapes intact here: soft-hyphen markers affect word wrapping.
    let mut paragraphs = vec![String::new()];
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('\\') => {
                    if encoding != 13 && !compatibility.block_backslash_without_ci13 {
                        return Err("field-block backslash requires ^CI13".into());
                    }
                    if encoding != 13 && encoding != 27 {
                        return Err(
                            "field-block backslash glyph unsupported for this encoding".into()
                        );
                    }
                    chars.next();
                    paragraphs.last_mut().unwrap().push('\\');
                    continue;
                }
                Some('&') => {
                    chars.next();
                    paragraphs.push(String::new());
                    continue;
                }
                _ => {}
            }
        }
        paragraphs.last_mut().unwrap().push(c);
    }
    let mut paragraphs = paragraphs.iter().peekable();
    while let Some(paragraph) = paragraphs.next() {
        let mut line = String::new();
        for (word_index, mut word) in paragraph.split_whitespace().enumerate() {
            if compatibility.block_narrow_printer_layout
                && word_index > 0
                && line.is_empty()
                && font::width_for(font_id, " ", w, h)?
                    >= width - if lines.is_empty() { 0. } else { indent }
            {
                lines.push((String::new(), false, false, false));
            }
            let mut splitting = false;
            while !word.is_empty() {
                let limit = width - if lines.is_empty() { 0. } else { indent };
                let word_width = font::width_for(font_id, word, w, h)?;
                splitting |= word_width > limit;
                if !splitting
                    || (!compatibility.block_hyphenation_printer_layout && word_width <= limit)
                {
                    let next = if line.is_empty() {
                        word.to_string()
                    } else {
                        format!("{line} {word}")
                    };
                    if !line.is_empty() && font::width_for(font_id, &next, w, h)? > limit {
                        lines.push((std::mem::take(&mut line), false, false, false));
                        continue;
                    }
                    line = next;
                    break;
                }
                // ^FB p. 187: split an overlong word and continue on the next
                // line. Printer captures reserve soft-hyphen space even for the
                // final remainder, and require strictly less than the budget.
                let hyphen = if compatibility.block_hyphenation_printer_layout {
                    '\u{ad}'
                } else {
                    '-'
                };
                let prefix = if line.is_empty() {
                    String::new()
                } else {
                    format!("{line} ")
                };
                let mut cut = 0;
                for end in word.char_indices().map(|(i, c)| i + c.len_utf8()) {
                    let trial = format!("{prefix}{}{hyphen}", &word[..end]);
                    let advance = font::width_for(font_id, &trial, w, h)?;
                    let fits = if compatibility.block_hyphenation_printer_layout {
                        advance < limit
                    } else {
                        advance <= limit
                    };
                    if !fits {
                        break;
                    }
                    cut = end;
                }
                if cut == 0 {
                    if line.is_empty() {
                        if compatibility.block_narrow_printer_layout {
                            // Captured narrow-block fallback consumes one glyph
                            // even when it exceeds the block width. At an exact
                            // glyph+hyphen fit it also paints the final hyphen.
                            let end = word.chars().next().unwrap().len_utf8();
                            let mut chunk = word[..end].to_string();
                            let with_hyphen = format!("{chunk}{hyphen}");
                            let paint_hyphen =
                                font::width_for(font_id, &with_hyphen, w, h)? <= limit;
                            if paint_hyphen {
                                chunk.push(hyphen);
                            }
                            lines.push((chunk, false, paint_hyphen, true));
                            word = &word[end..];
                            continue;
                        }
                        return Err("field block too narrow for a character and hyphen".into());
                    }
                    lines.push((std::mem::take(&mut line), false, false, false));
                    continue;
                }
                if cut == word.len() {
                    line = format!("{prefix}{word}");
                    break;
                }
                lines.push((
                    format!("{prefix}{}{hyphen}", &word[..cut]),
                    false,
                    true,
                    false,
                ));
                word = &word[cut..];
                line.clear();
            }
        }
        lines.push((line, paragraphs.peek().is_some(), false, false));
    }
    let mut path = Path::default();
    let mut text_parts = Vec::new();
    for (i, (line, hard_break, automatic_hyphen, forced_character)) in lines.iter().enumerate() {
        // CI27 incorrectly reinterprets the automatic soft-hyphen byte as eth.
        // Keep the selected hyphen for measurement and change only painted ink.
        let painted;
        let paint_line =
            if *automatic_hyphen && encoding == 27 && compatibility.block_hyphenation_ci27_uses_eth
            {
                let mut chars = line.chars();
                chars.next_back();
                painted = format!("{}\u{f0}", chars.as_str());
                painted.as_str()
            } else {
                line.as_str()
            };
        // Zebra ^FB pp. 185–187: indent subsequent lines, distribute
        // justification between words, and leave the final line left-aligned.
        let inset = if i == 0 { 0. } else { indent };
        let slack = width - inset - font::width_for(font_id, line, w, h)?;
        let slack = if *forced_character {
            slack.max(0.)
        } else {
            slack
        };
        let gaps = line.bytes().filter(|&c| c == b' ').count();
        let justified = align == b'J'
            && !*hard_break
            && (lines.len() <= max_lines || i + 1 < max_lines)
            && (i + 1 < lines.len()
                || (center_space && slack <= font::width_for(font_id, " ", w, h)?))
            && gaps > 0;
        let mut line_parts = Vec::new();
        let mut p = if justified {
            let mut p = Path::default();
            let mut x: f64 = 0.;
            for (gap, (word, paint_word)) in line.split(' ').zip(paint_line.split(' ')).enumerate()
            {
                let mut word_path = font::text_for(font_id, paint_word, w, h)?;
                // ^FB p. 187 distributes slack between words. The ZD621
                // rounds fractional positions upward; compute cumulative slack
                // directly to avoid rounding an accumulated floating-point error.
                let position = if compatibility.block_justification_rounds_up {
                    (x + slack * gap as f64 / gaps as f64).ceil()
                } else {
                    x.round()
                };
                word_path.transform(|p| Point::new(p.x + position, p.y));
                if compatibility.text_clamps_negative_origins {
                    for mut part in font::text_parts_for(font_id, paint_word, w, h)? {
                        part.transform(|p| Point::new(p.x + position, p.y));
                        line_parts.push(part);
                    }
                }
                p.segments.extend(word_path.segments);
                x += font::width_for(font_id, word, w, h)?
                    + font::width_for(font_id, " ", w, h)?
                    + if compatibility.block_justification_rounds_up {
                        0.
                    } else {
                        slack / gaps as f64
                    };
            }
            p
        } else {
            if compatibility.text_clamps_negative_origins {
                line_parts = font::text_parts_for(font_id, paint_line, w, h)?;
            }
            font::text_for(font_id, paint_line, w, h)?
        };
        let x = inset
            + match align {
                b'C' => {
                    let center = (slack
                        - if center_space
                            && !*hard_break
                            && !*automatic_hyphen
                            && slack > font::width_for(font_id, " ", w, h)?
                        {
                            font::width_for(font_id, " ", w, h)?
                        } else {
                            0.
                        })
                        / 2.;
                    if compatibility.block_center_rounds_down {
                        center.floor()
                    } else {
                        center
                    }
                }
                b'R' => slack,
                _ => 0.,
            };
        // ^FB p. 186: excess text overprints the last row. Union below keeps
        // overlapping glyph ink black. Captured in field-block-overflow-zd621-v1.
        let y = i.min(max_lines - 1) as f64 * (h + spacing);
        p.transform(|p| Point::new(p.x + x, p.y + y));
        for mut part in line_parts {
            part.transform(|p| Point::new(p.x + x, p.y + y));
            text_parts.push(part);
        }
        path.segments.extend(p.segments);
    }
    Ok(TextLayout {
        path: if lines.len() > 1 {
            font::union_lines(path)
        } else {
            path
        },
        baseline: (max_lines - 1) as f64 * (h + spacing) + font::baseline_for(font_id, h),
        parts: text_parts,
    })
}
fn font_dimensions(
    p: &[&str],
    default_w: f64,
    default_h: f64,
    id: char,
) -> Result<(f64, f64), String> {
    let h = number(p, 1, 0.)?;
    let w = number(p, 2, 0.)?;
    if id != '0' {
        // ZPL Programming Guide Table 31, p. 1583: native bitmap matrices.
        let (nh, nw) = match id {
            'A' => (9., 5.),
            'B' => (11., 7.),
            'E' => (28., 15.),
            'F' => (26., 13.),
            'G' => (60., 40.),
            'H' => (21., 13.),
            'S' => (24., 24.),
            _ => (18., 10.),
        };
        // ^A p. 61 and ^CF p. 154: one supplied dimension determines
        // the other from the native matrix. With neither, use the last CF pair.
        let (w, h) = if w == 0. && h == 0. {
            (default_w, default_h)
        } else {
            (w, h)
        };
        let hs = if h == 0. {
            (w / nw).round().max(1.)
        } else {
            (h / nh).round().max(1.)
        };
        let ws = if w == 0. {
            hs
        } else {
            (w / nw).round().max(1.)
        };
        return Ok((nw * ws, nh * hs));
    }
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
