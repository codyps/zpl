//! Resident font resources: capture selection, encoding maps and measured metrics.
//! The renderer consumes glyphs and scales without knowing their storage format.
//! ZPL Programming Guide ^CI pp.156–159 and font tables pp.1582–1584:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use crate::bitmap_font::GRAPHIC_SYMBOLS;
/// Resident face plus the legacy ASCII backslash replacement. Keep this
/// separate from U+00A2: bitmap faces have distinct native cent designs.
#[derive(Clone, Copy)]
pub(crate) struct Selection {
    pub(crate) id: char,
    pub(crate) legacy_backslash: bool,
    legacy_codepage: bool,
    serial_zero_source: bool,
    custom: bool,
    encoding: u8,
    default_glyph: bool,
    character_map: Option<[u8; 256]>,
    explicit_sources: Option<[u8; 32]>,
    tab_stops: bool,
    pub(crate) legacy_controls: bool,
    control_spaces: bool,
}
/// Lookup keys retain their namespace across Unicode-aware layout.
/// Printer source slots are not Unicode private-use characters.
#[derive(Clone, Copy, Debug)]
pub(crate) enum GlyphKey {
    Unicode(char),
    Source(u8),
    Input(u8),
}
impl From<char> for GlyphKey {
    fn from(c: char) -> Self {
        Self::Unicode(c)
    }
}
impl Selection {
    pub(crate) fn new(id: char, legacy_backslash: bool) -> Self {
        Self {
            id,
            legacy_backslash,
            legacy_codepage: false,
            serial_zero_source: false,
            custom: false,
            encoding: 28,
            default_glyph: false,
            character_map: None,
            explicit_sources: None,
            tab_stops: false,
            legacy_controls: false,
            control_spaces: false,
        }
    }
    pub(crate) fn with_serial_zero_source(mut self, enabled: bool) -> Self {
        self.serial_zero_source = enabled;
        self
    }
    pub(crate) fn with_custom(mut self, enabled: bool) -> Self {
        self.custom = enabled;
        self
    }
    pub(crate) fn with_encoding(mut self, encoding: u8) -> Self {
        self.encoding = encoding;
        self
    }
    pub(crate) fn with_legacy_codepage(mut self, enabled: bool) -> Self {
        self.legacy_codepage = enabled;
        self
    }
    pub(crate) fn with_tab_stops(mut self, enabled: bool) -> Self {
        self.tab_stops = enabled;
        self
    }
    pub(crate) fn is_tab(self, c: char) -> bool {
        self.tab_stops && c == '\t'
    }
    pub(crate) fn with_character_map(mut self, map: Option<[u8; 256]>) -> Self {
        self.character_map = map;
        self
    }
    pub(crate) fn with_explicit_sources(mut self, mask: Option<[u8; 32]>) -> Self {
        self.explicit_sources = mask;
        self
    }
    pub(crate) fn with_control_glyphs(mut self, legacy: bool, spaces: bool) -> Self {
        self.legacy_controls = legacy;
        self.control_spaces = spaces;
        self
    }
    pub(crate) fn map_char(self, c: char) -> Result<GlyphKey, String> {
        // Text is already Unicode before layout. Recover the input image index
        // only for ^CI remapping; do not decode layout-generated soft hyphens
        // (U+00AD) as CP850 byte AD (inverted exclamation mark).
        let image = if self.legacy_codepage && !c.is_ascii() {
            CP850.iter().position(|&glyph| glyph == c).map(|i| i + 128)
        } else {
            Some(c as usize)
        };
        if !self.custom && resident(self.id).is_some() {
            if let Some(source) = self
                .character_map
                .and_then(|map| image.and_then(|i| map.get(i).copied()))
            {
                let explicit = self.explicit_sources.is_some_and(|mask| {
                    image.is_some_and(|i| i < 256 && mask[i / 8] & (1 << (i % 8)) != 0)
                });
                if explicit || Some(source as usize) != image {
                    return Ok(GlyphKey::Source(source));
                }
            }
        }
        if !self.custom && resident(self.id).is_some() {
            // Legacy text was decoded for Unicode-aware layout. Recover its
            // byte index here; layout-generated characters use measured Unicode.
            let source = if self.legacy_codepage {
                image.filter(|&i| {
                    i <= 255
                        && (matches!(self.encoding, 0 | 13) || c != '\\' || self.legacy_backslash)
                })
            } else {
                None
            };
            if let Some(source) = source {
                // ^SN keeps the measured source zero under CI13; FD/SF use the
                // input map. The native serial preview controls distinguish them.
                if self.serial_zero_source && self.encoding == 13 && source == usize::from(b'0') {
                    return Ok(GlyphKey::Source(source as u8));
                }
                // Preserve the input byte independently of explicit source-slot
                // remapping. CI13's zero is not the CI0 source-slot zero.
                return Ok(if matches!(self.encoding, 0 | 13) {
                    GlyphKey::Input(source as u8)
                } else {
                    GlyphKey::Source(source as u8)
                });
            }
            if self.control_spaces && matches!(c, '\u{1b}' | '\u{7f}') {
                return Ok(GlyphKey::Source(b' '));
            }
            if self.legacy_controls && matches!(c, '\u{1b}' | '\u{7f}') {
                return Ok(GlyphKey::Source(c as u8));
            }
        }
        let c = match self
            .character_map
            .and_then(|map| image.and_then(|image| map.get(image).copied()))
        {
            Some(source) if Some(source as usize) != image => match source {
                // ^CI example, Programming Guide p. 158: legacy image 21 is euro.
                21 => '€',
                0..=127 => char::from(source),
                _ if self.legacy_codepage => legacy_char(source),
                _ => return Err("unsupported legacy remap source glyph".into()),
            },
            _ => c,
        };
        // Canonical keys identify private legacy-only strike entries. They do
        // not add Unicode arrow/house support to the normal resident strikes.
        Ok(GlyphKey::Unicode(match c {
            '\u{1b}' | '\u{7f}' if self.control_spaces => ' ',
            '\u{1b}' if self.legacy_controls => '\u{2190}',
            '\u{7f}' if self.legacy_controls => '\u{2302}',
            _ => c,
        }))
    }
    pub(crate) fn with_default_glyph(mut self, enabled: bool) -> Self {
        self.default_glyph = enabled;
        self
    }
}
impl From<char> for Selection {
    fn from(id: char) -> Self {
        Self::new(id, false)
    }
}
pub(crate) fn selected(id: impl Into<Selection> + Copy, w: f64, h: f64) -> (GlyphSet, f64, f64) {
    // C and D share the 18x10 matrix (ZPL Programming Guide Table 31,
    // p. 1583); resident-bc-zd621-v1 verifies the alias across all ASCII.
    let font = id.into();
    if let Some(face) = resident(font.id) {
        let metrics = face.cell_metrics().expect("resident cell metrics");
        return (
            GlyphSet::Compact(face, font.legacy_backslash, font.encoding),
            w / f64::from(metrics.cell_width),
            h / f64::from(metrics.cell_height),
        );
    }
    let id = font.id;
    let faces = variants(font);
    for &s in faces {
        let sw = if s.width == 0 { s.height } else { s.width } as f64;
        if s.font == id
            && (!matches!(id, '0' | 'P' | 'Q' | 'R' | 'S' | 'T' | 'U' | 'V')
                || (s.height as f64 == h && sw == w))
        {
            return (GlyphSet::Captured(s), w / sw, h / s.height as f64);
        }
    }
    if matches!(id, 'P' | 'Q' | 'R' | 'S' | 'T' | 'U' | 'V') {
        // Preset strikes are hinted independently at each size. Use an exact
        // captured size above, otherwise scale the closest sampled strike.
        // Table 31 p. 1584 gives each preset's native matrix.
        let distance = |s: &&zpl_bitmap_fonts::captures::Strike| {
            (w / s.width as f64).ln().abs() + (h / s.height as f64).ln().abs()
        };
        let s = faces
            .iter()
            .filter(|s| s.font == id)
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
            .expect("embedded preset font");
        return (
            GlyphSet::Captured(s),
            w / s.width as f64,
            h / s.height as f64,
        );
    }
    fallback(font, w, h)
}
fn fallback(font: Selection, w: f64, h: f64) -> (GlyphSet, f64, f64) {
    (
        GlyphSet::Captured(variants(font).last().unwrap()),
        w / 32.,
        h / 32.,
    )
}
fn variants(font: Selection) -> &'static [&'static zpl_bitmap_fonts::captures::Strike] {
    zpl_bitmap_fonts::captures::zd621::VARIANTS[usize::from(font.legacy_backslash)
        + 2 * usize::from(font.default_glyph)
        + 4 * usize::from(font.legacy_controls)]
}
// Native strikes may cover only ASCII. Keep the enriched base strike's
// Unicode, legacy and default glyphs available with their original scale.
pub(crate) fn selected_for_char(
    font: Selection,
    c: GlyphKey,
    w: f64,
    h: f64,
) -> (GlyphSet, f64, f64) {
    let face = selected(font, w, h);
    if font.id == '0' && glyph_from(face.0, c).is_err() {
        for &settings in zpl_bitmap_fonts::captures::zd621::RETAIL {
            let sw = if settings.width == 0 {
                settings.height
            } else {
                settings.width
            } as f64;
            if glyph_from(GlyphSet::Captured(settings), c).is_ok()
                && ((w == sw && h == settings.height as f64) || settings.width == 0)
            {
                return (
                    GlyphSet::Captured(settings),
                    w / sw,
                    h / settings.height as f64,
                );
            }
        }
        fallback(font, w, h)
    } else {
        face
    }
}
#[derive(Clone, Copy)]
pub(crate) enum GlyphSet {
    Captured(&'static zpl_bitmap_fonts::captures::Strike),
    Compact(&'static zpl_bitmap_fonts::Font, bool, u8),
}
pub(crate) enum Pixels<'a> {
    Compact(zpl_bitmap_fonts::Glyph),
    Custom(std::borrow::Cow<'a, crate::bitmap_font::Glyph>),
}
pub(crate) struct GlyphView<'a> {
    pub(crate) advance: u32,
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Pixels<'a>,
}
pub(crate) fn glyph_from(
    glyphs: GlyphSet,
    c: impl Into<GlyphKey>,
) -> Result<GlyphView<'static>, String> {
    let key = c.into();
    let glyph = match glyphs {
        GlyphSet::Captured(strike) => match key {
            GlyphKey::Unicode(c) => strike.glyph(c as u32),
            _ => None,
        },
        GlyphSet::Compact(face, legacy, encoding) => compact_glyph(key, face, legacy, encoding),
    }
    .ok_or_else(|| {
        // Retain historical diagnostic spelling without using Unicode markers
        // to route lookups. Actual Unicode PUA input never becomes a source key.
        let c = match key {
            GlyphKey::Unicode(c) => c,
            GlyphKey::Source(key) => char::from_u32(0xf0000 + u32::from(key)).unwrap(),
            GlyphKey::Input(key) => char::from_u32(0xf0100 + u32::from(key)).unwrap(),
        };
        format!("unsupported embedded font glyph {c:?}")
    })?;
    Ok(GlyphView {
        advance: u32::from(glyph.advance),
        left: i32::from(glyph.left),
        top: i32::from(glyph.top),
        width: u32::from(glyph.width),
        height: u32::from(glyph.height),
        pixels: Pixels::Compact(glyph),
    })
}
pub(crate) fn resident(id: char) -> Option<&'static zpl_bitmap_fonts::Font> {
    zpl_bitmap_fonts::resident(id)
}
pub(crate) fn compact_glyph(
    key: impl Into<GlyphKey>,
    face: &zpl_bitmap_fonts::Font,
    legacy_backslash: bool,
    encoding: u8,
) -> Option<zpl_bitmap_fonts::Glyph> {
    use zpl_bitmap_fonts::collection::Encoding;
    let c = match key.into() {
        GlyphKey::Source(source) => return face.glyph(source),
        GlyphKey::Input(byte) => {
            return face
                .encoded_glyph(Encoding::Input { ci: encoding }, u32::from(byte))
                .or_else(|| face.glyph(byte).filter(|g| g.width == 0 && g.height == 0))
        }
        GlyphKey::Unicode(c) => c,
    };
    // Profiles select the measured legacy or modern backslash behavior. The
    // per-face mappings carry the actual glyph choice, including symbol fonts.
    let ci = if c == '\\' && !legacy_backslash {
        27
    } else {
        28
    };
    let measured = if c == '\\' {
        face.encoded_glyph(Encoding::Input { ci }, c as u32)
    } else if face.encoding(Encoding::Input { ci: encoding }).is_some()
        && code_page(encoding).is_some()
    {
        // Decoding remains necessary for layout. These supported code pages have
        // unique defined character encodings, so recover their original input byte.
        let mut utf8 = [0; 4];
        let (bytes, _, errors) = code_page(encoding)?.encode(c.encode_utf8(&mut utf8));
        if !errors && bytes.len() == 1 {
            face.encoded_glyph(Encoding::Input { ci: encoding }, u32::from(bytes[0]))
        } else {
            None
        }
    } else {
        // Where the requested byte map has not been measured, use its decoded
        // Unicode character and the measured Unicode map, never candidate tables.
        face.encoded_glyph(Encoding::Input { ci: 28 }, c as u32)
    };
    measured
        .or_else(|| {
            // Layout inserts a visible discretionary hyphen after Unicode decoding.
            (c == '\u{ad}')
                .then(|| face.encoded_glyph(Encoding::Input { ci: 27 }, 173))
                .flatten()
        })
        .or_else(|| {
            // Blank-only captures cannot measure advance. Retain the independently
            // verified source advance only when that source glyph is also blank.
            let key = if c.is_ascii() {
                Some(c as u8)
            } else {
                CP850
                    .iter()
                    .position(|&key| key == c)
                    .map(|i| (i + 128) as u8)
            }?;
            face.glyph(key).filter(|g| g.width == 0 && g.height == 0)
        })
}
pub(crate) fn baseline_for(id: impl Into<Selection> + Copy, h: f64) -> f64 {
    let id = id.into().id;
    if let Some(face) = resident(id) {
        let metrics = face.cell_metrics().expect("resident cell metrics");
        return h * (f64::from(metrics.baseline) - 1.) / f64::from(metrics.cell_height);
    }
    // Scalable preset strikes retain their independently measured baselines.
    h * match id {
        // Native Q/R FO-to-FT controls locate these face baselines; Table 29
        // omits the presets (their matrices are in Table 31, p. 1584).
        'Q' => 22. / 28.,
        'R' => 28. / 35.,
        // Independent native FO/FT atlas: resident-tuv-zd621-v1.
        'T' => 36. / 48.,
        'U' => 46. / 59.,
        'V' => 62. / 80.,
        _ => 0.75,
    }
}
// ^CI0/^CI13: Zebra CP850. Unicode mapping reference:
// https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/PC/CP850.TXT
pub(crate) fn legacy_char(byte: u8) -> char {
    if byte.is_ascii() {
        char::from(byte)
    } else {
        CP850[byte as usize - 128]
    }
}

pub(crate) const CP850: [char; 128] = [
    '\u{c7}', '\u{fc}', '\u{e9}', '\u{e2}', '\u{e4}', '\u{e0}', '\u{e5}', '\u{e7}', '\u{ea}',
    '\u{eb}', '\u{e8}', '\u{ef}', '\u{ee}', '\u{ec}', '\u{c4}', '\u{c5}', '\u{c9}', '\u{e6}',
    '\u{c6}', '\u{f4}', '\u{f6}', '\u{f2}', '\u{fb}', '\u{f9}', '\u{ff}', '\u{d6}', '\u{dc}',
    '\u{f8}', '\u{a3}', '\u{d8}', '\u{d7}', '\u{192}', '\u{e1}', '\u{ed}', '\u{f3}', '\u{fa}',
    '\u{f1}', '\u{d1}', '\u{aa}', '\u{ba}', '\u{bf}', '\u{ae}', '\u{ac}', '\u{bd}', '\u{bc}',
    '\u{a1}', '\u{ab}', '\u{bb}', '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}',
    '\u{c1}', '\u{c2}', '\u{c0}', '\u{a9}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255d}',
    '\u{a2}', '\u{a5}', '\u{2510}', '\u{2514}', '\u{2534}', '\u{252c}', '\u{251c}', '\u{2500}',
    '\u{253c}', '\u{e3}', '\u{c3}', '\u{255a}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}',
    '\u{2550}', '\u{256c}', '\u{a4}', '\u{f0}', '\u{d0}', '\u{ca}', '\u{cb}', '\u{c8}', '\u{131}',
    '\u{cd}', '\u{ce}', '\u{cf}', '\u{2518}', '\u{250c}', '\u{2588}', '\u{2584}', '\u{a6}',
    '\u{cc}', '\u{2580}', '\u{d3}', '\u{df}', '\u{d4}', '\u{d2}', '\u{f5}', '\u{d5}', '\u{b5}',
    '\u{fe}', '\u{de}', '\u{da}', '\u{db}', '\u{d9}', '\u{fd}', '\u{dd}', '\u{af}', '\u{b4}',
    '\u{ad}', '\u{b1}', '\u{2017}', '\u{be}', '\u{b6}', '\u{a7}', '\u{f7}', '\u{b8}', '\u{b0}',
    '\u{a8}', '\u{b7}', '\u{b9}', '\u{b3}', '\u{b2}', '\u{25a0}', '\u{a0}',
];

// Zebra Programming Guide ^CI, pp. 156–159:
// https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
pub(crate) fn code_page(ci: u8) -> Option<&'static encoding_rs::Encoding> {
    match ci {
        27 => Some(encoding_rs::WINDOWS_1252),
        31 => Some(encoding_rs::WINDOWS_1250),
        33 => Some(encoding_rs::WINDOWS_1251),
        34 => Some(encoding_rs::WINDOWS_1253),
        35 => Some(encoding_rs::WINDOWS_1254),
        36 => Some(encoding_rs::WINDOWS_1255),
        _ => None,
    }
}

/// Resolve requested sizes using the face's native matrix and profile limits.
/// ZPL Programming Guide ^A pp.60–61, ^CF p.154, Table 31 pp.1583–1584.
pub(crate) fn dimensions(
    (w, h): (f64, f64),
    (default_w, default_h): (f64, f64),
    id: char,
    clamp_minimum: bool,
    clamp_maximum: bool,
) -> Result<(f64, f64), String> {
    if id != '0' {
        // ZPL Programming Guide Table 31, pp. 1583–1584: native bitmap matrices.
        let (nh, nw) = if let Some(face) = zpl_bitmap_fonts::resident(id) {
            let metrics = face.cell_metrics().expect("resident cell metrics");
            (
                f64::from(metrics.cell_height),
                f64::from(metrics.cell_width),
            )
        } else {
            match id {
                'P' => (20., 18.),
                'Q' => (28., 24.),
                'R' => (35., 31.),
                'S' => (40., 35.),
                'T' => (48., 42.),
                'U' => (59., 53.),
                'V' => (80., 71.),
                _ => (18., 10.),
            }
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

/// Captured bitmap-font FT dot placement. Native glyph metrics describe the
/// cell; rotated FT anchors include a final dot boundary. See resident-bc-zd621-v1
/// and the subsequent resident-font suites (through resident-h-zd621-v1),
/// with ^FT p. 205
/// Table 7 in the Programming Guide.
pub(crate) fn printer_ft_offset(id: char, height: f64, rotation: u8) -> (f64, f64) {
    let native = match id {
        'A' => 9.,
        'B' => 11.,
        'C' | 'D' => 18.,
        'E' => 28.,
        'F' => 26.,
        'G' => 60.,
        'H' => 21.,
        GRAPHIC_SYMBOLS => 24.,
        _ => return (0., 0.),
    };
    let scale = height / native;
    match rotation {
        b'R' => (scale, 0.),
        b'I' => (1., scale),
        b'B' => (1. - scale, 1.),
        _ => (0., 1. - scale),
    }
}

/// Return line pitch and baseline adjustment.
///
/// Captured S block pitch and ascent: native FO/FT controls at 40 and 80 dots
/// in resident-s-zd621-v1. ^FB pp. 186–188 describes nominal font-height
/// spacing; this printer uses distinct preset metrics. Other sizes scale the
/// closest measured height, just as unsampled glyph strikes are approximated.
pub(crate) fn block_metrics(id: impl Into<Selection>, h: f64, printer_s: bool) -> (f64, f64) {
    if !printer_s || id.into().id != 'S' {
        return (h, 0.);
    }
    let (native, pitch, ascent) = if h < 60. {
        (40., 34., 25.)
    } else {
        (80., 67., 50.)
    };
    let scale = h / native;
    (pitch * scale, ascent * scale - baseline_for('S', h))
}

pub(crate) fn inverted_margin(id: char, w: f64) -> Option<f64> {
    if id == 'A' {
        return Some(w / 5. + 2.);
    }
    if id == 'B' {
        return Some(2. * w / 7. + 2.);
    }
    if id == 'E' {
        // resident-e-zd621-v1: OCR-B's inverted margin is six native dots,
        // although its ordinary advance includes only five gap dots.
        return Some(6. * w / 15. + 2.);
    }
    if id == 'H' {
        return Some(6. * w / 13. + 2.);
    }
    if id == 'G' {
        return Some(8. * w / 40. + 2.);
    }
    if id == 'F' {
        return Some(3. * w / 13. + 2.);
    }
    if matches!(id, 'C' | 'D') {
        return Some(w / 5. + 2.);
    }
    None
}

/// Decode field bytes before Unicode layout; source remapping is font-specific
/// and applied by Selection after layout inserts any discretionary characters.
pub(crate) fn decode(bytes: &[u8], encoding: u8) -> Result<std::borrow::Cow<'_, str>, String> {
    if let Some(page) = code_page(encoding) {
        page.decode_without_bom_handling_and_without_replacement(bytes)
            .ok_or_else(|| "undefined code page byte".into())
    } else if matches!(encoding, 0 | 13) && !bytes.is_ascii() {
        Ok(std::borrow::Cow::Owned(
            bytes.iter().map(|&b| legacy_char(b)).collect(),
        ))
    } else {
        std::str::from_utf8(bytes)
            .map(std::borrow::Cow::Borrowed)
            .map_err(|_| "invalid UTF-8 text".into())
    }
}

impl Selection {
    pub(crate) fn has_glyph_fallback(self) -> bool {
        self.id == '0'
    }
    pub(crate) fn direction_pivot_inset(self) -> f64 {
        direction_pivot_inset(self.id)
    }
    pub(crate) fn bounded_pitch(
        self,
        h: f64,
        advance: impl FnOnce() -> Result<f64, String>,
    ) -> Result<f64, String> {
        // ZD621 TB controls: proportional leading is 25%; A's leading is
        // quantized at its horizontally magnified cell (bounded-text-zd621-v1).
        Ok(match self.id {
            '0' => h * 1.25,
            'A' => {
                let width = advance()? * 5. / 6.;
                h * (width * 1.1).floor() / width
            }
            _ => h,
        })
    }
}
/// Measured FO pivot insets, in scaled native dots. P/Q quantize to cells;
/// R retains its matrix boundary. T/U/V: resident-tuv-zd621-v1.
pub(crate) fn last_row(id: char, h: f64) -> f64 {
    match id {
        'P' => h / 20.,
        'Q' => h / 28.,
        'R' => 0.,
        'S' => 2. * h / 40.,
        'T' => 3. * h / 48.,
        'U' => h / 59.,
        'V' => 2. * h / 80.,
        _ => 1.,
    }
}

// Classifications belong to the face catalog, not scene construction.
pub(crate) fn is_scalable(id: char) -> bool {
    id == '0'
}
pub(crate) fn is_preset(id: char) -> bool {
    matches!(id, 'P'..='V')
}
pub(crate) fn is_graphic_symbols(id: char) -> bool {
    id == GRAPHIC_SYMBOLS
}
pub(crate) fn direction_pivot_inset(id: char) -> f64 {
    if is_scalable(id) {
        1.
    } else {
        0.
    }
}
pub(crate) fn adjust_baseline(id: char, baseline: f64, h: f64, graphic_last_row: bool) -> f64 {
    // Table 29 p.1582 gives GS a 3/4-height baseline. Native GS controls use
    // row 23 of 24; keep that measured deviation independently selectable.
    if is_graphic_symbols(id) && !graphic_last_row {
        baseline - h * 5. / 24.
    } else {
        baseline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_private_use_is_not_a_printer_source_slot() {
        // U+F0000 used to collide with the renderer's private source marker.
        // Unicode scalars and explicit ^CI source indices are separate domains.
        // ZPL guide ^CI pp.156–159; PR #53's CI13 captures verify input/source
        // distinctions even for explicit identity remapping.
        let font = Selection::new('A', false);
        let (set, _, _) = selected(font, 5., 9.);
        assert!(glyph_from(set, font.map_char('\u{f0041}').unwrap()).is_err());
        assert!(glyph_from(set, GlyphKey::Source(65)).is_ok());
        let mut map = std::array::from_fn(|i| i as u8);
        map[65] = 92;
        assert!(matches!(
            font.with_character_map(Some(map)).map_char('A').unwrap(),
            GlyphKey::Source(92)
        ));
        let legacy = font.with_legacy_codepage(true).with_encoding(13);
        assert!(matches!(legacy.map_char('0').unwrap(), GlyphKey::Input(48)));
        let mut explicit = [0; 32];
        explicit[48 / 8] |= 1 << (48 % 8);
        assert!(matches!(
            legacy
                .with_character_map(Some(std::array::from_fn(|i| i as u8)))
                .with_explicit_sources(Some(explicit))
                .map_char('0')
                .unwrap(),
            GlyphKey::Source(48)
        ));
    }
}
