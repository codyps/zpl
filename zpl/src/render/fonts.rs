//! Caller-supplied fonts, scoped to a render call; no filesystem or printer access.
use crate::{
    bitmap_font::{self, Glyph, Settings},
    output::raster::truetype::{rasterize, ScanMode},
    truetype::{Environment, Font, Hinting, Size},
};
use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

/// Fonts assigned to ZPL IDs (`0`–`9`, `A`–`Z`, or `@` for `^GS`) or
/// virtual printer filenames selected through `^CW` and `^A@`.
/// Unregistered IDs retain the renderer's resident-font behavior. Registered
/// faces replace the entire face: missing characters return an error rather
/// than silently mixing the caller's font with a resident font.
///
/// ```no_run
/// use zpl::render::{fonts::Fonts, profiles::SPECIFICATION, render_with_fonts};
/// use zpl::truetype::Hinting;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let bytes = std::fs::read("my-font.ttf")?;
/// let mut fonts = Fonts::new();
/// fonts.insert_truetype('Z', &bytes, Hinting::Native)?;
/// let document = render_with_fonts(
///     b"^XA^FO20,20^AZN,32,24^FDHello^FS^XZ", SPECIFICATION, &fonts,
/// )?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Default)]
pub struct Fonts<'a> {
    faces: BTreeMap<char, Arc<Face<'a>>>,
    named: BTreeMap<String, Arc<Face<'a>>>,
}

enum Source<'a> {
    Bitmap(Settings, Vec<Glyph>),
    TrueType(Font<'a>, Hinting),
}
pub(super) struct Face<'a> {
    source: Source<'a>,
    baseline: f64,
    missing_advance: Option<u32>,
}

impl<'a> Fonts<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Assign a decoded bitmap strike. `baseline` is the distance from the
    /// cell top to the baseline in native strike dots; glyph tops are relative
    /// to that baseline. Glyphs may be unsorted, but duplicates are rejected.
    /// The assignment ID is independent of the strike's resident-font tag.
    pub fn insert_bitmap(
        &mut self,
        id: char,
        settings: Settings,
        glyphs: Vec<Glyph>,
        baseline: f64,
    ) -> Result<(), String> {
        validate_id(id)?;
        self.faces
            .insert(id, Arc::new(Face::bitmap(settings, glyphs, baseline)?));
        Ok(())
    }

    /// Decode and assign a ZBF1/ZBF2 strike with an explicit native baseline.
    pub fn insert_zbf(&mut self, id: char, data: &[u8], baseline: f64) -> Result<(), String> {
        let (settings, glyphs) = bitmap_font::unpack(data)?;
        self.insert_bitmap(id, settings, glyphs, baseline)
    }

    /// Assign a TrueType/OpenType font with quadratic `glyf` outlines.
    /// CFF/CFF2 outlines, collections and unsupported hint instructions return
    /// errors through the existing TrueType engine. Data must outlive this set.
    /// Sizes are dots per em, rounded to whole dots (1..=4096); width and height
    /// scale independently. Uses standard hinting/scan semantics, independently
    /// of the resident printer compatibility profile. No kerning or shaping is
    /// added beyond the renderer's existing Unicode processing.
    pub fn insert_truetype(
        &mut self,
        id: char,
        data: &'a [u8],
        hinting: Hinting,
    ) -> Result<(), String> {
        validate_id(id)?;
        self.faces
            .insert(id, Arc::new(Face::truetype(data, hinting)?));
        Ok(())
    }

    /// Register a bitmap face under a virtual printer filename for `^CW`/`^A@`.
    /// Names are case-insensitive; an omitted device means `R:`. This never
    /// reads a host file. Supported devices are R/E/B/A, extensions FNT/TTF/TTE/OTF/DAT,
    /// and basenames contain 1–255 ASCII letters, digits, underscores or hyphens.
    /// See [`Self::insert_bitmap`] for bitmap metrics.
    pub fn insert_named_bitmap(
        &mut self,
        name: &str,
        settings: Settings,
        glyphs: Vec<Glyph>,
        baseline: f64,
    ) -> Result<(), String> {
        let name = font_name(name)?;
        let face = Face::bitmap(settings, glyphs, baseline)?;
        self.named.insert(name, Arc::new(face));
        Ok(())
    }

    /// Register a ZBF1/ZBF2 strike under a virtual printer filename.
    pub fn insert_named_zbf(
        &mut self,
        name: &str,
        data: &[u8],
        baseline: f64,
    ) -> Result<(), String> {
        let (settings, glyphs) = bitmap_font::unpack(data)?;
        self.insert_named_bitmap(name, settings, glyphs, baseline)
    }

    /// Register a TrueType face under a virtual printer filename.
    /// See [`Self::insert_truetype`] for format and hinting limits, and
    /// [`Self::insert_named_bitmap`] for filename rules. Unknown names in ZPL
    /// return errors; rendering never opens files automatically.
    ///
    /// ```no_run
    /// # use zpl::render::{fonts::Fonts, profiles::SPECIFICATION, render_with_fonts};
    /// # use zpl::truetype::Hinting;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let bytes = std::fs::read("brand.ttf")?;
    /// let mut fonts = Fonts::new();
    /// fonts.insert_named_truetype("R:BRAND.TTF", &bytes, Hinting::Native)?;
    /// let document = render_with_fonts(
    ///     b"^CWZ,R:BRAND.TTF^XA^FO20,20^AZN,32,24^FDHello^FS^XZ",
    ///     SPECIFICATION, &fonts,
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn insert_named_truetype(
        &mut self,
        name: &str,
        data: &'a [u8],
        hinting: Hinting,
    ) -> Result<(), String> {
        let name = font_name(name)?;
        let face = Face::truetype(data, hinting)?;
        self.named.insert(name, Arc::new(face));
        Ok(())
    }

    fn checked_name(&self, name: &str) -> Result<String, String> {
        let name = font_name(name)?;
        if !self.named.contains_key(&name) {
            return Err(format!("unresolved named font {name:?}"));
        }
        Ok(name)
    }
}

/// Request-local ZPL selections, separate from caller-owned font resources.
/// ^CW retains a filename, not a snapshot of the face stored under that name.
pub(super) struct RenderFonts<'r, 'a> {
    resources: &'r Fonts<'a>,
    aliases: BTreeMap<char, String>,
    selected_name: Option<String>,
    downloaded: BTreeMap<String, Face<'a>>,
}

impl<'r, 'a> RenderFonts<'r, 'a> {
    pub(super) fn new(resources: &'r Fonts<'a>) -> Self {
        Self {
            resources,
            aliases: BTreeMap::new(),
            selected_name: None,
            downloaded: BTreeMap::new(),
        }
    }

    fn checked_name(&self, name: &str) -> Result<String, String> {
        let normalized = font_name(name)?;
        if self.downloaded.contains_key(&normalized) {
            Ok(normalized)
        } else {
            self.resources.checked_name(name)
        }
    }

    pub(super) fn install(
        &mut self,
        download: &'a super::font_downloads::Download,
    ) -> Result<(), String> {
        use super::font_downloads::Download;
        let (name, face) = match download {
            Download::TrueType { name, data } => (name, Face::truetype(data, Hinting::Native)?),
            Download::Bitmap {
                name,
                settings,
                glyphs,
                baseline,
                space,
            } => {
                let mut face = Face::bitmap_metrics(*settings, glyphs.clone(), *baseline)?;
                face.missing_advance = Some(*space);
                (name, face)
            }
        };
        self.downloaded.insert(name.clone(), face);
        Ok(())
    }

    pub(super) fn alias(&mut self, id: char, name: &str) -> Result<(), String> {
        let name = self.checked_name(name)?;
        self.aliases.insert(id, name);
        Ok(())
    }

    pub(super) fn select_named(&mut self, name: &str) -> Result<(), String> {
        self.selected_name = Some(self.checked_name(name)?);
        Ok(())
    }

    pub(super) fn has_named_selection(&self) -> bool {
        self.selected_name.is_some()
    }

    pub(super) fn get(&self, id: char) -> Option<&Face<'a>> {
        let name = if id == NAMED_FONT {
            self.selected_name.as_ref()
        } else {
            self.aliases.get(&id)
        };
        match name {
            Some(name) => self
                .downloaded
                .get(name)
                .or_else(|| self.resources.named.get(name).map(Arc::as_ref)),
            None => self.resources.faces.get(&id).map(Arc::as_ref),
        }
    }
}

// Virtual filenames only: Zebra Programming Guide ^CW p. 168 / ^A@ p. 62.
// https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
// Restrict lookup to explicit font extensions on supported drives.
pub(super) fn font_name(name: &str) -> Result<String, String> {
    let name = name.trim().to_ascii_uppercase();
    let (device, file) = name.split_once(':').unwrap_or(("R", &name));
    if !matches!(device, "R" | "E" | "B" | "A") {
        return Err("unsupported named font device".into());
    }
    let (base, extension) = file
        .rsplit_once('.')
        .ok_or("named font requires FNT, TTF, TTE, OTF or DAT extension")?;
    if base.is_empty()
        || base.len() > 255
        || !base
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        || !matches!(extension, "FNT" | "TTF" | "TTE" | "OTF" | "DAT")
    {
        return Err("invalid named font filename or extension".into());
    }
    Ok(format!("{device}:{file}"))
}

// A direct ^A@ selection has its own slot, distinct from ^GS and all public IDs.
pub(super) const NAMED_FONT: char = '\0';

impl<'a> Face<'a> {
    fn bitmap(settings: Settings, glyphs: Vec<Glyph>, baseline: f64) -> Result<Self, String> {
        settings.validate()?;
        Self::bitmap_metrics(settings, glyphs, baseline)
    }
    fn bitmap_metrics(
        settings: Settings,
        mut glyphs: Vec<Glyph>,
        baseline: f64,
    ) -> Result<Self, String> {
        bitmap_font::validate_glyphs(&glyphs)?;
        if glyphs.is_empty()
            || !baseline.is_finite()
            || !(0. ..=settings.height as f64).contains(&baseline)
        {
            return Err("invalid bitmap font baseline or empty glyph set".into());
        }
        glyphs.sort_by_key(|g| g.codepoint);
        if glyphs
            .windows(2)
            .any(|pair| pair[0].codepoint == pair[1].codepoint)
        {
            return Err("duplicate bitmap font codepoint".into());
        }
        Ok(Self {
            baseline: baseline / settings.height as f64,
            source: Source::Bitmap(settings, glyphs),
            missing_advance: None,
        })
    }
    fn truetype(data: &'a [u8], hinting: Hinting) -> Result<Self, String> {
        let font = Font::parse(data).map_err(|e| e.to_string())?;
        // OpenType hhea ascender (offset 4), in design units:
        // https://learn.microsoft.com/en-us/typography/opentype/spec/hhea
        let hhea = font.table(b"hhea").ok_or("missing TrueType hhea")?;
        let ascent = i16::from_be_bytes(
            hhea.get(4..6)
                .ok_or("truncated TrueType hhea")?
                .try_into()
                .unwrap(),
        );
        let baseline = f64::from(ascent) / f64::from(font.units_per_em());
        Ok(Self {
            source: Source::TrueType(font, hinting),
            missing_advance: None,
            baseline,
        })
    }
}

fn validate_id(id: char) -> Result<(), String> {
    if id.is_ascii_uppercase() || id.is_ascii_digit() || id == bitmap_font::GRAPHIC_SYMBOLS {
        Ok(())
    } else {
        Err("font ID must be 0–9, A–Z, or @".into())
    }
}

impl Face<'_> {
    pub(super) fn baseline(&self, h: f64) -> f64 {
        self.baseline
            * match self.source {
                Source::TrueType(..) => h.round(),
                _ => h,
            }
    }

    pub(super) fn dimensions(&self, w: f64, h: f64) -> Result<(f64, f64), String> {
        let (nw, nh) = match &self.source {
            Source::Bitmap(s, _) => (
                if s.width == 0 { s.height } else { s.width } as f64,
                s.height as f64,
            ),
            Source::TrueType(..) => (1., 1.),
        };
        let (w, h) = match (w, h) {
            (0., 0.) => (nw, nh),
            (0., h) => (h * nw / nh, h),
            (w, 0.) => (w, w * nh / nw),
            pair => pair,
        };
        if !w.is_finite() || !h.is_finite() || w <= 0. || h <= 0. {
            return Err("font dimensions must be positive".into());
        }
        if matches!(self.source, Source::TrueType(..))
            && (!(1. ..=4096.).contains(&w.round()) || !(1. ..=4096.).contains(&h.round()))
        {
            return Err("TrueType size must be in 1..=4096 dots per em".into());
        }
        Ok((w, h))
    }

    pub(super) fn glyph(
        &self,
        c: char,
        w: f64,
        h: f64,
    ) -> Result<(Cow<'_, Glyph>, f64, f64), String> {
        self.dimensions(w, h)?;
        match &self.source {
            Source::Bitmap(s, glyphs) => {
                let width = if s.width == 0 { s.height } else { s.width };
                let index = match glyphs.binary_search_by_key(&(c as u32), |g| g.codepoint) {
                    Ok(index) => index,
                    Err(_) => {
                        let advance = self
                            .missing_advance
                            .ok_or_else(|| format!("unsupported custom font glyph {c:?}"))?;
                        return Ok((
                            Cow::Owned(Glyph {
                                codepoint: c as u32,
                                advance,
                                left: 0,
                                top: 0,
                                width: 0,
                                height: 0,
                                bitmap: vec![],
                            }),
                            w / width as f64,
                            h / s.height as f64,
                        ));
                    }
                };
                Ok((
                    Cow::Borrowed(&glyphs[index]),
                    w / width as f64,
                    h / s.height as f64,
                ))
            }
            Source::TrueType(font, hinting) => {
                let index = font
                    .glyph_index(c)
                    .ok_or_else(|| format!("unsupported custom font glyph {c:?}"))?;
                let size =
                    Size::new(w.round() as u16, h.round() as u16).map_err(|e| e.to_string())?;
                let instance = font
                    .instance(size, *hinting, Environment::Standard)
                    .map_err(|e| e.to_string())?;
                let outline = instance.outline(index).map_err(|e| e.to_string())?;
                let advance = instance.layout_advance(index).map_err(|e| e.to_string())?;
                let glyph = rasterize(&outline, c as u32, advance, 0, ScanMode::Center)
                    .map_err(|e| e.to_string())?;
                Ok((Cow::Owned(glyph), 1., 1.))
            }
        }
    }
}
