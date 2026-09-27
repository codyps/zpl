//! Local, deterministic ZPL previews. Unsupported rendering semantics are errors.
mod advanced_text;
mod barcode;
mod barcode_edges;
pub mod compatibility;
pub mod profiles;
use raster_diff::compression;
use unicode_normalization::UnicodeNormalization;
mod bounded_text;
mod concatenation;
mod field_block;
mod font;
mod graphics;
mod numbered;
mod printer_shapes;
mod serial;
mod validation;
use crate::{
    bitmap_font::GRAPHIC_SYMBOLS,
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
    center_overflow: Vec<bool>,
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
    standard_field_data: bool,
    hex: Option<u8>,
    barcode: Option<barcode::Barcode>,
    barcode_error: Option<String>,
    path: Option<Path>,
    origins: Option<Vec<Option<(f64, f64)>>>,
    multiple_paths: Option<Vec<(f64, f64, Path)>>,
    block: Option<(f64, usize, f64, u8, f64)>,
    bounded: Option<(f64, f64)>,
    requested_text_height: Option<f64>,
    barcode_split: Vec<barcode_edges::PartBoundary>,
    retail_caption_parts: Vec<barcode::CaptionPart>,
    barcode_width: f64,
    baseline_height: f64,
    inverted_margin: f64,
    leading_tab_advance: f64,
    text_size: Option<(f64, f64)>,
    text_parts: Vec<Path>,
    center_overflow: Vec<bool>,
    graphic_size: Option<(f64, f64)>,
    graphic_bitmap: bool,
    direction: (u8, f64),
    direction_metrics: font::DirectionMetrics,
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
            standard_field_data: false,
            hex: None,
            barcode: None,
            barcode_error: None,
            path: None,
            origins: None,
            multiple_paths: None,
            block: None,
            bounded: None,
            requested_text_height: None,
            barcode_split: Vec::new(),
            retail_caption_parts: Vec::new(),
            barcode_width: 0.,
            baseline_height: 0.,
            inverted_margin: 0.,
            leading_tab_advance: 0.,
            text_size: None,
            text_parts: Vec::new(),
            center_overflow: Vec::new(),
            graphic_size: None,
            graphic_bitmap: false,
            direction: (b'H', 0.),
            direction_metrics: font::DirectionMetrics::default(),
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
    for dimension in [
        options.compatibility.preview_width_quantum,
        options.compatibility.preview_max_width,
    ]
    .into_iter()
    .flatten()
    {
        if dimension == 0 || dimension > 4096 {
            return Err(RenderError {
                offset: 0,
                message: "preview width settings must be in 1..=4096".into(),
            });
        }
    }
    if input.len() > 1_048_576 {
        return Err(RenderError {
            offset: 0,
            message: "input exceeds 1 MiB renderer limit".into(),
        });
    }
    let numbered = numbered::plan(input, options.compatibility)?;
    let mut pending_terminator = None;
    // Dimensions can change after a field. Cull only beyond every declared
    // canvas, so a later PW/LL cannot reveal text discarded at an earlier FS.
    let (mut cull_width, mut cull_height) = (options.width, options.height);
    for item in ParseContext::from_bytes(input).filter_map(Result::ok) {
        let raw = item.as_bytes();
        if matches!(raw.get(1..3), Some(b"PW" | b"LL")) {
            if let Some(n) = std::str::from_utf8(&raw[3..])
                .ok()
                .and_then(|s| s.trim().parse::<f64>().ok())
            {
                let n = n as u32;
                if raw.get(1..3) == Some(b"PW") {
                    cull_width = cull_width.max(n);
                } else {
                    cull_height = cull_height.max(n);
                }
            }
        }
    }
    if let Some(quantum) = options.compatibility.preview_width_quantum {
        cull_width = cull_width.saturating_add(quantum - 1);
    }
    let mut parser = ParseContext::from_bytes(input);
    let mut labels = Vec::new();
    let mut total_segments = 0usize;
    let mut scene = None;
    let mut field = Field::default();
    let (mut width, mut height) = (
        options
            .width
            .min(options.compatibility.preview_max_width.unwrap_or(u32::MAX)),
        options.height,
    );
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
    let mut character_maps: [Option<[u8; 256]>; 14] = [None; 14];
    let mut advanced = [false; 4];
    let mut default_rotation = b'N';
    let mut default_justification = 2;
    let mut reverse = false;
    let mut code_validation = false;
    let mut upside_down = false;
    let mut mirror = false;
    loop {
        let (syntax, offset, mut item, replayed) =
            if let Some((syntax, offset, item)) = pending_terminator.take() {
                (syntax, offset, item, true)
            } else {
                let syntax = parser.syntax();
                let offset = parser.position();
                let Some(item) = parser.next() else { break };
                let item = item.map_err(|e| RenderError {
                    offset,
                    message: e.to_string(),
                })?;
                (syntax, offset, item, false)
            };
        let replacement = if replayed {
            None
        } else {
            numbered.get(&offset)
        };
        if matches!(replacement, Some(numbered::Action::Data(_))) {
            // A reference receives its data at FS, under its own layout state.
            pending_terminator = Some((syntax, offset, item));
            item = Element::FormatCommand(b"^FD");
        }
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
                        | "PM"
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
            if field.path.is_some()
                && !matches!(name, "FS" | "FR" | "FX" | "CC" | "CT" | "CD" | "SF")
            {
                return Err("drawing must end with FS before another command".into());
            }
            let max = match name {
                "XA" | "XZ" | "FS" | "FR" => Some(0),
                "PW" | "LL" | "LS" | "LT" | "LR" | "PO" | "FH" => Some(1),
                "LH" | "FW" | "FP" | "SF" => Some(2),
                "FO" | "FT" | "CF" | "BY" | "XG" | "TB" | "SN" => Some(3),
                "GB" | "GD" | "B3" | "FB" => Some(5),
                "BC" => Some(6),
                "GE" | "PA" => Some(4),
                "GC" => Some(3),
                "CI" => Some(513),
                n if n.starts_with('A') => Some(3),
                _ => None,
            };
            if max.is_some_and(|n| if n == 0 { !s.is_empty() } else { p.len() > n }) {
                return Err("unexpected command parameters".into());
            }

            match name {
                "CC" | "CT" | "CD" | "FX" => {}
                // Metadata was validated and resolved by numbered::plan.
                "FN" | "FE" => {}
                "PA" => {
                    // Zebra Programming Guide ^PA, p. 315 documents zero defaults.
                    // The printer profile preserves omitted operands. The embedded repertoire has no extra
                    // shaping/OpenType substitutions; native pair/ligature and
                    // Hebrew controls pin their unchanged glyphs and advances.
                    // https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
                    for (i, flag) in advanced.iter_mut().enumerate() {
                        *flag = match p.get(i).copied().unwrap_or("") {
                            "" if options.compatibility.advanced_text_omitted_flags_persist => {
                                *flag
                            }
                            "" | "0" => false,
                            "1" => true,
                            _ => return Err("PA flags must be zero or one".into()),
                        };
                    }
                }
                "CI" => {
                    if !matches!(
                        p[0],
                        "0" | "13" | "27" | "28" | "31" | "33" | "34" | "35" | "36"
                    ) {
                        return Err("character encoding unsupported".into());
                    }
                    if p.len() % 2 != 1 {
                        return Err("CI remapping requires source/destination pairs".into());
                    }
                    encoding = p[0].parse::<u8>().unwrap();
                    // ^CI pp. 155–158: source is the output image, destination
                    // is the input character. Only legacy encodings remap.
                    // Independent native controls: character-remap-zd621-v1.
                    for pair in p[1..].as_chunks::<2>().0 {
                        let source = pair[0].parse::<u8>().map_err(|_| "invalid CI source")?;
                        let destination = pair[1]
                            .parse::<u8>()
                            .map_err(|_| "invalid CI destination")?;
                        if matches!(encoding, 0 | 13)
                            && (destination != b' ' || options.compatibility.remap_space)
                        {
                            character_maps[encoding as usize]
                                .get_or_insert_with(|| std::array::from_fn(|i| i as u8))
                                [destination as usize] = source;
                        }
                    }
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
                    let invert =
                        upside_down && !options.compatibility.preview_ignores_print_orientation;
                    let mirrored = mirror && !options.compatibility.preview_ignores_print_mirror;
                    if invert || mirrored {
                        let (w, h) = (sc.width as f64, sc.height as f64);
                        for d in &mut sc.draws {
                            // ^PM p. 319 mirrors the whole printable area; ^PO
                            // p. 322 rotates it. Apply final settings to every field.
                            d.path.transform(|p| {
                                Point::new(
                                    if invert ^ mirrored { w - p.x } else { p.x },
                                    if invert { h - p.y } else { p.y },
                                )
                            });
                        }
                    }
                    if let Some(quantum) = options.compatibility.preview_width_quantum {
                        let logical_width = sc.width;
                        let rounded_width = logical_width.div_ceil(quantum) * quantum;
                        let offset = (rounded_width - logical_width) / 2;
                        Scene::new(rounded_width, sc.height, options.dpi)
                            .map_err(|e| e.to_string())?;
                        sc.width = rounded_width;
                        for draw in &mut sc.draws {
                            draw.path
                                .transform(|p| Point::new(p.x + offset as f64, p.y));
                        }
                        // The firmware clips against the rounded canvas, not
                        // the requested PW: width-boundary-1/65 retain ink in
                        // the added right-hand area. Do not erase that margin.
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
                        width = (n as u32)
                            .min(options.compatibility.preview_max_width.unwrap_or(u32::MAX))
                    } else if !options.compatibility.preview_ignores_label_length {
                        height = n as u32
                    }
                    let fresh =
                        Scene::new(width, height, options.dpi).map_err(|e| e.to_string())?;
                    if let Some(sc) = scene.as_mut() {
                        if !options.compatibility.preview_width_latched_at_first_draw
                            || sc.draws.is_empty()
                        {
                            sc.width = fresh.width;
                        }
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
                "PM" => {
                    // ^PM p. 319: missing/invalid values are ignored and the
                    // setting persists across labels until an explicit PMN.
                    match s {
                        "Y" => mirror = true,
                        "N" => mirror = false,
                        _ => {}
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
                    default_justification = justification(&p, 1, 2)?;
                    field.justification = default_justification;
                }
                "FP" => {
                    // Zebra Programming Guide ^FP p. 202: field-local direction
                    // and additional character spacing, independent of rotation.
                    let direction = match p[0] {
                        "" | "H" => b'H',
                        "V" => b'V',
                        "R" => b'R',
                        _ => return Err("invalid field direction".into()),
                    };
                    let gap = number(&p, 1, 0.)?;
                    if !(0. ..=9999.).contains(&gap) || gap.fract() != 0. {
                        return Err("invalid field character gap".into());
                    }
                    field.direction = (direction, gap);
                }
                "TB" => {
                    let width = number(&p, 1, 1.)?;
                    let height = number(&p, 2, 1.)?;
                    if width < 1. || height < 1. || width.fract() != 0. || height.fract() != 0. {
                        return Err("invalid bounded text dimensions".into());
                    }
                    if !p[0].is_empty() {
                        field.rotation = rotation(p[0])?;
                    }
                    field.bounded = Some((width, height));
                    field.block = None;
                }
                "FB" => {
                    field.bounded = None;
                    // ^FB selects text layout, replacing a preceding ^GS symbol field.
                    // ZD621 controls in graphic-symbols-zd621-v1 retain GS dimensions.
                    if font_id == GRAPHIC_SYMBOLS {
                        font_id = default_font_id;
                    }
                    let width = number(&p, 0, 0.)?;
                    let lines = number(&p, 1, 1.)?;
                    let spacing = number(&p, 2, 0.)?;
                    let indent = number(&p, 4, 0.)?;
                    let align = p.get(3).copied().unwrap_or("L");
                    if width < 0.
                        || !(1. ..=1000.).contains(&lines)
                        || lines.fract() != 0.
                        || !matches!(align, "" | "L" | "C" | "R" | "J")
                        || !(0. ..=9999.).contains(&indent)
                    {
                        return Err("unsupported or invalid field block parameters".into());
                    }
                    field.block = Some((
                        width,
                        lines as usize,
                        spacing,
                        align.as_bytes().first().copied().unwrap_or(b'L'),
                        indent,
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
                        "P" => 'P',
                        "Q" => 'Q',
                        "R" => 'R',
                        "S" => 'S',
                        "T" => 'T',
                        "U" => 'U',
                        "V" => 'V',
                        value
                            if matches!(
                                value.as_bytes(),
                                [b'1'..=b'9' | b'I'..=b'O' | b'W'..=b'Z']
                            ) && options.compatibility.unavailable_fonts_use_default =>
                        {
                            'A'
                        }
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
                    (font_w, font_h) = font_dimensions(
                        &p,
                        dw,
                        dh,
                        font_id,
                        options.compatibility.font0_minimum_dimensions,
                        options.compatibility.bitmap_font_maximum_dimensions,
                    )?;
                    field.requested_text_height = Some(number(&p, 1, dh)?);
                    default_w = font_w;
                    default_h = font_h;
                }
                n if n.starts_with('A') => {
                    if options.compatibility.bounded_text_font_cancels_block {
                        field.bounded = None;
                    }
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
                        "AP" => 'P',
                        "AQ" => 'Q',
                        "AR" => 'R',
                        "AS" => 'S',
                        "AT" => 'T',
                        "AU" => 'U',
                        "AV" => 'V',
                        value
                            if matches!(
                                value.as_bytes(),
                                [b'A', b'1'..=b'9' | b'I'..=b'O' | b'W'..=b'Z']
                            ) && options.compatibility.unavailable_fonts_use_default =>
                        {
                            default_font_id
                        }
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
                    (font_w, font_h) = font_dimensions(
                        &p,
                        dw,
                        dh,
                        font_id,
                        options.compatibility.font0_minimum_dimensions,
                        options.compatibility.bitmap_font_maximum_dimensions,
                    )?;
                    field.requested_text_height = Some(number(&p, 1, dh)?);
                }
                "GS" => {
                    // ^GS p. 217 selects a separate symbol face, not ^AS.
                    font_id = GRAPHIC_SYMBOLS;
                    field.barcode = None;
                    field.explicit_font = false;
                    field.rotation = if p[0].is_empty() {
                        default_rotation
                    } else {
                        rotation(p[0])?
                    };
                    (font_w, font_h) = font_dimensions(
                        &p,
                        default_requested_w,
                        default_requested_h,
                        font_id,
                        options.compatibility.font0_minimum_dimensions,
                        options.compatibility.bitmap_font_maximum_dimensions,
                    )?;
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
                    )
                    .and_then(|barcode| {
                        let next_module = barcode.module_after_command()?;
                        Ok((barcode, next_module))
                    });
                    match barcode {
                        Ok((barcode, next_module)) => {
                            module = next_module;
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
                "SF" => {
                    if !field.standard_field_data {
                        return Err("SF requires a preceding FD or FV in the same field".into());
                    }
                    serial::validate_mask(&p)?;
                }
                "FD" | "FV" | "SN" => {
                    field.standard_field_data = matches!(name, "FD" | "FV");
                    if matches!(replacement, Some(numbered::Action::Skip)) {
                        return Ok(());
                    }
                    if field.path.is_some() {
                        return Err("multiple drawing commands in one field".into());
                    }
                    let serial_data;
                    let data = if name == "SN" {
                        serial_data = serial::initial_value(&p)?;
                        serial_data.as_slice()
                    } else {
                        data
                    };
                    let mut bytes = match replacement {
                        Some(numbered::Action::Data(data) | numbered::Action::Value(data)) => {
                            data.as_ref().clone()
                        }
                        _ => Vec::new(),
                    };
                    let mut i = if matches!(replacement, Some(numbered::Action::Value(_))) {
                        data.len()
                    } else {
                        0
                    };
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
                    if name == "SN" {
                        serial::suppress_zeros(
                            &mut bytes,
                            &p,
                            options.compatibility.serial_overlong_keeps_value,
                        )?;
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
                    // Native ^FH NUL ends plain/TB text, while FB removes it.
                    // ^FD/^FH pp. 190/193 do not specify C-string truncation;
                    // printer-only evidence: nul-text-zd621-v1.
                    if field.barcode.is_none() && options.compatibility.text_nul_processing {
                        if field.block.is_some() {
                            bytes.retain(|&byte| byte != 0);
                        } else if let Some(end) = bytes.iter().position(|&byte| byte == 0) {
                            bytes.truncate(end);
                        }
                    }
                    // ^FD/^FH (guide pp. 190/193) do not define these layout-
                    // dependent controls; text-controls-zd621-v1 pins native output.
                    if field.barcode.is_none() && options.compatibility.text_control_processing {
                        if field.bounded.is_none() {
                            if let Some(end) = bytes.iter().position(|&b| b == b'\r' || b == b'\n')
                            {
                                bytes.truncate(end);
                            }
                        }
                        if field.block.is_some() {
                            for byte in &mut bytes {
                                if *byte == 1 {
                                    *byte = b' ';
                                }
                            }
                        } else {
                            bytes.retain(|&b| b != 1);
                        }
                    }
                    if field.barcode.is_none()
                        && field.block.is_none()
                        && options.compatibility.text_esc_del_processing
                        && !matches!(encoding, 0 | 13)
                    {
                        // CI33–36 preserve ESC as blank spacing; CI27/28/31
                        // omit it. DEL is omitted in modern plain/TB text.
                        bytes.retain(|b| *b != 0x7f && (*b != 0x1b || matches!(encoding, 33..=36)));
                    }
                    let decoded;
                    let value = if field.barcode.is_some() || field.barcode_error.is_some() {
                        ""
                    } else if let Some(code_page) = match encoding {
                        // Zebra Programming Guide ^CI, pp. 156–159:
                        // https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
                        27 => Some(encoding_rs::WINDOWS_1252),
                        31 => Some(encoding_rs::WINDOWS_1250),
                        33 => Some(encoding_rs::WINDOWS_1251),
                        34 => Some(encoding_rs::WINDOWS_1253),
                        35 => Some(encoding_rs::WINDOWS_1254),
                        36 => Some(encoding_rs::WINDOWS_1255),
                        _ => None,
                    } {
                        decoded = code_page
                            .decode_without_bom_handling_and_without_replacement(&bytes)
                            .ok_or("undefined code page byte")?;
                        &decoded
                    } else {
                        if matches!(encoding, 0 | 13) && !bytes.is_ascii() {
                            return Err(
                                "unsupported legacy text byte; select ^CI28 for UTF-8".into()
                            );
                        }
                        std::str::from_utf8(&bytes).map_err(|_| "invalid UTF-8 text")?
                    };
                    // UAX #15 sections 1.1–1.2: canonically equivalent Unicode
                    // text has the same appearance. Native decomposed-accent
                    // controls in unicode-conformance-zd621-v1 verify NFC.
                    // https://www.unicode.org/reports/tr15/
                    let canonical;
                    let value = if encoding == 28 {
                        let block_formatting = field.block.is_some()
                            && options.compatibility.block_utf8_formatting_visible;
                        canonical = value
                            .nfc()
                            .filter_map(|c| match c {
                                '\u{ad}' | '\u{200b}' if !block_formatting => None,
                                '\u{200b}' => Some(' '),
                                _ => Some(c),
                            })
                            .collect::<String>();
                        canonical.as_str()
                    } else {
                        value
                    };
                    let visual;
                    let value = if advanced[1]
                        && !value.is_empty()
                        && field.block.is_none()
                        && field.bounded.is_none()
                    {
                        visual = advanced_text::reorder(
                            value,
                            options.compatibility.bidi_skips_paired_bracket_resolution,
                            options.compatibility.bidi_isolates_as_missing_glyphs,
                        );
                        &visual
                    } else {
                        value
                    };
                    let rendered = (|| -> Result<Path, String> {
                        if let Some(error) = &field.barcode_error {
                            return Err(error.clone());
                        }
                        let path = if let Some(b) = &field.barcode {
                            if code_validation {
                                b.validate_retail_data(&bytes)?;
                            }
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
                                character_maps.get(encoding as usize).copied().flatten(),
                            )?;
                            let mut path = rendered.path;
                            if !field.baseline {
                                let offset = b.field_origin_y();
                                path.transform(|p| Point::new(p.x, p.y + offset));
                            }
                            field.barcode_split = rendered.split;
                            field.retail_caption_parts = rendered.caption_parts;
                            field.barcode_width = rendered.width;
                            field.baseline_height = rendered.baseline;
                            path
                        } else {
                            if font_w <= 0. || font_h <= 0. {
                                return Err("font dimensions must be positive".into());
                            }
                            let text_font = font::Font::new(
                                font_id,
                                encoding == 0
                                    || (encoding == 28
                                        && options.compatibility.utf8_uses_legacy_backslash),
                            )
                            .with_control_glyphs(
                                options.compatibility.text_esc_del_processing
                                    && matches!(encoding, 0 | 13),
                                options.compatibility.text_esc_del_processing
                                    && (field.block.is_some() || matches!(encoding, 33..=36))
                                    && !matches!(encoding, 0 | 13),
                            )
                            .with_tab_stops(options.compatibility.text_tab_stops)
                            .with_default_glyph(advanced[0])
                            .with_character_map(
                                character_maps.get(encoding as usize).copied().flatten(),
                            );
                            // ^FB p. 186 permits negative line spacing. The
                            // printer clamps the resulting pitch at zero.
                            let block =
                                field.block.map(|(width, lines, spacing, align, indent)| {
                                    let spacing = if options
                                        .compatibility
                                        .block_negative_pitch_clamps_to_zero
                                    {
                                        spacing.max(
                                            -font::block_metrics(
                                                text_font,
                                                font_h,
                                                options.compatibility.font_s_block_metrics,
                                            )
                                            .0,
                                        )
                                    } else {
                                        spacing
                                    };
                                    (width, lines, spacing, align, indent)
                                });
                            let TextLayout {
                                path,
                                baseline,
                                parts,
                                center_overflow,
                            } = if let Some(bounds) = field.bounded {
                                if field.direction != (b'H', 0.) {
                                    return Err("field direction with TB is unsupported".into());
                                }
                                // ^TB defaults to script-dependent justification (guide p. 356).
                                if field.justification == 2 {
                                    field.justification = u8::from(
                                        advanced_text::base_level(value)
                                            .is_some_and(|level| level.is_rtl()),
                                    );
                                }
                                bounded_text::layout(
                                    text_font,
                                    value,
                                    (font_w, font_h),
                                    field
                                        .requested_text_height
                                        .or(Some(default_requested_h))
                                        .filter(|h| *h > 0.)
                                        .unwrap_or(font_h),
                                    if options.compatibility.bounded_text_printer_anchors
                                        && font_id == '0'
                                        && matches!(field.rotation, b'R' | b'I')
                                    {
                                        (bounds.0, (bounds.1 - 1.).max(0.))
                                    } else {
                                        bounds
                                    },
                                    bounded_text::LayoutOptions {
                                        control_processing: options
                                            .compatibility
                                            .text_control_processing,
                                        right: field.justification == 1,
                                        printer_pitch: options
                                            .compatibility
                                            .bounded_text_printer_pitch,
                                        bidi: advanced[1].then_some(options.compatibility),
                                    },
                                )?
                            } else {
                                text_block(
                                    if block.is_some() {
                                        text_font
                                            .with_block_flow(field.direction, options.compatibility)
                                    } else {
                                        text_font
                                    },
                                    value,
                                    font_w,
                                    font_h,
                                    block,
                                    options.compatibility,
                                    encoding,
                                )?
                            };
                            let (path, parts, direction_size) =
                                if field.direction != (b'H', 0.) && block.is_none() {
                                    let layout = font::directed_text(
                                        text_font,
                                        value,
                                        font_w,
                                        font_h,
                                        field.direction,
                                        options.compatibility,
                                        field.justification == 1,
                                    )?;
                                    field.direction_metrics = layout.metrics;
                                    (layout.path, layout.parts, Some(layout.size))
                                } else {
                                    (path, parts, None)
                                };
                            if options.compatibility.text_tab_stops && field.block.is_none() {
                                let leading: String = value
                                    .chars()
                                    .take_while(|c| c.is_ascii_whitespace())
                                    .collect();
                                if leading.contains('\t') {
                                    // FO/R/right retains tab indentation; independent
                                    // paired font controls are in tabs-zd621-v1.
                                    field.leading_tab_advance =
                                        font::width_for(text_font, &leading, font_w, font_h)?;
                                }
                            }
                            field.text_parts = parts;
                            field.center_overflow = center_overflow;
                            // Table 29 p. 1582 gives GS a 3/4-height baseline.
                            // Printer controls instead use native row 23 of 24.
                            field.baseline_height = if font_id == GRAPHIC_SYMBOLS
                                && !options.compatibility.graphic_symbol_last_row_baseline
                            {
                                baseline - font_h * 5. / 24.
                            } else {
                                baseline
                            };
                            if field.bounded.is_none()
                                && field.block.is_none_or(|block| block.0 != 0.)
                                && options
                                    .compatibility
                                    .right_justified_inverted_text_uses_ink_margin
                            {
                                field.inverted_margin = if font_id == '0' && field.block.is_none() {
                                    font::inverted_text_margin(
                                        text_font,
                                        value,
                                        font_w,
                                        font_h,
                                        field.direction.1,
                                    )?
                                } else {
                                    font::inverted_margin(text_font, value, font_w, font_h)?
                                };
                            }
                            field.text_size = Some(if let Some(size) = field.bounded {
                                size
                            } else if let Some((width, lines, spacing, _, _)) = block {
                                let (pitch, delta) = font::block_metrics(
                                    text_font,
                                    font_h,
                                    options.compatibility.font_s_block_metrics,
                                );
                                (
                                    width,
                                    (lines - 1) as f64 * (pitch + spacing) + font_h + delta,
                                )
                            } else {
                                (font::width_for(text_font, value, font_w, font_h)?, font_h)
                            });
                            if let Some(size) = direction_size {
                                field.text_size = Some(size);
                            }
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
                                    && error.starts_with("retail")
                                    && field
                                        .barcode
                                        .as_ref()
                                        .is_some_and(|b| b.uses_ean_validation()))
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
                        let field_justification = if font_id == GRAPHIC_SYMBOLS
                            && field.text_size.is_some()
                            && options.compatibility.graphic_symbol_ignores_justification
                        {
                            0
                        } else {
                            field.justification
                        };
                        let transform = |p: Point| {
                            // ^FO p. 201 does not prescribe fractional baseline
                            // rounding. Native FO/FT pairs at heights 10..25
                            // place the normal horizontal baseline at floor(3h/4).
                            let p = if options.compatibility.font0_fo_floor_baseline
                                && font_id == '0'
                                && field.text_size.is_some()
                                && !field.baseline
                                && field.bounded.is_none()
                                && field.direction.0 == b'H'
                            {
                                Point::new(p.x, p.y - font::baseline_for(font_id, font_h).fract())
                            } else {
                                p
                            };
                            if let Some(bounds) = field.bounded {
                                let p = bounded_text::position(
                                    p,
                                    bounds,
                                    field.rotation,
                                    field.baseline,
                                    field_justification == 1,
                                    font_id == '0',
                                    options.compatibility.bounded_text_printer_anchors,
                                );
                                return Point::new(p.x + x, p.y + y);
                            }
                            // ^FO/^FT pp. 201/205: right justification changes
                            // the origin, not the character order. Auto (2) is
                            // left for the supported Latin scripts.
                            let mut advance =
                                field.text_size.or(field.graphic_size).map_or(w, |s| s.0);
                            if options.compatibility.field_direction_printer_anchors
                                && field.text_size.is_some()
                                && field.block.is_none()
                                && field.direction.0 == b'R'
                                && matches!(field.rotation, b'I' | b'B')
                            {
                                advance += field.direction_metrics.end_margin
                                    + field.direction_metrics.first_delta
                                    + if font_id == '0' { 1. } else { 0. };
                            }
                            if options.compatibility.field_direction_printer_anchors
                                && field.text_size.is_some()
                                && field.block.is_none()
                                && field.direction.0 == b'R'
                                && matches!(field.rotation, b'N' | b'R')
                            {
                                advance += field.direction_metrics.end_left;
                            }
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
                                        if font_id == '0'
                                            || (matches!(
                                                font_id,
                                                'P' | 'Q' | 'R' | 'S' | 'T' | 'U' | 'V'
                                            ) && options.compatibility.preset_font_fo_last_dot)
                                        {
                                            // A zero-width printer block still
                                            // pivots at width minus one dot.
                                            let w = if w == 0. && field.block.is_some() {
                                                -1.
                                            } else {
                                                (w - 1.).max(0.)
                                            };
                                            // P/Q quantize the height pivot in native cells;
                                            // R retains the matrix boundary in native controls.
                                            // its proportional horizontal advance loses one dot.
                                            let last_row = match font_id {
                                                'P' => font_h / 20.,
                                                'Q' => font_h / 28.,
                                                'R' => 0.,
                                                'S' => 2. * font_h / 40.,
                                                // Native T/U/V atlas: resident-tuv-zd621-v1.
                                                'T' => 3. * font_h / 48.,
                                                'U' => font_h / 59.,
                                                'V' => 2. * font_h / 80.,
                                                _ => 1.,
                                            };
                                            (w, (h - last_row).max(0.))
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
                                let w = field.direction_metrics.pivot.unwrap_or(w);
                                match field.rotation {
                                    b'R' => (h, 0.),
                                    b'I' => (w, h),
                                    b'B' => (0., w),
                                    _ => (0., 0.),
                                }
                            };
                            let (mut jx, mut jy) = if !field.baseline && field_justification == 1 {
                                if let Some((tw, th)) = field.text_size {
                                    match field.rotation {
                                        b'R' if field.block.is_some()
                                            && options
                                                .compatibility
                                                .block_fo_right_justification_printer_layout =>
                                        {
                                            (-th, 0.)
                                        }
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
                                        b'R' => (-th, -left + field.leading_tab_advance),
                                        b'I' => (field.inverted_margin - dx + if font_id == '0'
                                            && field.block.is_none()
                                            && options.compatibility.right_justified_inverted_text_uses_ink_margin
                                            { 0. } else { left }, 0.),
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
                            if options.compatibility.field_direction_printer_anchors
                                && field.text_size.is_some()
                                && !field.baseline
                                && field_justification == 1
                                && field.block.is_none()
                            {
                                let dot = if font_id == '0' { 0. } else { 1. };
                                match (field.direction.0, field.rotation) {
                                    (b'H', b'I') => {
                                        jx += field.direction.1;
                                    }
                                    (b'R', b'R') => jy = 0.,
                                    (b'R', b'N') => jx -= field.direction_metrics.end_left,
                                    (b'R', b'I') => jx = dot - dx,
                                    (b'R', b'B') => jy = advance - dy + dot,
                                    (b'V', b'I') => {
                                        jx = if field.direction_metrics.count == 1 {
                                            field.direction_metrics.end_left
                                                + field.direction_metrics.end_margin
                                                + dot
                                                - dx
                                        } else {
                                            -field.direction_metrics.vertical_extent.0
                                                - field.direction_metrics.vertical_extent.1
                                                + 2. * dot
                                                - dx
                                        };
                                    }
                                    // The right anchor uses the capital row; B also
                                    // restores the leading glyph's crop and descent.
                                    // See descender/ascender controls in the FP fixture.
                                    (b'V', b'R') => {
                                        jx = field.direction_metrics.first_ink.1
                                            - field.direction_metrics.leading_descent
                                            - font_h
                                            - dx
                                    }
                                    (b'V', b'B') => {
                                        jx += field.direction_metrics.bottom_margin
                                            + 2. * field.direction_metrics.leading_descent
                                            + field.direction_metrics.leading_top;
                                        jy = advance - dy + dot;
                                    }
                                    _ => {}
                                }
                            }
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
                        if !field.text_parts.is_empty()
                            && (options.compatibility.text_clamps_negative_origins
                                || field.center_overflow.iter().any(|&v| v))
                        {
                            let shifts: Vec<_> = field
                                .text_parts
                                .iter()
                                .enumerate()
                                .map(|(index, part)| {
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
                                    let (mut dx, mut dy) =
                                        if options.compatibility.text_clamps_negative_origins {
                                            ((-min_x).max(0.), (-min_y).max(0.))
                                        } else {
                                            (0., 0.)
                                        };
                                    // ZD621 centered overflowing FB lines clamp each
                                    // glyph to the absolute inline-axis origin, including
                                    // inverted directions (field-block-limits fixtures).
                                    let mut skip = false;
                                    if field.center_overflow.get(index).copied().unwrap_or(false) {
                                        skip = if matches!(field.rotation, b'R' | b'B') {
                                            min_y < 0.
                                        } else {
                                            min_x < 0.
                                        };
                                        if matches!(field.rotation, b'R' | b'B') {
                                            dy = -min_y;
                                        } else {
                                            dx = -min_x;
                                        }
                                    }
                                    (dx, dy, skip)
                                })
                                .collect();
                            if shifts
                                .iter()
                                .any(|&(dx, dy, skip)| dx != 0. || dy != 0. || skip)
                            {
                                let mut shifted = Path::default();
                                for (part, (dx, dy, skip)) in field.text_parts.iter().zip(shifts) {
                                    if skip {
                                        continue;
                                    }
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
                        // Native retail captions clamp along their reading axis:
                        // N/R use the caption origin, I/B use its visible edge.
                        // Independent sizes/orientations: retail-caption-edges-zd621-v1.
                        let caption_shifts: Vec<_> = if options
                            .compatibility
                            .retail_caption_clamps_negative_inline_origin
                        {
                            field
                                .retail_caption_parts
                                .iter()
                                .map(|part| {
                                    let origin = transform(part.origin);
                                    let mut edge = if field.rotation == b'R' {
                                        origin.y
                                    } else {
                                        origin.x
                                    };
                                    if matches!(field.rotation, b'I' | b'B') {
                                        edge = path.segments[part.start..part.end]
                                            .iter()
                                            .filter_map(|s| match s {
                                                crate::output::Segment::Move(p)
                                                | crate::output::Segment::Line(p) => {
                                                    let p = transform(*p);
                                                    Some(if field.rotation == b'B' {
                                                        p.y
                                                    } else {
                                                        p.x
                                                    })
                                                }
                                                _ => None,
                                            })
                                            .fold(f64::INFINITY, f64::min);
                                    }
                                    (-edge).max(0.)
                                })
                                .collect()
                        } else {
                            Vec::new()
                        };
                        let mut moved_caption = false;
                        for (part, delta) in field.retail_caption_parts.iter().zip(caption_shifts) {
                            if delta == 0. {
                                continue;
                            }
                            moved_caption = true;
                            for segment in &mut path.segments[part.start..part.end] {
                                if let crate::output::Segment::Move(p)
                                | crate::output::Segment::Line(p) = segment
                                {
                                    p.x += if matches!(field.rotation, b'I' | b'B') {
                                        -delta
                                    } else {
                                        delta
                                    };
                                }
                            }
                        }
                        if moved_caption && !(field.reverse || reverse) {
                            path = font::union_lines(path);
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
                        if field.text_size.is_some() {
                            font::cull_outside(&mut path, cull_width, cull_height);
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
    font_id: font::Font,
    value: &str,
    w: f64,
    h: f64,
    block: Option<(f64, usize, f64, u8, f64)>,
    compatibility: compatibility::Compatibility,
    encoding: u8,
) -> Result<TextLayout, String> {
    let center_space = compatibility.block_center_includes_trailing_space;
    let collect_parts = compatibility.text_clamps_negative_origins
        || compatibility.block_center_overflow_clamps_to_origin;
    let Some((width, max_lines, spacing, align, indent)) = block else {
        let path = font::text_for(font_id, value, w, h)?;
        let parts = if collect_parts {
            font::text_parts_for(font_id, value, w, h)?
        } else {
            Vec::new()
        };
        return Ok(TextLayout {
            path,
            baseline: font::baseline_for(font_id, h),
            center_overflow: vec![false; parts.len()],
            parts,
        });
    };
    // ^FB p. 186 specifies no printing below the selected font width.
    // Captured ZD621 previews instead emit individual characters.
    if !compatibility.block_narrow_printer_layout && width < w {
        return Ok(TextLayout {
            path: Path::default(),
            baseline: font::baseline_for(font_id, h),
            parts: Vec::new(),
            center_overflow: Vec::new(),
        });
    }
    let (pitch, baseline_delta) =
        font::block_metrics(font_id, h, compatibility.font_s_block_metrics);
    let lines = field_block::wrap(
        font_id,
        value,
        (w, h),
        (width, indent),
        compatibility,
        encoding,
    )?;
    let mut path = Path::default();
    let mut text_parts = Vec::new();
    let mut center_overflow = Vec::new();
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
        let inset = if i == 0 || (compatibility.block_hard_break_resets_indent && lines[i - 1].1) {
            0.
        } else {
            indent
        };
        let slack = width - inset - font::width_for(font_id, line, w, h)?;
        let slack = if *forced_character {
            slack.max(0.)
        } else {
            slack
        };
        let gaps = line.bytes().filter(|&c| c == b' ').count();
        // The native trailing-space width sweep extends final-line
        // eligibility to two spaces when an extra separator is retained;
        // see field-block-spaces-zd621-v1 (55 terminal-line controls).
        let justified = align == b'J'
            && !*hard_break
            && (lines.len() <= max_lines || i + 1 < max_lines)
            && (i + 1 < lines.len()
                || (center_space
                    && slack >= 0.
                    && slack
                        <= font::width_for(font_id, " ", w, h)?
                            * if compatibility.block_preserves_extra_spaces && line.ends_with(' ') {
                                2.
                            } else {
                                1.
                            }))
            && gaps > 0;
        let mut line_parts = Vec::new();
        let mut p = if justified {
            let mut p = Path::default();
            let mut x: f64 = 0.;
            for (gap, (word, paint_word)) in line.split(' ').zip(paint_line.split(' ')).enumerate()
            {
                let mut word_path = font::text_for(font_id, paint_word, w, h)?;
                // ^FB p. 187 distributes slack between words. The ZD621
                // distributes whole extra dots to the earliest gaps. Four or
                // more gaps distinguish this from ceiling cumulative positions;
                // see the common-font paragraph/remainder native controls.
                let position = if font_id.block_reverses() {
                    // Native ^FP R/^FB J distributes the integer quotient
                    // backwards, but the remainder forwards, one dot in
                    // each of the first remaining word gaps.
                    // The raw width sweep in field-block-direction-zd621-v1
                    // distinguishes this from rounding the signed position.
                    let quotient = (slack / gaps as f64).trunc();
                    let remainder = slack - quotient * gaps as f64;
                    x + quotient * gap as f64 - (gap as f64).min(remainder)
                } else if font_id.block_overprints() {
                    let extra = slack * gap as f64 / gaps as f64;
                    if compatibility.block_justification_rounds_up {
                        extra.ceil()
                    } else {
                        extra.round()
                    }
                } else if compatibility.block_justification_rounds_up {
                    if slack >= 0. {
                        let quotient = (slack / gaps as f64).floor();
                        let remainder = slack - quotient * gaps as f64;
                        (x + quotient * gap as f64 + (gap as f64).min(remainder)).ceil()
                    } else {
                        (x + slack * gap as f64 / gaps as f64).ceil()
                    }
                } else {
                    x.round()
                };
                let position = font_id.block_position(position);
                word_path.transform(|p| Point::new(p.x + position, p.y));
                if collect_parts {
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
            if collect_parts {
                line_parts = font::text_parts_for(font_id, paint_line, w, h)?;
            }
            font::text_for(font_id, paint_line, w, h)?
        };
        let x = inset
            + font_id.block_position(match align {
                b'C' => {
                    let center = (slack
                        - if center_space
                            && !*hard_break
                            && !*automatic_hyphen
                            && (slack > font::width_for(font_id, " ", w, h)?
                                || (slack < 0.
                                    && compatibility.block_center_overflow_clamps_to_origin))
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
            });
        // ^FB p. 186: excess text overprints the last row. Union below keeps
        // overlapping glyph ink black. Captured in field-block-overflow-zd621-v1.
        let y = i.min(max_lines - 1) as f64 * (pitch + spacing) + baseline_delta;
        p.transform(|p| Point::new(p.x + x, p.y + y));
        for mut part in line_parts {
            part.transform(|p| Point::new(p.x + x, p.y + y));
            text_parts.push(part);
            center_overflow.push(
                align == b'C' && slack < 0. && compatibility.block_center_overflow_clamps_to_origin,
            );
        }
        path.segments.extend(p.segments);
    }
    Ok(TextLayout {
        path: if lines.len() > 1 {
            font::union_lines(path)
        } else {
            path
        },
        baseline: (max_lines - 1) as f64 * (pitch + spacing)
            + font::baseline_for(font_id, h)
            + baseline_delta,
        parts: text_parts,
        center_overflow,
    })
}
fn font_dimensions(
    p: &[&str],
    default_w: f64,
    default_h: f64,
    id: char,
    clamp_minimum: bool,
    clamp_maximum: bool,
) -> Result<(f64, f64), String> {
    let h = number(p, 1, 0.)?;
    let w = number(p, 2, 0.)?;
    if id != '0' {
        // ZPL Programming Guide Table 31, pp. 1583–1584: native bitmap matrices.
        let (nh, nw) = match id {
            'A' => (9., 5.),
            'B' => (11., 7.),
            'E' => (28., 15.),
            'F' => (26., 13.),
            'G' => (60., 40.),
            'H' => (21., 13.),
            'P' => (20., 18.),
            'Q' => (28., 24.),
            'R' => (35., 31.),
            'S' => (40., 35.),
            'T' => (48., 42.),
            'U' => (59., 53.),
            'V' => (80., 71.),
            GRAPHIC_SYMBOLS => (24., 24.),
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
        // ^A p. 60 limits resident bitmap matrices to tenfold enlargement.
        // bitmap-maximum-zd621-v1 verifies independent native axis clamping.
        if (matches!(id, 'A'..='H') || (id == GRAPHIC_SYMBOLS && clamp_maximum))
            && (ws > 10. || hs > 10.)
        {
            if !clamp_maximum {
                return Err(
                    "bitmap font dimensions must not exceed ten times the native matrix".into(),
                );
            }
            return Ok((nw * ws.min(10.), nh * hs.min(10.)));
        }
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
    // ^A p. 60 specifies 10 dots as the scalable minimum. Native controls
    // map requests 1..9 to 10 independently in each dimension, after zero
    // inference. Keep the documented range strict without the printer option.
    if clamp_minimum {
        return Ok((w.max(10.), h.max(10.)));
    }
    if w < 10. || h < 10. {
        return Err("scalable font dimensions must be at least 10 dots".into());
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
