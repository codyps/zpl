use super::barcode_edges::PartBoundary;
use crate::output::Path;
mod aztec;
mod aztec_text;
mod bits;
mod codabar;
mod codablock;
mod codablock_a;
mod code11;
pub(super) mod code128;
pub(super) mod code39;
mod code49;
mod code49_patterns;
mod code93;
mod composite;
mod composite_a;
mod composite_b;
mod composite_c;
#[cfg(test)]
mod composite_tests;
mod data_matrix;
mod data_matrix_legacy;
mod data_matrix_text;
mod databar;
mod databar_expanded;
mod databar_limited;
mod databar_stacked;
mod ean13;
mod ean8;
mod gs1_128;
mod gs1_compaction;
mod industrial2of5;
mod intelligent_mail;
mod interleaved2of5;
mod logmars;
mod maxicode;
mod maxicode_modules;
mod micropdf417;
mod msi;
mod multi_origin;
mod pdf417;
mod pdf417_patterns;
mod planet;
mod plessey;
mod postal;
mod postnet;
mod qr;
mod qr_mask;
mod qr_model1;
mod reed_solomon;
mod retail;
mod standard2of5;
mod tlc39;
mod two_of_five;
mod upc_extension;
mod upca;
mod upce;

use crate::output::Point;

#[derive(Clone)]
pub(super) struct Barcode {
    name: String,
    params: Vec<String>,
    pub show: bool,
    above: bool,
    pub height: f64,
    module: f64,
    ratio: f64,
    dpi: u32,
    compatibility: super::compatibility::Compatibility,
}
pub(super) fn supported(name: &str) -> bool {
    matches!(
        name,
        "B0" | "B1"
            | "B2"
            | "B3"
            | "B4"
            | "B5"
            | "B7"
            | "B8"
            | "B9"
            | "BA"
            | "BB"
            | "BC"
            | "BD"
            | "BE"
            | "BF"
            | "BI"
            | "BJ"
            | "BK"
            | "BL"
            | "BM"
            | "BO"
            | "BP"
            | "BQ"
            | "BR"
            | "BS"
            | "BT"
            | "BU"
            | "BX"
            | "BZ"
    )
}
#[derive(Clone)]
pub(super) struct CaptionPart {
    pub start: usize,
    pub end: usize,
    pub origin: Point,
}
pub(super) struct Rendered {
    pub path: Path,
    pub width: f64,
    pub baseline: f64,
    pub split: Vec<PartBoundary>,
    pub caption_parts: Vec<CaptionPart>,
}

impl Barcode {
    fn uses_retail_caption(&self) -> bool {
        self.compatibility.retail_interpretation_printer_layout
            && self.dpi == 203
            && self.show
            && !self.above
            && matches!(self.name.as_str(), "B8" | "B9" | "BE" | "BU")
    }

    pub fn field_origin_width(&self, width: f64, bar_width: f64) -> f64 {
        if self.name == "BC" && self.compatibility.code128_fo_uses_bar_width {
            // ^FO p. 201; resident-f-zd621-v1 includes captions wider than bars.
            return bar_width;
        }
        if self.uses_retail_caption() {
            // Outer interpretation digits do not move the ^FO rotation pivot.
            self.module
                * match self.name.as_str() {
                    "B8" => 67.,
                    "B9" => 51.,
                    _ => 95.,
                }
        } else {
            width
        }
    }
    fn postal_pitch(&self) -> f64 {
        if self.compatibility.postal_fixed_pitch {
            (self.module * 2.5).floor()
        } else {
            self.module * (1. + self.ratio)
        }
    }
    pub fn multiple_origin_offset(&self, rotation: u8) -> (f64, f64) {
        if self.name == "BF"
            && self
                .compatibility
                .macro_micropdf417_reverse_origin_omits_side_raps
        {
            match rotation {
                b'I' => (-20. * self.module, 0.),
                b'B' => (0., -20. * self.module),
                _ => (0., 0.),
            }
        } else {
            (0., 0.)
        }
    }
    pub fn uses_multiple_origins(&self) -> bool {
        matches!(self.name.as_str(), "B7" | "BF")
    }
    pub fn render_multiple(&self, bytes: &[u8], count: usize) -> Result<Vec<Path>, String> {
        multi_origin::render(self, bytes, count)
    }
    pub fn field_origin_y(&self) -> f64 {
        // ZD621 V93.21.33Z controls with ^BY heights 40/60/100 place ^FO
        // QR ink at y + height - 1. This does not apply to ^FT. See the
        // captured controls and reproduction notes in docs/printer-accuracy.md.
        if self.name == "BQ" && self.compatibility.qr_fo_uses_by_height {
            self.height - 1.
        } else {
            0.
        }
    }
    pub fn field_baseline_height(&self, height: f64, rotation: u8) -> f64 {
        // ^FT p. 205, Table 7; linear-ft-zd621-v1 and Code 39/retail controls
        // include the last bar row for N/B. Keep full height for ^FO pivots.
        let linear = matches!(
            self.name.as_str(),
            "B1" | "B2"
                | "B3"
                | "B5"
                | "B8"
                | "B9"
                | "BA"
                | "BC"
                | "BE"
                | "BI"
                | "BJ"
                | "BK"
                | "BL"
                | "BM"
                | "BP"
                | "BS"
                | "BU"
        ) || (self.name == "BZ"
            && self.integer(4, 0, 0, 3).is_ok_and(|kind| kind <= 1));
        if linear
            && self.compatibility.linear_barcode_ft_uses_last_bar_row
            && matches!(rotation, b'N' | b'B')
        {
            (height - 1.).max(0.)
        } else {
            height
        }
    }

    pub fn new(
        name: &str,
        p: &[&str],
        module: f64,
        ratio: f64,
        height: f64,
        dpi: u32,
        compatibility: super::compatibility::Compatibility,
    ) -> Result<Self, String> {
        let max = match name {
            "BD" | "BF" | "BL" => 3,
            "B5" | "B8" | "BE" | "BI" | "BJ" | "BS" | "B4" => 4,
            "B1" | "B3" | "B9" | "BA" | "BP" | "BQ" | "BU" | "BZ" => 5,
            "B2" | "B7" | "BB" | "BC" | "BM" | "BR" | "BT" => 6,
            "BK" | "BO" | "B0" => 7,
            "BX" => 8,
            _ => return Err("unknown barcode".into()),
        };
        if p.len() > max {
            return Err(format!("{name}: unexpected command parameters"));
        }
        let mut b = Self {
            name: name.into(),
            params: p.iter().map(|s| (*s).into()).collect(),
            show: false,
            above: false,
            height,
            module,
            ratio,
            dpi,
            compatibility,
        };
        let layout = match name {
            "B1" | "B3" | "BK" | "BM" | "BP" => Some((2, 3, 4)),
            "BC" | "B2" | "B5" | "B8" | "B9" | "BA" | "BE" | "BI" | "BJ" | "BS" | "BU" | "BZ" => {
                Some((1, 2, 3))
            }
            _ => None,
        };
        if let Some((h, f, g)) = layout {
            b.height = b.num(h, height, 1., 32000.)?;
            b.show = b.flag(f, name != "BZ")?;
            b.above = b.flag(g, false)?;
        }
        if name == "B3" {
            b.flag(1, false)?;
        }
        if name == "BL" {
            b.height = b.num(1, height, 1., 32000.)?;
            b.show = true;
            b.above = b.flag(2, false)?;
        }
        if name == "B4" {
            b.height = b.num(1, height, 1., 32000.)?;
            b.show = b.param(2, "N") != "N";
            b.above = b.param(2, "N") == "A";
        }
        Ok(b)
    }
    fn param<'a>(&'a self, i: usize, default: &'a str) -> &'a str {
        self.params
            .get(i)
            .filter(|s| !s.is_empty())
            .map(String::as_str)
            .unwrap_or(default)
    }
    fn num(&self, i: usize, default: f64, min: f64, max: f64) -> Result<f64, String> {
        let n = match self.params.get(i).filter(|s| !s.is_empty()) {
            Some(s) => s
                .parse::<f64>()
                .map_err(|_| format!("{}: invalid number", self.name))?,
            None => default,
        };
        if !n.is_finite() || n < min || n > max {
            return Err(format!("{}: parameter {} out of range", self.name, i + 1));
        }
        Ok(n)
    }
    fn integer(&self, i: usize, default: usize, min: usize, max: usize) -> Result<usize, String> {
        let n = self.num(i, default as f64, min as f64, max as f64)?;
        if n.fract() != 0. {
            return Err("barcode parameter must be an integer".into());
        }
        Ok(n as usize)
    }
    fn flag(&self, i: usize, default: bool) -> Result<bool, String> {
        match self.param(i, if default { "Y" } else { "N" }) {
            "Y" => Ok(true),
            "N" => Ok(false),
            _ => Err(format!("{}: invalid Y/N flag", self.name)),
        }
    }
    fn require(&self, i: usize, default: &str, allowed: &[&str]) -> Result<(), String> {
        if allowed.contains(&self.param(i, default)) {
            Ok(())
        } else {
            Err(format!(
                "{}: parameter {} mode unsupported",
                self.name,
                i + 1
            ))
        }
    }
    pub(super) fn module_after_command(&self) -> Result<f64, String> {
        // ^BQ pp. 129–134 describes QR magnification, not a ^BY state change.
        // Native same/next-field and reset controls: qr-module-state-zd621-v1.
        if self.name == "BQ" && self.compatibility.qr_updates_barcode_module_width {
            self.num(2, self.scale(), 1., 100.)
        } else {
            Ok(self.module)
        }
    }
    fn scale(&self) -> f64 {
        match self.dpi {
            0..=150 => 1.,
            151..=250 => 2.,
            251..=450 => 3.,
            _ => 6.,
        }
    }
    fn linear(&self, modules: &[bool], wide: Option<f64>) -> Result<Path, String> {
        let mut path = Path::default();
        let mut x = 0.;
        let mut i = 0;
        while i < modules.len() {
            let start = i;
            let black = modules[i];
            while i < modules.len() && modules[i] == black {
                i += 1;
            }
            let count = i - start;
            let width = self.module * wide.filter(|_| count > 1).unwrap_or(count as f64);
            if black {
                path.rect(x, 0., width, self.height);
            }
            x += width;
        }
        Ok(path)
    }
    fn matrix(&self, m: &Matrix, x: f64, y: f64) -> Result<Path, String> {
        let mut p = Path::default();
        for row in 0..m.h {
            let mut col = 0;
            while col < m.w {
                if !m.get(col, row) {
                    col += 1;
                    continue;
                }
                let start = col;
                while col < m.w && m.get(col, row) {
                    col += 1;
                }
                p.rect(
                    start as f64 * x,
                    row as f64 * y,
                    (col - start) as f64 * x,
                    y,
                );
            }
        }
        Ok(p)
    }
    pub fn uses_ean_validation(&self) -> bool {
        matches!(self.name.as_str(), "B8" | "BE")
    }
    pub fn validate_retail_data(&self, bytes: &[u8]) -> Result<(), String> {
        let n = match self.name.as_str() {
            "B8" if self.compatibility.retail_printer_overlong_data && bytes.len() > 8 => 13,
            "B8" => 8,
            "BE" => 13,
            "BU" => 12,
            _ => return Ok(()),
        };
        // ^CV validates the original data before the ordinary retail input
        // padding/coercion. Native paired CVN/CVY controls retain S/C/E/L labels.
        if bytes.is_empty() {
            return Err("empty barcode data".into());
        }
        retail::checked(bytes, n).map(|_| ())
    }
    pub(super) fn render_with_fonts(
        &self,
        fonts: &super::fonts::RenderFonts<'_, '_, '_>,
        bytes: &[u8],
        font: Option<(char, f64, f64)>,
        rotation: u8,
        character_map: Option<[u8; 256]>,
    ) -> Result<Rendered, String> {
        let normalized;
        let bytes = if self.name == "B3" && self.compatibility.code39_normalize_input {
            normalized = code39::normalize(bytes);
            normalized.as_slice()
        } else if let Some(n) = match self.name.as_str() {
            "B8" => Some(8),
            "BE" => Some(13),
            "BU" => Some(12),
            _ => None,
        } {
            normalized = retail::normalize(bytes, n, self.compatibility)?;
            normalized.as_slice()
        } else {
            bytes
        };
        if bytes.is_empty() && self.name != "B3" {
            return Err(format!("{}: empty barcode data", self.name));
        }
        let mut p = match self.name.as_str() {
            "B0" | "BO" => aztec::render(self, bytes),
            "B1" => code11::render(self, bytes),
            "B2" => interleaved2of5::render(self, bytes),
            "B3" => code39::standalone(self, bytes),
            "B4" => code49::render(self, bytes),
            "B5" => planet::render(self, bytes),
            "B7" => pdf417::render(self, bytes),
            "B8" => ean8::render(self, bytes),
            "B9" => upce::render(self, bytes),
            "BA" => code93::render(self, bytes),
            "BB" => codablock::render(self, bytes),
            "BC" => code128::render(self, bytes),
            "BD" => maxicode::render(self, bytes),
            "BE" => ean13::render(self, bytes),
            "BF" => micropdf417::render(self, bytes),
            "BI" => industrial2of5::render(self, bytes),
            "BJ" => standard2of5::render(self, bytes),
            "BK" => codabar::render(self, bytes),
            "BL" => logmars::render(self, bytes),
            "BM" => msi::render(self, bytes),
            "BP" => plessey::render(self, bytes),
            "BQ" => qr::render(self, bytes),
            "BR" => databar::render(self, bytes),
            "BS" => upc_extension::render(self, bytes),
            "BT" => tlc39::render(self, bytes),
            "BU" => upca::render(self, bytes),
            "BX" => data_matrix::render(self, bytes),
            "BZ" => postal::render(self, bytes),
            _ => unreachable!(),
        }?;
        let (_, _, width, mut height) = super::bounds(&p);
        if matches!(self.name.as_str(), "B8" | "B9" | "BE" | "BU") {
            // Guard extensions descend into the interpretation-line area.
            height = self.height;
        }
        let mut baseline = height;
        if self.name == "BD" && self.compatibility.maxicode_printer_dot_geometry {
            baseline = 199. * self.dpi as f64 / 203.;
        }
        if self.name == "BQ" && self.compatibility.qr_ft_includes_margin {
            // ^FT includes a three-module lower margin, less one dot. Verified
            // with ZD621 magnifications 1–5 and ^BY heights 40/100; unlike ^FO,
            // this anchor is independent of the default linear barcode height.
            baseline += 3. * self.num(2, self.scale(), 1., 100.)? - 1.;
        }
        let edge_family = matches!(self.name.as_str(), "B1" | "B2" | "B3" | "BA" | "BC");
        let clamp = self.compatibility.linear_barcode_clamps_negative_ink && edge_family;
        let track_bars =
            edge_family && (clamp || self.compatibility.linear_barcode_rotated_edge_loses_dot);
        let mut split = if track_bars {
            vec![
                PartBoundary::new(0, 0.),
                PartBoundary::new(p.segments.len(), 0.),
            ]
        } else {
            Vec::new()
        };
        let mut caption_parts = Vec::new();
        if self.uses_retail_caption() {
            let (caption, mut parts) = retail::caption(self, bytes, rotation, character_map)?;
            let offset = p.segments.len();
            for part in &mut parts {
                part.start += offset;
                part.end += offset;
            }
            caption_parts = parts;
            p.segments.extend(caption.segments);
        } else if self.show {
            let special = if self.name == "BA" {
                code93::extended_caption(self, bytes)?
            } else {
                None
            };
            let value = special.as_ref().map_or_else(
                || self.interpretation(bytes),
                |caption| Ok(caption.text.clone()),
            )?;
            let font = if (self.name == "B3"
                && self.compatibility.code39_interpretation_ignores_font)
                || (matches!(self.name.as_str(), "B1" | "B2" | "B5" | "BA" | "BK")
                    && self.compatibility.linear_interpretation_ignores_font)
            {
                None
            } else {
                font
            };
            // ^BC p. 94 permits an explicit preceding font command. Without
            // one, resident A scales with ^BY, independently of ^CF.
            let (id, fw, fh) = font.unwrap_or(('A', 5. * self.module, 9. * self.module));
            let caption_font = super::font::Font::from(id)
                .with_fonts(fonts)
                .with_truetype_environment(
                    self.compatibility.supplied_truetype_printer_metrics,
                    rotation,
                )
                .with_character_map(character_map);
            let cap_caption = font.is_none()
                && self.compatibility.bitmap_font_maximum_dimensions
                && self.module > 10.;
            let (ink_w, ink_h) = if cap_caption { (50., 90.) } else { (fw, fh) };
            let mut t = if clamp {
                let mut text = Path::default();
                let mut x = 0.;
                for c in value.chars() {
                    let value = c.to_string();
                    let mut glyph = super::font::text_for(caption_font, &value, ink_w, ink_h)?;
                    let padding = if id == 'A' {
                        (7. * ink_h / 9. - super::bounds(&glyph).3).max(0.)
                    } else {
                        0.
                    };
                    glyph.transform(|p| Point::new(p.x + x, p.y));
                    text.segments.extend(glyph.segments);
                    split.push(PartBoundary::new(
                        p.segments.len() + text.segments.len(),
                        padding,
                    ));
                    x += super::font::width_for(caption_font, &value, ink_w, ink_h)?;
                }
                text
            } else {
                super::font::text_for(caption_font, &value, ink_w, ink_h)?
            };
            // Captured ^B1/^BA interpretation uses dedicated start/stop
            // glyphs, not the resident font's printable asterisk. Reserve
            // their cells with spaces, then draw the measured native masks.
            let marker: &[u8] = match self.name.as_str() {
                "B1" if self.compatibility.code11_interpretation_symbols => &[4, 10, 17, 31],
                "BA" if self.compatibility.code93_interpretation_symbols && special.is_none() => {
                    &[31, 17, 17, 17, 17, 17, 31]
                }
                _ => &[],
            };
            if !marker.is_empty() {
                let right = super::font::width_for(caption_font, &value, ink_w, ink_h)?
                    - super::font::width_for(caption_font, " ", ink_w, ink_h)?;
                for x in [0., right] {
                    // The two-check Code 11 stop glyph is a taller triangle;
                    // its start glyph and the one-check stop stay four rows.
                    let marker = if self.name == "B1" && x == right && !self.flag(1, false)? {
                        &[4, 4, 10, 10, 17, 17, 31][..]
                    } else {
                        marker
                    };
                    caption_glyph(&mut t, x, ink_w, ink_h, marker);
                    if clamp {
                        split.push(PartBoundary::new(
                            p.segments.len() + t.segments.len(),
                            if id == 'A' {
                                7_usize.saturating_sub(marker.len()) as f64 * ink_h / 9.
                            } else {
                                0.
                            },
                        ));
                    }
                }
            }
            if let Some(caption) = special {
                for (index, glyph) in caption.glyphs {
                    let x = super::font::width_for(caption_font, &value[..index], ink_w, ink_h)?;
                    caption_glyph(&mut t, x, ink_w, ink_h, glyph);
                    if clamp {
                        split.push(PartBoundary::new(
                            p.segments.len() + t.segments.len(),
                            if id == 'A' {
                                7_usize.saturating_sub(glyph.len()) as f64 * ink_h / 9.
                            } else {
                                0.
                            },
                        ));
                    }
                }
            }
            if cap_caption {
                // Native implicit captions cap ink, but retain the requested
                // seven-row ink bottom and centering width; explicit ^A is capped
                // before layout. See bitmap-maximum-zd621-v1.
                t.transform(|p| Point::new(p.x, p.y + 7. * (self.module - 10.)));
            }
            let printer = self.compatibility.barcode_interpretation_printer_layout;
            let gap_below = if printer { 6. } else { 3. };
            let gap_above = if printer { 8. } else { 3. };
            if font.is_none() {
                let mut width = super::bounds(&p).2;
                // ^B5/^BZ: captured interpretation spans complete postal
                // pitches, while the path ends at the final bar's ink edge.
                if self.compatibility.postal_interpretation_full_pitch
                    && (self.name == "B5"
                        || (self.name == "BZ"
                            && self.integer(4, 0, 0, 3).is_ok_and(|kind| kind <= 1)))
                {
                    width += self.postal_pitch() - self.module;
                }
                let advance = super::font::width_for(caption_font, &value, fw, fh)?;
                let x = ((width - advance) / 2.).floor();
                t.transform(|p| Point::new(p.x + x, p.y));
            }
            if self.compatibility.barcode_reverse_interpretation_shift
                && matches!(rotation, b'I' | b'B')
                && (font.is_none()
                    || (self.name == "BC"
                        && matches!(id, 'A' | 'B' | 'C' | 'D' | 'E' | 'F' | 'G' | 'H')))
            {
                // Resident-font controls through resident-h-zd621-v1: bitmap captions
                // use the final-dot boundary; proportional font 0 does not.
                t.transform(|p| Point::new(p.x - 1., p.y));
            }
            let preserve_bar_origin = if self.name == "BC" {
                self.compatibility.code128_above_text_keeps_bar_origin
            } else {
                self.compatibility.barcode_above_text_keeps_bar_origin
            };
            if self.above && preserve_bar_origin {
                t.transform(|p| Point::new(p.x, p.y - fh - gap_above));
            } else if self.above {
                p.transform(|p| Point::new(p.x, p.y + fh + gap_above));
                baseline += fh + gap_above;
            } else {
                t.transform(|p| Point::new(p.x, p.y + height + gap_below));
            }
            p.segments.extend(t.segments);
        }
        Ok(Rendered {
            path: p,
            width,
            baseline,
            split,
            caption_parts,
        })
    }
    fn interpretation(&self, bytes: &[u8]) -> Result<String, String> {
        if self.name == "B1" && self.compatibility.code11_interpretation_symbols {
            let digits: String = code11::checked_values(self, bytes)?
                .into_iter()
                .map(|digit| {
                    if digit == 10 {
                        '-'
                    } else {
                        (b'0' + digit) as char
                    }
                })
                .collect();
            return Ok(format!(" {digits} "));
        }
        if self.name == "BK" && self.compatibility.codabar_interpretation_delimiters {
            return Ok(format!(
                "{}{}{}",
                self.param(5, "A"),
                ascii(bytes)?,
                self.param(6, "A")
            ));
        }
        if self.name == "B3" && self.compatibility.code39_interpretation_symbols {
            let value = ascii(bytes)?;
            let value = if self.flag(1, false)? {
                code39::with_checksum(value)?
            } else {
                value.to_owned()
            };
            return Ok(format!("*{value}*"));
        }
        if self.name == "BC" {
            return code128::interpretation(self, bytes);
        }
        if self.name == "BL" {
            const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%";
            let sum = bytes
                .iter()
                .map(|c| ALPHABET.iter().position(|v| v == c).unwrap())
                .sum::<usize>();
            let mut value = ascii(bytes)?.to_string();
            value.push(ALPHABET[sum % 43] as char);
            return Ok(value);
        }
        if self.name == "BP" {
            return plessey::interpretation(self, bytes);
        }
        if self.name == "BA" {
            let value = code93::interpretation(
                bytes,
                self.compatibility.code93_normalize_input,
                self.flag(4, false)?,
            )?;
            return Ok(if self.compatibility.code93_interpretation_symbols {
                format!(" {value} ")
            } else {
                value
            });
        }
        let mut decimal = match self.name.as_str() {
            "B8" => Some(retail::checked(bytes, 8)?),
            "BE" => Some(retail::checked(bytes, 13)?),
            "BU" => Some(retail::checked(bytes, 12)?),
            "B9" => Some(upce::canonical(bytes)?),
            "BM" if self.flag(5, false)? => Some(msi::checks(bytes, self.param(1, "B"))?),
            _ => None,
        };
        if let Some(ref mut digits) = decimal {
            if matches!(self.name.as_str(), "BU" | "B9") && !self.flag(4, true)? {
                digits.pop();
            }
            return Ok(digits.iter().map(|v| (v + b'0') as char).collect());
        }
        Ok(ascii(bytes)?.to_string())
    }
}
fn caption_glyph(path: &mut Path, x: f64, width: f64, height: f64, rows: &[u8]) {
    for (row, bits) in rows.iter().enumerate() {
        for col in 0..5 {
            if bits & (16 >> col) != 0 {
                path.rect(
                    x + col as f64 * width / 5.,
                    row as f64 * height / 9.,
                    width / 5.,
                    height / 9.,
                );
            }
        }
    }
}
fn ascii(bytes: &[u8]) -> Result<&str, String> {
    std::str::from_utf8(bytes).map_err(|_| "barcode requires ASCII data".into())
}

struct Matrix {
    w: usize,
    h: usize,
    cells: Vec<bool>,
}
impl Matrix {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            cells: vec![false; w * h],
        }
    }
    fn get(&self, x: usize, y: usize) -> bool {
        self.cells[y * self.w + x]
    }
    fn set(&mut self, x: usize, y: usize, v: bool) {
        self.cells[y * self.w + x] = v;
    }
}
fn digits(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() || !data.iter().all(u8::is_ascii_digit) {
        return Err("barcode requires decimal digits".into());
    }
    Ok(data.iter().map(|b| b - b'0').collect())
}
fn mod10(data: &[u8]) -> u8 {
    ((10 - data
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &v)| v as usize * if i % 2 == 0 { 3 } else { 1 })
        .sum::<usize>()
        % 10)
        % 10) as u8
}
fn append_pattern(out: &mut Vec<bool>, pattern: u32, width: usize) {
    out.extend((0..width).rev().map(|i| pattern & (1 << i) != 0));
}
fn runs(out: &mut Vec<bool>, widths: &[usize]) {
    for (i, &n) in widths.iter().enumerate() {
        out.extend(std::iter::repeat_n(i % 2 == 0, n));
    }
}
