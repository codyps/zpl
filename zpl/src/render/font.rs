//! Embedded, captured resident fonts. Bitmap pixels become output-neutral paths.
//! Metrics: ZPL Programming Guide Tables 29/31, pp. 1582–1583:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use crate::{
    bitmap_font::{self, Glyph, Settings, GRAPHIC_SYMBOLS},
    output::Path,
};
use std::{collections::BTreeMap, sync::OnceLock};
/// Resident face plus the legacy ASCII backslash replacement. Keep this
/// separate from U+00A2: bitmap faces have distinct native cent designs.
#[derive(Clone, Copy)]
pub(super) struct Font {
    id: char,
    legacy_backslash: bool,
    default_glyph: bool,
    character_map: Option<[u8; 256]>,
    block_flow: Option<BlockFlow>,
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
            default_glyph: false,
            character_map: None,
            block_flow: None,
        }
    }
    pub(super) fn with_character_map(mut self, map: Option<[u8; 256]>) -> Self {
        self.character_map = map;
        self
    }
    fn map_char(self, c: char) -> Result<char, String> {
        let Some(map) = self.character_map else {
            return Ok(c);
        };
        let Some(&source) = map.get(c as usize) else {
            return Ok(c);
        };
        if u32::from(source) == c as u32 {
            return Ok(c);
        }
        // ^CI example, Programming Guide p. 158: legacy image 21 is euro.
        match source {
            21 => Ok('€'),
            0..=127 => Ok(char::from(source)),
            _ => Err("unsupported legacy remap source glyph".into()),
        }
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
        ('A', 9, 5) => include_bytes!("../../assets/fontA-9-5-cent.zbf"),
        ('B', 11, 7) => include_bytes!("../../assets/fontB-11-7-cent.zbf"),
        ('D', 18, 10) => include_bytes!("../../assets/fontD-18-10-cent.zbf"),
        ('E', 28, 15) => include_bytes!("../../assets/fontE-28-15-cent.zbf"),
        ('F', 26, 13) => include_bytes!("../../assets/fontF-26-13-cent.zbf"),
        ('G', 60, 40) => include_bytes!("../../assets/fontG-60-40-cent.zbf"),
        ('H', 21, 13) => include_bytes!("../../assets/fontH-21-13-cent.zbf"),
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
        ('A', 9, 5) => include_bytes!("../../assets/fontA-9-5-hyphen.zbf"),
        ('0', 32, 0) => include_bytes!("../../assets/font0-32-0-hyphen.zbf"),
        ('0', 16, 0) => include_bytes!("../../assets/font0-16-0-hyphen.zbf"),
        ('0', 20, 0) => include_bytes!("../../assets/font0-20-0-hyphen.zbf"),
        ('0', 64, 0) => include_bytes!("../../assets/font0-64-0-hyphen.zbf"),
        ('0', 32, 16) => include_bytes!("../../assets/font0-32-16-hyphen.zbf"),
        ('0', 32, 24) => include_bytes!("../../assets/font0-32-24-hyphen.zbf"),
        ('0', 32, 64) => include_bytes!("../../assets/font0-32-64-hyphen.zbf"),
        ('B', 11, 7) => include_bytes!("../../assets/fontB-11-7-hyphen.zbf"),
        ('D', 18, 10) => include_bytes!("../../assets/fontD-18-10-hyphen.zbf"),
        ('E', 28, 15) => include_bytes!("../../assets/fontE-28-15-hyphen.zbf"),
        ('F', 26, 13) => include_bytes!("../../assets/fontF-26-13-hyphen.zbf"),
        ('G', 60, 40) => include_bytes!("../../assets/fontG-60-40-hyphen.zbf"),
        ('H', 21, 13) => include_bytes!("../../assets/fontH-21-13-hyphen.zbf"),
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
            include_bytes!("../../assets/fontA-9-5.zbf").as_slice(),
            include_bytes!("../../assets/fontB-11-7.zbf").as_slice(),
            include_bytes!("../../assets/fontD-18-10.zbf").as_slice(),
            include_bytes!("../../assets/fontF-26-13.zbf").as_slice(),
            include_bytes!("../../assets/fontG-60-40.zbf").as_slice(),
            include_bytes!("../../assets/fontH-21-13.zbf").as_slice(),
            include_bytes!("../../assets/fontGS-24-24.zbf").as_slice(),
            include_bytes!("../../assets/fontS-40-35.zbf").as_slice(),
            include_bytes!("../../assets/fontS-80-70.zbf").as_slice(),
            include_bytes!("../../assets/fontE-28-15.zbf").as_slice(),
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
                ('A', 9, 5) => Some(include_bytes!(
                    "../../assets/fontA-9-5-legacy-backslash.zbf"
                )),
                ('B', 11, 7) => Some(include_bytes!(
                    "../../assets/fontB-11-7-legacy-backslash.zbf"
                )),
                ('D', 18, 10) => Some(include_bytes!(
                    "../../assets/fontD-18-10-legacy-backslash.zbf"
                )),
                ('E', 28, 15) => Some(include_bytes!(
                    "../../assets/fontE-28-15-legacy-backslash.zbf"
                )),
                ('F', 26, 13) => Some(include_bytes!(
                    "../../assets/fontF-26-13-legacy-backslash.zbf"
                )),
                ('G', 60, 40) => Some(include_bytes!(
                    "../../assets/fontG-60-40-legacy-backslash.zbf"
                )),
                ('H', 21, 13) => Some(include_bytes!(
                    "../../assets/fontH-21-13-legacy-backslash.zbf"
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
fn selected(id: impl Into<Font> + Copy, w: f64, h: f64) -> (&'static [Glyph], f64, f64) {
    // C and D share the 18x10 matrix (ZPL Programming Guide Table 31,
    // p. 1583); resident-bc-zd621-v1 verifies the alias across all ASCII.
    let font = id.into();
    let id = if font.id == 'C' { 'D' } else { font.id };
    let faces = if font.default_glyph {
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
            return (glyphs, w / sw, h / s.height as f64);
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
        return (glyphs, w / s.width as f64, h / s.height as f64);
    }
    let fallback = if font.default_glyph {
        &default_glyph_strikes(font.legacy_backslash)
            .last()
            .unwrap()
            .1
    } else if font.legacy_backslash {
        &legacy_strikes().last().unwrap().1
    } else {
        &strike().1
    };
    (fallback, w / 32., h / 32.)
}
fn glyph_from(glyphs: &'static [Glyph], c: char) -> Result<&'static Glyph, String> {
    let index = glyphs
        .binary_search_by_key(&(c as u32), |g| g.codepoint)
        .map_err(|_| format!("unsupported embedded font glyph {c:?}"))?;
    Ok(&glyphs[index])
}
#[cfg(test)]
fn glyph(c: char) -> Result<&'static Glyph, String> {
    glyph_from(&strike().1, c)
}
#[cfg(test)]
fn baseline(h: f64) -> f64 {
    baseline_for('0', h)
}
pub(super) fn baseline_for(id: impl Into<Font> + Copy, h: f64) -> f64 {
    // Zebra guide p. 1582: one-based baselines 7 (A), 11 (B), 14 (C/D),
    // 23 (E), 21 (F), 48 (G), 21 (H). These zero-based offsets locate native glyph ink in its cell.
    h * match id.into().id {
        'A' => 6. / 9.,
        'B' => 10. / 11.,
        'C' | 'D' => 13. / 18.,
        'E' => 22. / 28.,
        'F' => 20. / 26.,
        'G' => 47. / 60.,
        'H' => 20. / 21.,
        // Native Q/R FO-to-FT controls locate these face baselines; Table 29
        // omits the presets (their matrices are in Table 31, p. 1584).
        'Q' => 22. / 28.,
        'R' => 28. / 35.,
        // Independent native FO/FT atlas: resident-tuv-zd621-v1.
        'T' => 36. / 48.,
        'U' => 46. / 59.,
        'V' => 62. / 80.,
        // Captured GS metrics use baseline 23; the specification FT anchor
        // is selected separately by graphic_symbol_last_row_baseline.
        GRAPHIC_SYMBOLS => 23. / 24.,
        _ => 0.75,
    }
}
#[cfg(test)]
fn width(s: &str, w: f64) -> Result<f64, String> {
    width_for('0', s, w, 32.)
}
pub(super) fn width_for(
    id: impl Into<Font> + Copy,
    s: &str,
    w: f64,
    h: f64,
) -> Result<f64, String> {
    let font = id.into();
    let (glyphs, sx, _) = selected(font, w, h);
    s.chars().try_fold(0., |sum, c| {
        let gap = font.block_flow.map_or(0., |flow| flow.gap_for(c));
        Ok(sum + glyph_from(glyphs, font.map_char(c)?)?.advance as f64 * sx + gap)
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
    let (glyphs, sx, _) = selected(font, w, h);
    let mut pen = 0.;
    let mut right = 0_f64;
    let mut first_advance = 0.;
    for (i, c) in value.chars().enumerate() {
        let g = glyph_from(glyphs, font.map_char(c)?)?;
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
    let (glyphs, sx, _) = selected(font, w, h);
    let g = glyph_from(glyphs, font.map_char(c)?)?;
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
    for c in s.chars() {
        let value = c.to_string();
        let mut path = text_for(id, &value, w, h)?;
        path.transform(|p| crate::output::Point::new(p.x + pen, p.y));
        pen += width_for(id, &value, w, h)?;
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
    let (glyphs, sx, sy) = selected(font, w, h);
    // Merge ink spans before emitting even-odd subpaths. Proportional glyphs can
    // overhang their advance; overlapping strokes must remain black, not XOR.
    let mut rows: BTreeMap<i32, Vec<(f64, f64)>> = BTreeMap::new();
    let mut pen = 0.;
    for c in s.chars() {
        let g = glyph_from(glyphs, font.map_char(c)?)?;
        for (y, row) in g.bitmap.iter().enumerate() {
            let mut start = None;
            for x in 0..=g.width as usize {
                let black = x < g.width as usize && row[x / 8] & (128 >> (x % 8)) != 0;
                match (start, black) {
                    (None, true) => start = Some(x),
                    (Some(a), false) => {
                        rows.entry(g.top + y as i32).or_default().push((
                            pen + g.left as f64 + a as f64,
                            pen + g.left as f64 + x as f64,
                        ));
                        start = None
                    }
                    _ => {}
                }
            }
        }
        pen += g.advance as f64;
    }
    let mut path = Path::default();
    for (y, mut spans) in rows {
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Option<(f64, f64)> = None;
        for (a, b) in spans {
            if let Some((left, right)) = merged {
                if a <= right {
                    merged = Some((left, right.max(b)));
                } else {
                    path.rect(
                        left * sx,
                        baseline_for(id, h) + y as f64 * sy,
                        (right - left) * sx,
                        sy,
                    );
                    merged = Some((a, b));
                }
            } else {
                merged = Some((a, b));
            }
        }
        if let Some((left, right)) = merged {
            path.rect(
                left * sx,
                baseline_for(id, h) + y as f64 * sy,
                (right - left) * sx,
                sy,
            );
        }
    }
    Ok(path)
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
    let mut visible = Vec::new();
    for rect in path.segments.as_chunks::<5>().0 {
        let [Segment::Move(a), Segment::Line(b), Segment::Line(c), Segment::Line(d), Segment::Close] =
            rect
        else {
            unreachable!("font ink consists of rectangles")
        };
        let points = [a, b, c, d];
        let left = points.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
        let right = points.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
        let top = points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
        let bottom = points.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
        if right > 0. && bottom > 0. && left < width as f64 && top < height as f64 {
            visible.extend_from_slice(rect);
        }
    }
    path.segments = visible;
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let advance = width_for(font, &text, w, h)? + gap;
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
