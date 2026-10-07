//! Shared native bitmap fonts and captured scalable-font strikes.
//! Bitmap pixels become output-neutral paths.
//! Metrics: ZPL Programming Guide Tables 29/31, pp. 1582–1583:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use crate::{
    bitmap_font::{self, Glyph, Settings, GRAPHIC_SYMBOLS},
    output::{Path, Point, Segment},
};
use std::{collections::BTreeMap, sync::OnceLock};

/// Emit sorted bitmap rows, extending identical runs that touch vertically.
/// Joining only exactly shared edges preserves the even-odd region, including
/// fractional scales whose adjacent row boundaries can differ by an ULP.
#[derive(Default)]
struct RowPath {
    // Keep rectangles small while joining rows. Expanding every intermediate
    // rectangle to five general segments makes buffer growth depend heavily
    // on allocator fragmentation, including the font initialization history.
    rectangles: Vec<(Point, Point)>,
    previous: Vec<usize>,
    current: Vec<usize>,
    row: Option<f64>,
    next: usize,
}
impl RowPath {
    fn rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        if width <= 0. || height <= 0. {
            return;
        }
        if self.row != Some(y) {
            std::mem::swap(&mut self.previous, &mut self.current);
            self.current.clear();
            self.next = 0;
            self.row = Some(y);
        }
        let right = x + width;
        let bottom = y + height;
        while let Some(&index) = self.previous.get(self.next) {
            let (a, c) = self.rectangles[index];
            if a.x < x {
                self.next += 1;
                continue;
            }
            if a.x == x && c.x == right && c.y == y {
                self.rectangles[index].1.y = bottom;
                self.current.push(index);
                self.next += 1;
                return;
            }
            break;
        }
        self.current.push(self.rectangles.len());
        self.rectangles
            .push((Point::new(x, y), Point::new(right, bottom)));
    }

    fn into_path(self) -> Path {
        let mut segments = Vec::with_capacity(self.rectangles.len() * 5);
        for (a, c) in self.rectangles {
            // Preserve computed endpoints exactly, including fractional row
            // boundaries; do not subtract and re-add widths or heights.
            segments.extend([
                Segment::Move(a),
                Segment::Line(Point::new(c.x, a.y)),
                Segment::Line(c),
                Segment::Line(Point::new(a.x, c.y)),
                Segment::Close,
            ]);
        }
        Path { segments }
    }
}

/// Resident face plus the legacy ASCII backslash replacement. Keep this
/// separate from U+00A2: bitmap faces have distinct native cent designs.
#[derive(Clone, Copy)]
pub(super) struct Font {
    id: char,
    legacy_backslash: bool,
    legacy_codepage: bool,
    encoding: u8,
    default_glyph: bool,
    character_map: Option<[u8; 256]>,
    explicit_sources: Option<[u8; 32]>,
    block_flow: Option<BlockFlow>,
    tab_stops: bool,
    legacy_controls: bool,
    control_spaces: bool,
}
#[derive(Clone, Copy)]
struct BlockFlow {
    direction: u8,
    gap: f64,
    vertical_gap: f64,
    printer_layout: bool,
    skip_space_gap: bool,
}
impl BlockFlow {
    fn gap_for(self, c: char) -> f64 {
        if self.skip_space_gap && self.direction != b'V' && c == ' ' {
            0.
        } else {
            self.gap
        }
    }
}

impl Font {
    pub(super) fn new(id: char, legacy_backslash: bool) -> Self {
        Self {
            id,
            legacy_backslash,
            legacy_codepage: false,
            encoding: 28,
            default_glyph: false,
            character_map: None,
            explicit_sources: None,
            block_flow: None,
            tab_stops: false,
            legacy_controls: false,
            control_spaces: false,
        }
    }
    pub(super) fn with_encoding(mut self, encoding: u8) -> Self {
        self.encoding = encoding;
        self
    }
    pub(super) fn with_legacy_codepage(mut self, enabled: bool) -> Self {
        self.legacy_codepage = enabled;
        self
    }
    pub(super) fn with_tab_stops(mut self, enabled: bool) -> Self {
        self.tab_stops = enabled;
        self
    }
    pub(super) fn is_tab(self, c: char) -> bool {
        self.tab_stops && c == '\t'
    }
    pub(super) fn with_character_map(mut self, map: Option<[u8; 256]>) -> Self {
        self.character_map = map;
        self
    }
    pub(super) fn with_explicit_sources(mut self, mask: Option<[u8; 32]>) -> Self {
        self.explicit_sources = mask;
        self
    }
    pub(super) fn with_control_glyphs(mut self, legacy: bool, spaces: bool) -> Self {
        self.legacy_controls = legacy;
        self.control_spaces = spaces;
        self
    }
    fn map_char(self, c: char) -> Result<char, String> {
        // Text is already Unicode before layout. Recover the input image index
        // only for ^CI remapping; do not decode layout-generated soft hyphens
        // (U+00AD) as CP850 byte AD (inverted exclamation mark).
        let image = if self.legacy_codepage && !c.is_ascii() {
            CP850.iter().position(|&glyph| glyph == c).map(|i| i + 128)
        } else {
            Some(c as usize)
        };
        if resident(self.id).is_some() {
            if let Some(source) = self
                .character_map
                .and_then(|map| image.and_then(|i| map.get(i).copied()))
            {
                let explicit = self.explicit_sources.is_some_and(|mask| {
                    image.is_some_and(|i| i < 256 && mask[i / 8] & (1 << (i % 8)) != 0)
                });
                if explicit || Some(source as usize) != image {
                    return Ok(char::from_u32(0xf0000 + u32::from(source)).unwrap());
                }
            }
        }
        if resident(self.id).is_some() {
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
                // Preserve the input byte independently of explicit source-slot
                // remapping. CI13's zero is not the CI0 source-slot zero.
                let tag = if matches!(self.encoding, 0 | 13) {
                    0xf0100
                } else {
                    0xf0000
                };
                return Ok(char::from_u32(tag + source as u32).unwrap());
            }
            if self.control_spaces && matches!(c, '\u{1b}' | '\u{7f}') {
                return Ok(char::from_u32(0xf0000 + u32::from(b' ')).unwrap());
            }
            if self.legacy_controls && matches!(c, '\u{1b}' | '\u{7f}') {
                return Ok(char::from_u32(0xf0000 + c as u32).unwrap());
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
        Ok(match c {
            '\u{1b}' | '\u{7f}' if self.control_spaces => ' ',
            '\u{1b}' if self.legacy_controls => '\u{2190}',
            '\u{7f}' if self.legacy_controls => '\u{2302}',
            _ => c,
        })
    }
    pub(super) fn with_default_glyph(mut self, enabled: bool) -> Self {
        self.default_glyph = enabled;
        self
    }
    pub(super) fn with_block_flow(
        mut self,
        direction: (u8, f64),
        compatibility: super::compatibility::Compatibility,
    ) -> Self {
        if direction != (b'H', 0.) {
            self.block_flow = Some(BlockFlow {
                direction: direction.0,
                gap: direction.1,
                vertical_gap: if compatibility.field_vertical_ignores_gap {
                    0.
                } else {
                    direction.1
                },
                printer_layout: compatibility.block_field_direction_printer_layout,
                skip_space_gap: compatibility.block_spaces_ignore_character_gap,
            });
        }
        self
    }
    pub(super) fn bounded_pitch(self, w: f64, h: f64) -> Result<f64, String> {
        // ZD621 TB controls: proportional line leading is 25%; A's leading
        // is quantized at its horizontally magnified bitmap cell width.
        Ok(match self.id {
            '0' => h * 1.25,
            'A' => {
                let width = width_for(self, "A", w, h)? * 5. / 6.;
                h * (width * 1.1).floor() / width
            }
            _ => h,
        })
    }
    pub(super) fn block_overprints(self) -> bool {
        self.block_flow
            .is_some_and(|flow| flow.printer_layout && flow.direction == b'V')
    }
    pub(super) fn block_position(self, position: f64) -> f64 {
        if self.block_reverses() {
            -position
        } else {
            position
        }
    }
    pub(super) fn block_reverses(self) -> bool {
        self.block_flow
            .is_some_and(|flow| flow.printer_layout && flow.direction == b'R')
    }
}
impl From<char> for Font {
    fn from(id: char) -> Self {
        Self::new(id, false)
    }
}
const DATA: &[u8] = include_bytes!("../../assets/font0-32.zbf");
fn strike() -> &'static (Settings, Vec<Glyph>) {
    static FONT: OnceLock<(Settings, Vec<Glyph>)> = OnceLock::new();
    FONT.get_or_init(|| {
        let (settings, mut glyphs) = bitmap_font::unpack(DATA).expect("validated ASCII strike");
        glyphs.extend(
            bitmap_font::unpack(include_bytes!("../../assets/font0-32-latin1.zbf"))
                .expect("validated Latin-1 supplement")
                .1,
        );
        extend_hyphens(settings, &mut glyphs);
        extend_cent(settings, &mut glyphs);
        extend_unicode(settings, &mut glyphs);
        (settings, glyphs)
    })
}
// Unicode Hebrew letters, captured with CI28 and PA0 (Zebra guide ^CI,
// pp. 156–159 and ^PA p. 315). Native pages and advance controls are pinned
// in unicode-fonts-zd621-v1, including all five final forms.
fn extend_unicode(settings: Settings, glyphs: &mut Vec<Glyph>) {
    let sources: &[&[u8]] = match (settings.font, settings.height, settings.width) {
        ('0', 32, 0) => &[
            include_bytes!("../../assets/font0-32-0-hebrew.zbf"),
            include_bytes!("../../assets/font0-32-0-missing.zbf"),
        ],
        ('0', 40, 24) => &[
            include_bytes!("../../assets/font0-40-24-hebrew.zbf"),
            include_bytes!("../../assets/font0-40-24-extended.zbf"),
            include_bytes!("../../assets/font0-40-24-missing.zbf"),
            include_bytes!("../../assets/font0-40-24-conformance.zbf"),
            include_bytes!("../../assets/font0-40-24-controls.zbf"),
        ],
        _ => return,
    };
    for data in sources {
        // Missing-glyph captures include an ASCII A as an independent visible
        // verification anchor; the base strike already supplies that glyph.
        glyphs.extend(
            bitmap_font::unpack(data)
                .expect("validated Unicode strike")
                .1
                .into_iter()
                .filter(|g| g.codepoint > 126),
        );
    }
    glyphs.sort_by_key(|g| g.codepoint);
    // Overlapping independent captures retain the existing encoding's glyph.
    // In particular CI27 soft hyphen is visible; CI28 formatting is handled
    // before layout, while FB may still generate its printable hyphen.
    glyphs.dedup_by_key(|g| g.codepoint);
}
// CI27 cent character, measured independently at each embedded strike.
// ^CI pp. 156–159; raw pages and advance checks: cent-glyph-zd621-v1.
fn extend_cent(settings: Settings, glyphs: &mut Vec<Glyph>) {
    let data: &[u8] = match (settings.font, settings.height, settings.width) {
        ('0', 16, 0) => include_bytes!("../../assets/font0-16-0-cent.zbf"),
        ('0', 20, 0) => include_bytes!("../../assets/font0-20-0-cent.zbf"),
        ('0', 24, 24) => include_bytes!("../../assets/font0-24-24-cent.zbf"),
        ('0', 32, 0) => include_bytes!("../../assets/font0-32-0-cent.zbf"),
        ('0', 32, 16) => include_bytes!("../../assets/font0-32-16-cent.zbf"),
        ('0', 32, 24) => include_bytes!("../../assets/font0-32-24-cent.zbf"),
        ('0', 32, 64) => include_bytes!("../../assets/font0-32-64-cent.zbf"),
        ('0', 64, 0) => include_bytes!("../../assets/font0-64-0-cent.zbf"),
        _ => return,
    };
    glyphs.extend(
        bitmap_font::unpack(data)
            .expect("validated cent supplement")
            .1,
    );
    glyphs.sort_by_key(|g| g.codepoint);
}
// Independently captured soft-hyphen and eth supplements. ^FB p. 187;
// see field-block-hyphenation-zd621-v1 for the CI27 automatic-break departure.
fn extend_hyphens(settings: Settings, glyphs: &mut Vec<Glyph>) {
    let data: &[u8] = match (settings.font, settings.height, settings.width) {
        ('0', 28, 14) => include_bytes!("../../assets/font0-28-14-hyphen.zbf"),
        ('0', 24, 24) => include_bytes!("../../assets/font0-24-24-hyphen.zbf"),
        ('0', 32, 0) => include_bytes!("../../assets/font0-32-0-hyphen.zbf"),
        ('0', 16, 0) => include_bytes!("../../assets/font0-16-0-hyphen.zbf"),
        ('0', 20, 0) => include_bytes!("../../assets/font0-20-0-hyphen.zbf"),
        ('0', 64, 0) => include_bytes!("../../assets/font0-64-0-hyphen.zbf"),
        ('0', 32, 16) => include_bytes!("../../assets/font0-32-16-hyphen.zbf"),
        ('0', 32, 24) => include_bytes!("../../assets/font0-32-24-hyphen.zbf"),
        ('0', 32, 64) => include_bytes!("../../assets/font0-32-64-hyphen.zbf"),
        _ => return,
    };
    glyphs.extend(
        bitmap_font::unpack(data)
            .expect("validated hyphen supplement")
            .1,
    );
    glyphs.sort_by_key(|g| g.codepoint);
}
fn strikes() -> &'static Vec<(Settings, Vec<Glyph>)> {
    static STRIKES: OnceLock<Vec<(Settings, Vec<Glyph>)>> = OnceLock::new();
    STRIKES.get_or_init(|| {
        [
            // Native SurePost sizes; sampling and independent controls in
            // tests/fixtures/surepost-zd621-v1 (^A guide p. 60).
            include_bytes!("../../assets/font0-18-22.zbf").as_slice(),
            include_bytes!("../../assets/font0-20-24.zbf").as_slice(),
            include_bytes!("../../assets/font0-22-26.zbf").as_slice(),
            include_bytes!("../../assets/font0-23-23.zbf").as_slice(),
            include_bytes!("../../assets/font0-26-30.zbf").as_slice(),
            include_bytes!("../../assets/font0-28-32.zbf").as_slice(),
            include_bytes!("../../assets/font0-30-34.zbf").as_slice(),
            include_bytes!("../../assets/font0-39-42.zbf").as_slice(),
            include_bytes!("../../assets/font0-45-44.zbf").as_slice(),
            include_bytes!("../../assets/font0-72-68.zbf").as_slice(),
            // Native shipping/typography sizes, independently reconstructed
            // from shipping-fonts-zd621-v1; ^A guide p. 60, FO/FT pp. 201/205.
            include_bytes!("../../assets/font0-20-12.zbf").as_slice(),
            include_bytes!("../../assets/font0-28-15.zbf").as_slice(),
            include_bytes!("../../assets/font0-36-22.zbf").as_slice(),
            include_bytes!("../../assets/font0-28-16.zbf").as_slice(),
            include_bytes!("../../assets/font0-52-30.zbf").as_slice(),
            include_bytes!("../../assets/font0-42-24.zbf").as_slice(),
            include_bytes!("../../assets/font0-10-0.zbf").as_slice(),
            include_bytes!("../../assets/font0-10-32.zbf").as_slice(),
            include_bytes!("../../assets/font0-32-10.zbf").as_slice(),
            include_bytes!("../../assets/font0-15-0.zbf").as_slice(),
            include_bytes!("../../assets/font0-17-0.zbf").as_slice(),
            include_bytes!("../../assets/font0-31-0.zbf").as_slice(),
            include_bytes!("../../assets/font0-33-0.zbf").as_slice(),
            include_bytes!("../../assets/font0-63-0.zbf").as_slice(),
            include_bytes!("../../assets/font0-65-0.zbf").as_slice(),
            include_bytes!("../../assets/font0-64-16.zbf").as_slice(),
            include_bytes!("../../assets/font0-16-64.zbf").as_slice(),
            include_bytes!("../../assets/font0-96-96.zbf").as_slice(),
            include_bytes!("../../assets/font0-16-10.zbf").as_slice(),
            include_bytes!("../../assets/fontT-48-42.zbf").as_slice(),
            include_bytes!("../../assets/fontU-59-53.zbf").as_slice(),
            include_bytes!("../../assets/fontV-80-71.zbf").as_slice(),
            include_bytes!("../../assets/font0-16-0.zbf").as_slice(),
            include_bytes!("../../assets/font0-20-0.zbf").as_slice(),
            include_bytes!("../../assets/font0-20-18.zbf").as_slice(),
            include_bytes!("../../assets/font0-20-10.zbf").as_slice(),
            include_bytes!("../../assets/font0-24-24.zbf").as_slice(),
            include_bytes!("../../assets/font0-24-12.zbf").as_slice(),
            include_bytes!("../../assets/font0-24-16.zbf").as_slice(),
            // Native 28-dot natural width: qr-segmentation-zd621-v1/font-0-28.
            include_bytes!("../../assets/font0-28-0.zbf").as_slice(),
            include_bytes!("../../assets/font0-28-14.zbf").as_slice(),
            include_bytes!("../../assets/font0-26-16.zbf").as_slice(),
            include_bytes!("../../assets/font0-40-24.zbf").as_slice(),
            include_bytes!("../../assets/font0-40-22.zbf").as_slice(),
            include_bytes!("../../assets/font0-48-32.zbf").as_slice(),
            include_bytes!("../../assets/font0-64-0.zbf").as_slice(),
            include_bytes!("../../assets/font0-32-16.zbf").as_slice(),
            include_bytes!("../../assets/font0-32-24.zbf").as_slice(),
            include_bytes!("../../assets/font0-32-20.zbf").as_slice(),
            include_bytes!("../../assets/font0-32-64.zbf").as_slice(),
            include_bytes!("../../assets/fontS-40-35.zbf").as_slice(),
            include_bytes!("../../assets/fontS-80-70.zbf").as_slice(),
            include_bytes!("../../assets/fontP-20-18.zbf").as_slice(),
            include_bytes!("../../assets/fontP-40-18.zbf").as_slice(),
            include_bytes!("../../assets/fontP-40-36.zbf").as_slice(),
            include_bytes!("../../assets/fontQ-28-24.zbf").as_slice(),
            include_bytes!("../../assets/fontR-35-31.zbf").as_slice(),
        ]
        .into_iter()
        .map(|data| {
            let (settings, mut glyphs) =
                bitmap_font::unpack(data).expect("validated resident strike");
            extend_hyphens(settings, &mut glyphs);
            extend_cent(settings, &mut glyphs);
            extend_unicode(settings, &mut glyphs);
            (settings, glyphs)
        })
        .collect()
    })
}
// Native CI0 byte 0x5C, sampled separately from Unicode U+00A2.
// ^CI p. 159; independent advance controls in legacy-backslash-zd621-v1.
fn legacy_strikes() -> &'static Vec<(Settings, Vec<Glyph>)> {
    static FONTS: OnceLock<Vec<(Settings, Vec<Glyph>)>> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut faces = strikes().clone();
        faces.push(strike().clone());
        for (settings, glyphs) in &mut faces {
            let data: Option<&[u8]> = match (settings.font, settings.height, settings.width) {
                ('0', 16, 0) => Some(include_bytes!(
                    "../../assets/font0-16-0-legacy-backslash.zbf"
                )),
                ('0', 20, 0) => Some(include_bytes!(
                    "../../assets/font0-20-0-legacy-backslash.zbf"
                )),
                ('0', 40, 24) => Some(include_bytes!(
                    "../../assets/font0-40-24-legacy-backslash.zbf"
                )),
                ('0', 28, 14) => Some(include_bytes!(
                    "../../assets/font0-28-14-legacy-backslash.zbf"
                )),
                ('0', 24, 24) => Some(include_bytes!(
                    "../../assets/font0-24-24-legacy-backslash.zbf"
                )),
                ('0', 32, 0) => Some(include_bytes!(
                    "../../assets/font0-32-0-legacy-backslash.zbf"
                )),
                ('0', 32, 16) => Some(include_bytes!(
                    "../../assets/font0-32-16-legacy-backslash.zbf"
                )),
                ('0', 32, 24) => Some(include_bytes!(
                    "../../assets/font0-32-24-legacy-backslash.zbf"
                )),
                ('0', 32, 64) => Some(include_bytes!(
                    "../../assets/font0-32-64-legacy-backslash.zbf"
                )),
                ('0', 64, 0) => Some(include_bytes!(
                    "../../assets/font0-64-0-legacy-backslash.zbf"
                )),
                _ => None,
            };
            if let Some(data) = data {
                let (_, replacement) =
                    bitmap_font::unpack(data).expect("validated legacy backslash");
                let index = glyphs
                    .binary_search_by_key(&u32::from(b'\\'), |g| g.codepoint)
                    .expect("ASCII backslash");
                glyphs[index] = replacement[0].clone();
            } else if settings.font == '0' {
                // Without a sampled legacy replacement, use the enriched
                // base strike's cent-shaped backslash, not the ASCII slash.
                glyphs.retain(|g| g.codepoint != u32::from(b'\\'));
            }
        }
        faces
    })
}
// ^PAa, Zebra guide p. 315: use the resident default glyph rather than a
// space for measured missing characters. Native controls: advanced-text-zd621-v1.
fn default_glyph_strikes(legacy: bool) -> &'static Vec<(Settings, Vec<Glyph>)> {
    static NORMAL: OnceLock<Vec<(Settings, Vec<Glyph>)>> = OnceLock::new();
    static LEGACY: OnceLock<Vec<(Settings, Vec<Glyph>)>> = OnceLock::new();
    (if legacy { &LEGACY } else { &NORMAL }).get_or_init(|| {
        let mut faces = if legacy {
            legacy_strikes().clone()
        } else {
            let mut faces = strikes().clone();
            faces.push(strike().clone());
            faces
        };
        for (settings, glyphs) in &mut faces {
            let data: &[u8] = match (settings.font, settings.height, settings.width) {
                ('0', 32, 0) => include_bytes!("../../assets/font0-32-0-default-glyph.zbf"),
                ('0', 40, 24) => include_bytes!("../../assets/font0-40-24-default-glyph.zbf"),
                _ => continue,
            };
            for glyph in bitmap_font::unpack(data)
                .expect("validated default glyph strike")
                .1
                .into_iter()
                .filter(|g| g.codepoint > 126)
            {
                match glyphs.binary_search_by_key(&glyph.codepoint, |g| g.codepoint) {
                    Ok(i) => glyphs[i] = glyph,
                    Err(i) => glyphs.insert(i, glyph),
                }
            }
        }
        faces
    })
}
// Native ESC/DEL samples and independent CI13 verification:
// legacy-controls-zd621-v1. Keep these glyphs separate from Unicode faces and
// from the independently selectable legacy-backslash and PA default glyphs.
type ControlStrikes = Vec<(Settings, Vec<Glyph>)>;
fn control_strikes(
    legacy_backslash: bool,
    default_glyph: bool,
) -> &'static Vec<(Settings, Vec<Glyph>)> {
    static FACES: [OnceLock<ControlStrikes>; 4] = [const { OnceLock::new() }; 4];
    FACES[usize::from(legacy_backslash) + 2 * usize::from(default_glyph)].get_or_init(|| {
        let mut faces = if default_glyph {
            default_glyph_strikes(legacy_backslash).clone()
        } else if legacy_backslash {
            legacy_strikes().clone()
        } else {
            let mut faces = strikes().clone();
            faces.push(strike().clone());
            faces
        };
        for (settings, glyphs) in &mut faces {
            let data: &[u8] = match (
                settings.font,
                settings.height,
                settings.width,
                default_glyph,
            ) {
                ('0', 32, 0, true) => {
                    include_bytes!("../../assets/font0-32-0-legacy-controls-default.zbf")
                }
                ('0', 16, 0, true) => {
                    include_bytes!("../../assets/font0-16-0-legacy-controls-default.zbf")
                }
                ('0', 32, 0, false) => {
                    include_bytes!("../../assets/font0-32-0-legacy-controls.zbf")
                }
                ('0', 16, 0, false) => {
                    include_bytes!("../../assets/font0-16-0-legacy-controls.zbf")
                }
                _ => continue,
            };
            for glyph in bitmap_font::unpack(data)
                .expect("validated native control strike")
                .1
            {
                match glyphs.binary_search_by_key(&glyph.codepoint, |g| g.codepoint) {
                    Ok(i) => glyphs[i] = glyph,
                    Err(i) => glyphs.insert(i, glyph),
                }
            }
        }
        faces
    })
}
fn selected(id: impl Into<Font> + Copy, w: f64, h: f64) -> (GlyphSet, f64, f64) {
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
    let faces = if font.legacy_controls {
        control_strikes(font.legacy_backslash, font.default_glyph)
    } else if font.default_glyph {
        default_glyph_strikes(font.legacy_backslash)
    } else if font.legacy_backslash {
        legacy_strikes()
    } else {
        strikes()
    };
    for (s, glyphs) in faces {
        let sw = if s.width == 0 { s.height } else { s.width } as f64;
        if s.font == id
            && (!matches!(id, '0' | 'P' | 'Q' | 'R' | 'S' | 'T' | 'U' | 'V')
                || (s.height as f64 == h && sw == w))
        {
            return (GlyphSet::Captured(glyphs), w / sw, h / s.height as f64);
        }
    }
    if matches!(id, 'P' | 'Q' | 'R' | 'S' | 'T' | 'U' | 'V') {
        // Preset strikes are hinted independently at each size. Use an exact
        // captured size above, otherwise scale the closest sampled strike.
        // Table 31 p. 1584 gives each preset's native matrix.
        let distance =
            |s: &Settings| (w / s.width as f64).ln().abs() + (h / s.height as f64).ln().abs();
        let (s, glyphs) = faces
            .iter()
            .filter(|(s, _)| s.font == id)
            .min_by(|(a, _), (b, _)| distance(a).total_cmp(&distance(b)))
            .expect("embedded preset font");
        return (
            GlyphSet::Captured(glyphs),
            w / s.width as f64,
            h / s.height as f64,
        );
    }
    fallback(font, w, h)
}
fn fallback(font: Font, w: f64, h: f64) -> (GlyphSet, f64, f64) {
    let fallback = if font.legacy_controls {
        &control_strikes(font.legacy_backslash, font.default_glyph)
            .last()
            .unwrap()
            .1
    } else if font.default_glyph {
        &default_glyph_strikes(font.legacy_backslash)
            .last()
            .unwrap()
            .1
    } else if font.legacy_backslash {
        &legacy_strikes().last().unwrap().1
    } else {
        &strike().1
    };
    (GlyphSet::Captured(fallback), w / 32., h / 32.)
}
// Native strikes may cover only ASCII. Keep the enriched base strike's
// Unicode, legacy and default glyphs available with their original scale.
fn selected_for_char(font: Font, c: char, w: f64, h: f64) -> (GlyphSet, f64, f64) {
    let face = selected(font, w, h);
    if font.id == '0' && glyph_from(face.0, c).is_err() {
        static RETAIL: OnceLock<Vec<(Settings, Vec<Glyph>)>> = OnceLock::new();
        let strikes = RETAIL.get_or_init(|| {
            vec![
                bitmap_font::unpack(include_bytes!("../../assets/font0-25-14-retail.zbf"))
                    .expect("validated retail glyphs"),
                bitmap_font::unpack(include_bytes!("../../assets/font0-32-0-retail.zbf"))
                    .expect("validated retail glyphs"),
            ]
        });
        for (settings, glyphs) in strikes {
            let sw = if settings.width == 0 {
                settings.height
            } else {
                settings.width
            } as f64;
            if glyph_from(glyphs, c).is_ok()
                && ((w == sw && h == settings.height as f64) || settings.width == 0)
            {
                return (
                    GlyphSet::Captured(glyphs),
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
// Read compact pixels directly; do not expand the shared pool into heap-backed strikes.
#[derive(Clone, Copy)]
enum GlyphSet {
    Captured(&'static [Glyph]),
    Compact(&'static zpl_bitmap_fonts::Font, bool, u8),
}
impl From<&'static Vec<Glyph>> for GlyphSet {
    fn from(glyphs: &'static Vec<Glyph>) -> Self {
        Self::Captured(glyphs)
    }
}
#[derive(Clone, Copy)]
enum Pixels {
    Captured(&'static Glyph),
    Compact(zpl_bitmap_fonts::Glyph),
}
struct GlyphView {
    advance: u32,
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    pixels: Pixels,
}
impl GlyphView {
    // Select storage once per glyph and scan packed bytes by runs. Skip
    // uniform bits together instead of dispatching and tracking span state
    // at every pixel, including for the retained scalable strikes.
    fn for_each_span(&self, mut emit: impl FnMut(usize, usize, usize)) {
        fn row_spans(bits: &[u8], offset: usize, width: usize, mut emit: impl FnMut(usize, usize)) {
            let mut x = 0;
            let mut start = None;
            while x < width {
                let bit = offset + x;
                let byte = bits[bit / 8] << (bit % 8);
                let remaining = (8 - bit % 8).min(width - x);
                if byte & 128 != 0 {
                    start.get_or_insert(x);
                    x += (byte.leading_ones() as usize).min(remaining);
                } else {
                    if let Some(left) = start.take() {
                        emit(left, x);
                    }
                    x += (byte.leading_zeros() as usize).min(remaining);
                }
            }
            if let Some(left) = start {
                emit(left, width);
            }
        }
        let width = self.width as usize;
        match self.pixels {
            Pixels::Captured(g) => {
                for (y, row) in g.bitmap.iter().enumerate() {
                    row_spans(row, 0, width, |left, right| emit(y, left, right));
                }
            }
            Pixels::Compact(g) => {
                for y in 0..self.height as usize {
                    row_spans(g.bitmap(), g.row_offset(y), width, |left, right| {
                        emit(y, left, right)
                    });
                }
            }
        }
    }
    #[cfg(test)]
    fn pixel(&self, x: usize, y: usize) -> bool {
        if x >= self.width as usize || y >= self.height as usize {
            return false;
        }
        match self.pixels {
            Pixels::Captured(g) => g.bitmap[y][x / 8] & (128 >> (x % 8)) != 0,
            Pixels::Compact(g) => g.pixel(x as u16, y as u16),
        }
    }
}
fn resident(id: char) -> Option<&'static zpl_bitmap_fonts::Font> {
    zpl_bitmap_fonts::resident(id)
}
fn compact_glyph(
    c: char,
    face: &zpl_bitmap_fonts::Font,
    legacy_backslash: bool,
    encoding: u8,
) -> Option<zpl_bitmap_fonts::Glyph> {
    use zpl_bitmap_fonts::collection::Encoding;
    // Explicit ^CI remapping preserves the printer source position through layout.
    if (0xf0000..=0xf00ff).contains(&(c as u32)) {
        return face.glyph((c as u32 - 0xf0000) as u8);
    }
    if (0xf0100..=0xf01ff).contains(&(c as u32)) {
        let byte = (c as u32 - 0xf0100) as u8;
        return face
            .encoded_glyph(Encoding::Input { ci: encoding }, u32::from(byte))
            .or_else(|| face.glyph(byte).filter(|g| g.width == 0 && g.height == 0));
    }
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
fn glyph_from(glyphs: impl Into<GlyphSet>, c: char) -> Result<GlyphView, String> {
    let missing = || format!("unsupported embedded font glyph {c:?}");
    match glyphs.into() {
        GlyphSet::Captured(glyphs) => {
            let index = glyphs
                .binary_search_by_key(&(c as u32), |g| g.codepoint)
                .map_err(|_| missing())?;
            let g = &glyphs[index];
            Ok(GlyphView {
                advance: g.advance,
                left: g.left,
                top: g.top,
                width: g.width,
                height: g.height,
                pixels: Pixels::Captured(g),
            })
        }
        GlyphSet::Compact(face, legacy, encoding) => {
            let g = compact_glyph(c, face, legacy, encoding).ok_or_else(missing)?;
            Ok(GlyphView {
                advance: u32::from(g.advance),
                left: i32::from(g.left),
                top: i32::from(g.top),
                width: u32::from(g.width),
                height: u32::from(g.height),
                pixels: Pixels::Compact(g),
            })
        }
    }
}
#[cfg(test)]
fn glyph(c: char) -> Result<&'static Glyph, String> {
    let glyphs = &strike().1;
    glyphs
        .binary_search_by_key(&(c as u32), |g| g.codepoint)
        .map(|i| &glyphs[i])
        .map_err(|_| "missing glyph".into())
}
#[cfg(test)]
fn baseline(h: f64) -> f64 {
    baseline_for('0', h)
}
pub(super) fn baseline_for(id: impl Into<Font> + Copy, h: f64) -> f64 {
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
#[cfg(test)]
fn width(s: &str, w: f64) -> Result<f64, String> {
    width_for('0', s, w, 32.)
}
// tabs-zd621-v1: TAB advances to the next 80-dot stop relative
// to the field or line origin, independently of the selected font matrix.
fn next_tab(pen: f64) -> f64 {
    (pen / 80.).floor().mul_add(80., 80.)
}
pub(super) fn width_for(
    id: impl Into<Font> + Copy,
    s: &str,
    w: f64,
    h: f64,
) -> Result<f64, String> {
    let font = id.into();
    s.chars().try_fold(0., |sum, c| {
        if font.is_tab(c) {
            return Ok(next_tab(sum));
        }
        let gap = font.block_flow.map_or(0., |flow| flow.gap_for(c));
        let c = font.map_char(c)?;
        let (glyphs, sx, _) = selected_for_char(font, c, w, h);
        Ok(sum + glyph_from(glyphs, c)?.advance as f64 * sx + gap)
    })
}
/// ZD621 font-0 FO/I/right anchor: measure ink with a backwards pen.
/// The glyphs themselves are still printed in their normal field order.
/// See field-direction-zd621-v1 pair controls (notably jW versus Wj),
/// and Zebra Programming Guide ^FO p. 201 / Field Interactions pp. 1606–1611.
pub(super) fn inverted_text_margin(
    font: Font,
    value: &str,
    w: f64,
    h: f64,
    gap: f64,
) -> Result<f64, String> {
    let mut pen = 0.;
    let mut right = 0_f64;
    let mut first_advance = 0.;
    for (i, c) in value.chars().enumerate() {
        if font.is_tab(c) {
            let next = next_tab(pen);
            if i == 0 {
                first_advance = next;
            }
            pen = next;
            continue;
        }
        let c = font.map_char(c)?;
        let (glyphs, sx, _) = selected_for_char(font, c, w, h);
        let g = glyph_from(glyphs, c)?;
        if i == 0 {
            first_advance = g.advance as f64 * sx;
        }
        if g.width != 0 && g.height != 0 {
            right = right.max((g.left as f64 + g.width as f64) * sx - pen);
        }
        pen += g.advance as f64 * sx + gap;
    }
    Ok(first_advance - right)
}

pub(super) fn inverted_margin(
    id: impl Into<Font> + Copy,
    value: &str,
    w: f64,
    h: f64,
) -> Result<f64, String> {
    let font = id.into();
    let id = font.id;
    if id == 'A' {
        return Ok(w / 5. + 2.);
    }
    if id == 'B' {
        return Ok(2. * w / 7. + 2.);
    }
    if id == 'E' {
        // resident-e-zd621-v1: OCR-B's inverted margin is six native dots,
        // although its ordinary advance includes only five gap dots.
        return Ok(6. * w / 15. + 2.);
    }
    if id == 'H' {
        return Ok(6. * w / 13. + 2.);
    }
    if id == 'G' {
        return Ok(8. * w / 40. + 2.);
    }
    if id == 'F' {
        return Ok(3. * w / 13. + 2.);
    }
    if matches!(id, 'C' | 'D') {
        return Ok(w / 5. + 2.);
    }
    let Some(c) = value.chars().last() else {
        return Ok(0.);
    };
    if font.is_tab(c) {
        return Ok(0.);
    }
    let c = font.map_char(c)?;
    let (glyphs, sx, _) = selected_for_char(font, c, w, h);
    let g = glyph_from(glyphs, c)?;
    Ok(((g.advance as f64 - g.left as f64 - g.width as f64) * sx - 1.).max(0.))
}
#[cfg(test)]
fn text(s: &str, w: f64, h: f64) -> Result<Path, String> {
    text_for('0', s, w, h)
}
// Preserve individual glyph ink for printer edge placement. The regular
// text path remains merged, so unclamped output does not change.
pub(super) fn text_parts_for(
    id: impl Into<Font> + Copy,
    s: &str,
    w: f64,
    h: f64,
) -> Result<Vec<Path>, String> {
    let font = id.into();
    if let Some(flow) = font.block_flow {
        let plain = Font {
            block_flow: None,
            ..font
        };
        let (mut x, mut y) = (0., 0.);
        let mut parts = Vec::new();
        for (i, c) in s.chars().enumerate() {
            let value = c.to_string();
            let advance = width_for(plain, &value, w, h)? + flow.gap_for(c);
            if flow.direction == b'R' && (flow.printer_layout || i != 0) {
                x -= advance;
            }
            let mut path = text_for(plain, &value, w, h)?;
            path.transform(|p| crate::output::Point::new(p.x + x, p.y + y));
            if !path.segments.is_empty() {
                parts.push(path);
            }
            match flow.direction {
                b'H' => x += advance,
                b'V' if !flow.printer_layout => y += h + flow.vertical_gap,
                _ => {}
            }
        }
        return Ok(parts);
    }
    let mut parts = Vec::new();
    let mut pen = 0.;
    let baseline = baseline_for(font, h);
    for c in s.chars() {
        if font.is_tab(c) {
            pen = next_tab(pen);
            continue;
        }
        let c = font.map_char(c)?;
        let (glyphs, sx, sy) = selected_for_char(font, c, w, h);
        let g = glyph_from(glyphs, c)?;
        // A single bitmap glyph already has disjoint row spans. Unlike a
        // complete proportional string, it needs no BTreeMap or union/sort
        // pass to prevent even-odd cancellation of overlapping glyph ink.
        let mut path = RowPath::default();
        g.for_each_span(|y, left, right| {
            path.rect(
                (g.left as f64 + left as f64) * sx,
                baseline + (g.top + y as i32) as f64 * sy,
                (right - left) as f64 * sx,
                sy,
            );
        });
        let mut path = path.into_path();
        path.transform(|p| crate::output::Point::new(p.x + pen, p.y));
        pen += g.advance as f64 * sx;
        if !path.segments.is_empty() {
            parts.push(path);
        }
    }
    Ok(parts)
}
pub(super) fn text_for(
    id: impl Into<Font> + Copy,
    s: &str,
    w: f64,
    h: f64,
) -> Result<Path, String> {
    let font = id.into();
    if font.block_flow.is_some() {
        // ^FB pp. 186–187 wraps by advances including the ^FP gap (p. 202).
        // Captured printer V fields overprint each line; R predecrements even
        // the first character. Union preserves overlapping glyph ink.
        let mut path = Path::default();
        for part in text_parts_for(font, s, w, h)? {
            path.segments.extend(part.segments);
        }
        return Ok(union_lines(path));
    }
    let (mut glyphs, mut sx, mut sy) = selected(font, w, h);
    if font.id == '0' {
        for c in s.chars().filter(|&c| !font.is_tab(c)) {
            let c = font.map_char(c)?;
            if glyph_from(glyphs, c).is_err() {
                if s.chars().count() == 1 {
                    (glyphs, sx, sy) = selected_for_char(font, c, w, h);
                } else {
                    // Mixed strikes have different bitmap units. Compose in
                    // output coordinates, using each glyph's own advances.
                    let mut path = Path::default();
                    for part in text_parts_for(font, s, w, h)? {
                        path.segments.extend(part.segments);
                    }
                    return Ok(union_lines(path));
                }
            }
        }
    }
    // Merge ink spans before emitting even-odd subpaths. Proportional glyphs can
    // overhang their advance; overlapping strokes must remain black, not XOR.
    let mut rows: BTreeMap<i32, Vec<(f64, f64)>> = BTreeMap::new();
    let mut pen = 0.;
    let baseline = baseline_for(font, h);
    // Cap the estimate: a long field may contain mostly advancing blanks.
    let row_capacity = (s.chars().count() * 2).min(64);
    for c in s.chars() {
        if font.is_tab(c) {
            pen = next_tab(pen * sx) / sx;
            continue;
        }
        let g = glyph_from(glyphs, font.map_char(c)?)?;
        g.for_each_span(|y, left, right| {
            rows.entry(g.top + y as i32)
                .or_insert_with(|| Vec::with_capacity(row_capacity))
                .push((
                    pen + g.left as f64 + left as f64,
                    pen + g.left as f64 + right as f64,
                ));
        });
        pen += g.advance as f64;
    }
    let mut path = RowPath::default();
    for (y, mut spans) in rows {
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Option<(f64, f64)> = None;
        for (a, b) in spans {
            if let Some((left, right)) = merged {
                if a <= right {
                    merged = Some((left, right.max(b)));
                } else {
                    path.rect(left * sx, baseline + y as f64 * sy, (right - left) * sx, sy);
                    merged = Some((a, b));
                }
            } else {
                merged = Some((a, b));
            }
        }
        if let Some((left, right)) = merged {
            path.rect(left * sx, baseline + y as f64 * sy, (right - left) * sx, sy);
        }
    }
    Ok(path.into_path())
}
/// Union rectangular ink from multiple lines. Descenders and rounded glyph
/// overshoots can touch the next line even with zero line spacing.
pub(super) fn union_lines(path: Path) -> Path {
    use crate::output::Segment;
    let mut rectangles = Vec::new();
    let mut events = Vec::new();
    for segments in path.segments.as_chunks::<5>().0 {
        let [Segment::Move(a), Segment::Line(b), Segment::Line(c), Segment::Line(_), Segment::Close] =
            segments
        else {
            unreachable!("font ink consists of rectangles")
        };
        let index = rectangles.len();
        rectangles.push((a.x, b.x));
        events.push((a.y, index, true));
        events.push((c.y, index, false));
    }
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut active = BTreeMap::new();
    let mut output = Path::default();
    let mut i = 0;
    while i < events.len() {
        let y = events[i].0;
        while i < events.len() && events[i].0 == y {
            let (_, id, add) = events[i];
            if add {
                active.insert(id, rectangles[id]);
            } else {
                active.remove(&id);
            }
            i += 1;
        }
        let Some(&(next_y, _, _)) = events.get(i) else {
            break;
        };
        let mut spans: Vec<_> = active.values().copied().collect();
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Option<(f64, f64)> = None;
        for (left, right) in spans {
            if let Some((a, b)) = merged {
                if left <= b {
                    merged = Some((a, b.max(right)))
                } else {
                    output.rect(a, y, b - a, next_y - y);
                    merged = Some((left, right));
                }
            } else {
                merged = Some((left, right));
            }
        }
        if let Some((a, b)) = merged {
            output.rect(a, y, b - a, next_y - y);
        }
    }
    output
}

/// Captured bitmap-font FT dot placement. Native glyph metrics describe the
/// cell; rotated FT anchors include a final dot boundary. See resident-bc-zd621-v1
/// and the subsequent resident-font suites (through resident-h-zd621-v1),
/// with ^FT p. 205
/// Table 7 in the Programming Guide.
pub(super) fn printer_ft_offset(id: char, height: f64, rotation: u8) -> (f64, f64) {
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

/// Discard wholly invisible text rectangles before applying the document path
/// budget. Long hanging-indent continuations can extend far beyond the label;
/// partially visible rectangles keep their original geometry and clipping.
pub(super) fn cull_outside(path: &mut Path, width: u32, height: u32) {
    use crate::output::Segment;
    let mut visible = 0;
    for start in (0..path.segments.len()).step_by(5) {
        let [Segment::Move(a), Segment::Line(_), Segment::Line(c), Segment::Line(_), Segment::Close] =
            &path.segments[start..start + 5]
        else {
            unreachable!("font ink consists of rectangles")
        };
        // Quarter-turn text rotations preserve axis-aligned rectangles. A/C
        // are opposite corners, including inverted and bottom-up placement.
        let (left, right) = (a.x.min(c.x), a.x.max(c.x));
        let (top, bottom) = (a.y.min(c.y), a.y.max(c.y));
        if right > 0. && bottom > 0. && left < width as f64 && top < height as f64 {
            if start != visible {
                for offset in 0..5 {
                    path.segments.swap(visible + offset, start + offset);
                }
            }
            visible += 5;
        }
    }
    path.segments.truncate(visible);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_span_scans_preserve_every_glyph_pixel() {
        let check = |g: GlyphView| {
            let mut pixels = vec![vec![false; g.width as usize]; g.height as usize];
            g.for_each_span(|y, left, right| {
                assert!(left < right);
                for pixel in &mut pixels[y][left..right] {
                    assert!(!*pixel, "spans must not overlap");
                    *pixel = true;
                }
            });
            for (y, row) in pixels.iter().enumerate() {
                for (x, &pixel) in row.iter().enumerate() {
                    assert_eq!(pixel, g.pixel(x, y), "pixel {x},{y}");
                }
            }
        };
        for id in ['A', 'B', 'D', 'E', 'F', 'G', 'H', GRAPHIC_SYMBOLS] {
            let face = resident(id).unwrap();
            for key in 0..=255 {
                let c = char::from_u32(0xf0000 + key).unwrap();
                if let Ok(g) = glyph_from(GlyphSet::Compact(face, false, 28), c) {
                    check(g);
                }
            }
        }
        for id in ['0', 'P', 'Q', 'R', 'S', 'T', 'U', 'V'] {
            for size in [18., 28., 32., 40., 64.] {
                let (set, _, _) = selected(id, size, size);
                for c in ' '..='~' {
                    check(glyph_from(set, c).unwrap());
                }
            }
        }
    }

    #[test]
    fn measured_input_maps_render_characters_outside_cp850() {
        // ZD621 measured CI27/28 maps include these inputs, which the old
        // Unicode-to-CP850 source lookup could not resolve.
        use zpl_bitmap_fonts::collection::Encoding;
        let face = resident('A').unwrap();
        for (character, byte) in [('Œ', 0x8c), ('…', 0x85), ('™', 0x99), ('€', 0x80)] {
            assert!(!CP850.contains(&character));
            let expected = face
                .encoded_glyph(Encoding::Input { ci: 27 }, byte)
                .unwrap();
            for ci in [27, 28] {
                let font = Font::new('A', false).with_encoding(ci);
                let (set, _, _) = selected(font, 5., 9.);
                let actual = glyph_from(set, font.map_char(character).unwrap()).unwrap();
                assert_eq!(
                    (
                        actual.advance,
                        actual.left,
                        actual.top,
                        actual.width,
                        actual.height
                    ),
                    (
                        u32::from(expected.advance),
                        i32::from(expected.left),
                        i32::from(expected.top),
                        u32::from(expected.width),
                        u32::from(expected.height)
                    )
                );
                for y in 0..actual.height {
                    for x in 0..actual.width {
                        assert_eq!(
                            actual.pixel(x as usize, y as usize),
                            expected.pixel(x as u16, y as u16)
                        );
                    }
                }
            }
        }
        assert!(compact_glyph('漢', face, false, 28).is_none());
    }

    #[test]
    fn resident_bitmap_faces_read_shared_pool_and_preserve_source_mapping() {
        for id in ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', GRAPHIC_SYMBOLS] {
            let native = resident(id).unwrap();
            let metrics = native.cell_metrics().unwrap();
            let font = Font::new(id, false);
            let (set, sx, sy) = selected(
                font,
                f64::from(metrics.cell_width),
                f64::from(metrics.cell_height),
            );
            assert!(matches!(set, GlyphSet::Compact(..)));
            assert_eq!((sx, sy), (1., 1.));
            assert_eq!(
                baseline_for(font, f64::from(metrics.cell_height)),
                f64::from(metrics.baseline) - 1.
            );
            for (c, key) in [
                ('A', 65),
                ('¢', 189),
                ('é', 130),
                ('\u{ad}', 240),
                ('ð', 208),
            ] {
                let g = glyph_from(set, c).unwrap();
                let expected = native.glyph(key).unwrap();
                assert_eq!(g.advance, u32::from(expected.advance));
                for y in 0..g.height as usize {
                    for x in 0..g.width as usize {
                        assert_eq!(g.pixel(x, y), expected.pixel(x as u16, y as u16));
                    }
                }
            }
        }
        let mut map = std::array::from_fn(|i| i as u8);
        map[usize::from(b'A')] = b'\\';
        let f = Font::new('A', false).with_character_map(Some(map));
        let (set, _, _) = selected(f, 5., 9.);
        let remapped = glyph_from(set, f.map_char('A').unwrap()).unwrap();
        let legacy = resident('A').unwrap().glyph(92).unwrap();
        assert_eq!(
            (remapped.width, remapped.height),
            (u32::from(legacy.width), u32::from(legacy.height))
        );
        assert_eq!(
            compact_glyph('\\', resident('A').unwrap(), false, 28)
                .unwrap()
                .width,
            resident('A').unwrap().glyph(31).unwrap().width
        );
        assert_eq!(
            compact_glyph('\\', resident('A').unwrap(), true, 28)
                .unwrap()
                .width,
            resident('A').unwrap().glyph(92).unwrap().width
        );
        assert!(matches!(selected('0', 32., 32.).0, GlyphSet::Captured(..)));
        assert!(matches!(selected('P', 18., 20.).0, GlyphSet::Captured(..)));
    }

    #[test]
    fn joined_rows_preserve_exact_edges_and_even_odd_pixels() {
        use crate::output::{raster::rasterize, Draw, Paint, Scene};
        // Output-independent even-odd path contract: docs/local-renderer.md.
        // Test exact joins, gaps, changing widths, and fractional rounding.
        for scale in [0.1, 0.5, 1., 1.1, 1.5, 2.3] {
            let mut rows = RowPath::default();
            let mut original = Path::default();
            for y in 0..12 {
                for (x, width) in [(1., 2.), (5., if y % 3 == 0 { 2. } else { 3. })] {
                    if y == 7 {
                        continue;
                    }
                    let args = (x * scale, y as f64 * scale, width * scale, scale);
                    rows.rect(args.0, args.1, args.2, args.3);
                    original.rect(args.0, args.1, args.2, args.3);
                }
            }
            let rows = rows.into_path();
            if scale == 1. {
                assert!(rows.segments.len() < original.segments.len() / 2);
            }
            for paint in [Paint::Black, Paint::White, Paint::Invert] {
                let mut background = Path::default();
                background.rect(0., 3., 20., 12.);
                let mut scene = Scene::new(32, 32, 203).unwrap();
                scene.draws.push(Draw {
                    path: background,
                    paint: Paint::Black,
                });
                scene.draws.push(Draw {
                    path: original.clone(),
                    paint,
                });
                let expected = rasterize(&scene).unwrap();
                scene.draws[1].path = rows.clone();
                assert_eq!(
                    rasterize(&scene).unwrap(),
                    expected,
                    "scale={scale}, paint={paint:?}"
                );
            }
        }
        let mut path = RowPath::default();
        path.rect(0., 0., 1., 1.);
        path.rect(0., 1f64.next_up(), 1., 1.);
        assert_eq!(
            path.into_path().segments.len(),
            10,
            "do not fill even a subpixel gap"
        );
    }

    #[test]
    fn individual_parts_match_full_glyph_layout() {
        // Preserve captured strike geometry and advances, including scaled
        // fallbacks and remapped controls (Zebra ^CI p. 158, Tables 29/31).
        for id in [
            '0', 'A', 'B', 'D', 'E', 'F', 'G', 'H', 'P', 'Q', 'R', 'S', 'T', 'U', 'V',
        ] {
            for (w, h) in [(10., 16.), (16., 24.), (32., 32.), (17.3, 29.7)] {
                for font in [
                    Font::new(id, false),
                    Font::new(id, true)
                        .with_default_glyph(true)
                        .with_control_glyphs(true, false),
                ] {
                    for c in (' '..='~').chain(['¢', 'é', 'א', '\u{1b}', '\u{7f}']) {
                        let value = c.to_string();
                        let expected = text_for(font, &value, w, h);
                        let actual = text_parts_for(font, &value, w, h);
                        match (expected, actual) {
                            (Ok(expected), Ok(parts)) => {
                                let actual = Path {
                                    segments: parts.into_iter().flat_map(|p| p.segments).collect(),
                                };
                                assert_eq!(actual, expected, "{id} {c:?} {w}x{h}");
                            }
                            (Err(expected), Err(actual)) => assert_eq!(actual, expected),
                            other => panic!("glyph layout disagrees: {other:?}"),
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn ascii_strikes_preserve_enriched_fallback_glyphs() {
        // ^CI / ^PA and legacy control behavior: use the same enriched
        // 32-dot base face and scaling available before these ASCII captures.
        for (h, w) in [
            (18., 22.),
            (20., 24.),
            (22., 26.),
            (23., 23.),
            (26., 30.),
            (28., 32.),
            (28., 28.),
            (30., 34.),
            (39., 42.),
            (45., 44.),
            (72., 68.),
        ] {
            for font in [
                Font::new('0', false),
                Font::new('0', true),
                Font::new('0', false).with_default_glyph(true),
                Font::new('0', true).with_default_glyph(true),
                Font::new('0', true).with_control_glyphs(true, false),
                Font::new('0', true)
                    .with_default_glyph(true)
                    .with_control_glyphs(true, false),
            ] {
                let mut characters = vec!['¢', 'é', 'ð', '\u{378}', 'א', 'ب'];
                if font.legacy_backslash {
                    characters.push('\\');
                }
                if font.legacy_controls {
                    characters.extend(['\u{1b}', '\u{7f}']);
                }
                for c in characters {
                    let value = c.to_string();
                    let mut expected = text_for(font, &value, 32., 32.).unwrap();
                    expected.transform(|p| crate::output::Point::new(p.x * w / 32., p.y * h / 32.));
                    let actual = text_for(font, &value, w, h).unwrap();
                    assert_eq!(actual, expected, "{c:?} at {h}x{w}");
                    assert_eq!(
                        width_for(font, &value, w, h).unwrap(),
                        width_for(font, &value, 32., 32.).unwrap() * w / 32.
                    );
                }
                // ASCII remains native, even in a field containing fallback
                // glyphs; width and ink composition agree across both scales.
                let value = "A¢Wé";
                let expected_width: f64 = value
                    .chars()
                    .map(|c| width_for(font, &c.to_string(), w, h).unwrap())
                    .sum();
                assert_eq!(width_for(font, value, w, h).unwrap(), expected_width);
                let mut expected = Path::default();
                for part in text_parts_for(font, value, w, h).unwrap() {
                    expected.segments.extend(part.segments);
                }
                assert_eq!(text_for(font, value, w, h).unwrap(), union_lines(expected));
            }
        }
    }
    #[test]
    fn embedded_strike_is_complete_and_compact() {
        let (s, g) = strike();
        assert_eq!((s.font, s.height, s.width, s.dpi), ('0', 32, 0, 203));
        assert_eq!(
            g.iter().map(|g| g.codepoint).collect::<Vec<_>>(),
            (32..=126)
                .chain([162, 173, 233, 240])
                .chain([0x378])
                .chain(0x5d0..=0x5ea)
                .chain([0x627, 0x628, 0x62d, 0x631, 0x645])
                .collect::<Vec<_>>()
        );
        assert!(DATA.len() < 4500);
        assert_eq!(width("Wi i", 32.).unwrap(), 51.);
        assert_eq!(glyph('é').unwrap().codepoint, 233);
    }
    #[test]
    fn lowercase_and_baselines() {
        assert_ne!(text("a", 32., 32.).unwrap(), text("A", 32., 32.).unwrap());
        assert_eq!(
            glyph('j').unwrap().top + glyph('j').unwrap().height as i32,
            6
        );
        assert_eq!(baseline(32.), 24.);
    }
}

#[derive(Clone, Default)]
pub(super) struct DirectionMetrics {
    pub pivot: Option<f64>,
    pub end_margin: f64,
    pub bottom_margin: f64,
    pub end_left: f64,
    pub first_delta: f64,
    pub first_ink: (f64, f64),
    pub count: usize,
    pub vertical_extent: (f64, f64),
    pub leading_descent: f64,
    pub leading_top: f64,
}
pub(super) struct DirectedText {
    pub path: Path,
    pub parts: Vec<Path>,
    pub size: (f64, f64),
    pub metrics: DirectionMetrics,
}
/// ^FP, Zebra Programming Guide p. 202 and Field Interactions pp. 1606–1611.
pub(super) fn directed_text(
    font: Font,
    value: &str,
    w: f64,
    h: f64,
    direction: (u8, f64),
    compatibility: super::compatibility::Compatibility,
    right_justified: bool,
) -> Result<DirectedText, String> {
    let (d, gap) = direction;
    let mut parts = Vec::new();
    let mut path = Path::default();
    let (mut x, mut y) = (0., 0.);
    let mut width = 0_f64;
    let mut tab_pen = 0.;
    let mut end_left = 0_f64;
    let mut first_advance = 0_f64;
    let mut first_ink = (0_f64, 0_f64);
    let mut leading_top = f64::INFINITY;
    let mut leading_descent = 0.;
    let mut max_advance = 0_f64;
    let mut max_right = 0_f64;
    let (mut end_margin, mut bottom) = (0_f64, 0_f64);
    let last_advance = value
        .chars()
        .last()
        .map(|c| width_for(font, &c.to_string(), w, h))
        .transpose()?
        .unwrap_or(0.)
        + gap;
    for (index, c) in value.chars().enumerate() {
        let text = c.to_string();
        let advance = if font.is_tab(c) && d != b'V' {
            next_tab(tab_pen) - tab_pen
        } else {
            width_for(font, &text, w, h)? + gap
        };
        tab_pen += advance;
        max_advance = max_advance.max(advance);
        let mut part = text_for(font, &text, w, h)?;
        if d == b'R' && index != 0 {
            x -= advance;
        }
        if d == b'V' && right_justified {
            x = last_advance - advance;
        }
        let mut right = 0_f64;
        let mut left = f64::INFINITY;
        if index == 0 {
            first_advance = advance;
        }
        for segment in &part.segments {
            if let crate::output::Segment::Move(p) | crate::output::Segment::Line(p) = segment {
                left = left.min(p.x);
                right = right.max(p.x);
                bottom = bottom.max(p.y);
                if index == 0 {
                    leading_top = leading_top.min(p.y);
                    first_ink.0 = first_ink.0.max(p.x);
                    first_ink.1 = first_ink.1.max(p.y);
                }
            }
        }
        max_right = max_right.max(right);
        end_margin = advance - right;
        end_left = if left.is_finite() { left } else { 0. };
        part.transform(|p| crate::output::Point::new(p.x + x, p.y + y));
        path.segments.extend(part.segments.iter().cloned());
        parts.push(part);
        width = if d == b'V' { last_advance } else { x + advance };
        if d == b'H' {
            x += advance;
        }
        if d == b'V' {
            y += h + if compatibility.field_vertical_ignores_gap {
                0.
            } else {
                gap
            };
        }
    }
    if d == b'V' && compatibility.field_direction_printer_anchors {
        let capital = text_for(font, "H", w, h)?;
        let capital_bottom = capital
            .segments
            .iter()
            .filter_map(|s| match s {
                crate::output::Segment::Move(p) | crate::output::Segment::Line(p) => Some(p.y),
                _ => None,
            })
            .fold(0_f64, f64::max);
        leading_descent = (first_ink.1 - capital_bottom).max(0.);
        y += leading_descent;
    }
    Ok(DirectedText {
        path: union_lines(path),
        parts,
        size: (width, if d == b'V' { y } else { h }),
        metrics: DirectionMetrics {
            pivot: (d != b'H').then_some(w - if font.id == '0' { 1. } else { 0. }),
            end_left,
            first_ink,
            count: value.chars().count(),
            vertical_extent: (max_right, max_advance - last_advance),
            leading_descent,
            leading_top: if leading_top.is_finite() {
                leading_top
            } else {
                0.
            },
            first_delta: first_advance - last_advance,
            end_margin,
            bottom_margin: h - bottom,
        },
    })
}

/// Return line pitch and baseline adjustment.
///
/// Captured S block pitch and ascent: native FO/FT controls at 40 and 80 dots
/// in resident-s-zd621-v1. ^FB pp. 186–188 describes nominal font-height
/// spacing; this printer uses distinct preset metrics. Other sizes scale the
/// closest measured height, just as unsampled glyph strikes are approximated.
pub(super) fn block_metrics(id: impl Into<Font>, h: f64, printer_s: bool) -> (f64, f64) {
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

// ^CI0/^CI13: Zebra CP850. Unicode mapping reference:
// https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/PC/CP850.TXT
pub(super) fn legacy_char(byte: u8) -> char {
    if byte.is_ascii() {
        char::from(byte)
    } else {
        CP850[byte as usize - 128]
    }
}

const CP850: [char; 128] = [
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
pub(super) fn code_page(ci: u8) -> Option<&'static encoding_rs::Encoding> {
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
