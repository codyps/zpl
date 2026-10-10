//! Font resources, encoding, glyph lookup and metrics, scoped to a render call.
//! No scene construction, filesystem access or printer access.
pub(crate) mod resident;
use crate::{
    bitmap_font::{self, Settings},
    output::raster::truetype::{rasterize, ScanMode},
    truetype::{Environment, Font, Hinting, Size},
};
use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

/// Format-independent monochrome glyph data used by [`BitmapFont`].
pub use crate::bitmap_font::Glyph;

/// Native cell dimensions and baseline for a runtime bitmap font, in dots.
#[derive(Debug, Clone, Copy)]
pub struct BitmapMetrics {
    /// Nominal cell width, in 1..=4096. Controls horizontal scaling.
    pub width: u32,
    /// Nominal cell height, in 1..=4096. Controls vertical scaling.
    pub height: u32,
    /// Baseline in native dots, in 0..=height, using the same metrics as ~DB.
    /// Printer profiles interpret it as one-based; SPECIFICATION uses it directly.
    pub baseline: f64,
}

/// A caller-supplied bitmap strike, independent of its file format or storage.
///
/// Implementations may borrow decoded glyphs or return owned glyphs on demand.
/// Glyphs use native dot metrics: `left` and `top` are offsets from the pen's
/// baseline, and `advance` is the horizontal pen movement. Each bitmap row has
/// `width.div_ceil(8)` bytes, most significant bit first; one means foreground.
/// Dimensions, advances, and absolute offsets must not exceed 4096 dots.
///
/// Return `Ok(None)` for a missing character (a rendering error, with no resident
/// fallback), or `Err` for a loading/decoding failure. Returned glyphs must match
/// the requested Unicode codepoint. Metrics are read once at registration;
/// glyph lookup should remain deterministic, including across cloned font sets.
/// The renderer validates each returned glyph before using its bitmap.
/// No filesystem access or ZBF metadata is required by this interface.
///
/// ```
/// use std::{borrow::Cow, sync::Arc};
/// use zpl::fonts::{BitmapFont, BitmapMetrics, Fonts, Glyph};
///
/// struct MyStrike(Vec<Glyph>);
/// impl BitmapFont for MyStrike {
///     fn metrics(&self) -> BitmapMetrics {
///         BitmapMetrics { width: 8, height: 12, baseline: 9. }
///     }
///     fn glyph(&self, c: char) -> Result<Option<Cow<'_, Glyph>>, String> {
///         Ok(self.0.iter().find(|g| g.codepoint == c as u32).map(Cow::Borrowed))
///     }
/// }
/// // Populate these glyphs using your own decoder or in-memory data.
/// let strike = MyStrike(vec![Glyph {
///     codepoint: 'A' as u32, advance: 8, left: 0, top: -3,
///     width: 3, height: 3, bitmap: vec![vec![0x40], vec![0xa0], vec![0xe0]],
/// }]);
/// let mut fonts = Fonts::new();
/// fonts.insert_bitmap_font('Z', Arc::new(strike))?;
/// let document = zpl::render::render_with_fonts(
///     b"^XA^FO20,20^AZN,24,16^FDA^FS^XZ",
///     zpl::render::profiles::SPECIFICATION,
///     &fonts,
/// )?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub trait BitmapFont: Send + Sync {
    fn metrics(&self) -> BitmapMetrics;
    fn glyph(&self, character: char) -> Result<Option<Cow<'_, Glyph>>, String>;
}

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
    resolver: Option<Arc<Resolver<'a>>>,
}

type Resolver<'a> = dyn Fn(&str) -> Result<Option<Face<'a>>, String> + Send + Sync + 'a;

enum Source<'a> {
    Compact(&'static zpl_bitmap_fonts::Font),
    Bitmap(Settings, Arc<Vec<Glyph>>),
    Provider(BitmapMetrics, Arc<dyn BitmapFont + 'a>),
    TrueType(Font<'a>, Hinting),
}
/// A resolved font resource. Construct bitmap or TrueType faces with the methods
/// below, or obtain a measured ROM face from [`resolve_rom_font`].
pub struct Face<'a> {
    source: Source<'a>,
    baseline: f64,
    missing_advance: Option<u32>,
    bitmap_printer_metrics: bool,
}

impl<'a> Fonts<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Assign a format-independent bitmap provider to a ZPL font ID.
    /// The provider may borrow data for `'a`; clones of this set share it.
    /// Invalid metrics leave an existing assignment unchanged.
    pub fn insert_bitmap_font(
        &mut self,
        id: char,
        font: Arc<dyn BitmapFont + 'a>,
    ) -> Result<(), String> {
        validate_id(id)?;
        self.faces.insert(id, Arc::new(Face::provider(font)?));
        Ok(())
    }

    /// Register a bitmap provider under a virtual printer filename.
    /// See [`Self::insert_named_bitmap`] for filename rules.
    pub fn insert_named_bitmap_font(
        &mut self,
        name: &str,
        font: Arc<dyn BitmapFont + 'a>,
    ) -> Result<(), String> {
        let name = font_name(name)?;
        self.named.insert(name, Arc::new(Face::provider(font)?));
        Ok(())
    }

    /// Assign a decoded bitmap strike as an already-installed ~DB resource.
    /// `baseline` is in native strike dots; glyph tops are relative to it.
    /// Printer profiles use one-based baselines and integer magnification;
    /// SPECIFICATION uses the supplied baseline and continuous sizing directly.
    /// Glyphs may be unsorted, but duplicates are rejected.
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

    /// Assign a TrueType/OpenType font with quadratic `glyf` outlines.
    /// CFF/CFF2 outlines, collections and unsupported hint instructions return
    /// errors through the existing TrueType engine. Data must outlive this set.
    /// Sizes are dots per em, rounded to whole dots (1..=4096); width and height
    /// scale independently. The ZPL cell baseline is 3/4 of the rounded height,
    /// not the font's typographic ascender. The specification profile uses standard
    /// hinting/scan semantics. `supplied_truetype_printer_metrics` selects measured
    /// ZD621 scaling, minimum dimensions, spacing and device-space scan conversion.
    /// No kerning or shaping is added beyond the existing Unicode processing.
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
    /// reads a host file. Supported devices are R/E/B/A/Z, extensions FNT/TTF/TTE/OTF/DAT,
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

    /// Replace the default ROM resolver. Explicit registrations and in-job downloads
    /// take precedence. The callback receives an uppercase, validated device:path
    /// (an omitted device becomes R:), and is called once per resolved name per render.
    /// `Ok(None)` means unknown; errors propagate at the selecting ZPL command.
    /// Call [`resolve_rom_font`] in the callback to retain bundled ROM lookup.
    /// No filesystem or network access is performed by the default resolver.
    pub fn set_resolver(
        &mut self,
        resolver: impl Fn(&str) -> Result<Option<Face<'a>>, String> + Send + Sync + 'a,
    ) {
        self.resolver = Some(Arc::new(resolver));
    }
}

/// Request-local ZPL selections, separate from caller-owned font resources.
/// ^CW retains a filename, not a snapshot of the face stored under that name.
pub(crate) struct RenderFonts<'r, 'a, 'd> {
    resources: &'r Fonts<'a>,
    aliases: BTreeMap<char, String>,
    selected_name: Option<String>,
    downloaded: BTreeMap<String, Face<'d>>,
    resolved: BTreeMap<String, Face<'a>>,
    registered_faces: BTreeMap<char, Face<'a>>,
    registered_named: BTreeMap<String, Face<'a>>,
    printer_metrics: bool,
    pub(crate) truetype_printer_metrics: bool,
}

impl<'r, 'a, 'd> RenderFonts<'r, 'a, 'd> {
    pub(crate) fn new(
        resources: &'r Fonts<'a>,
        printer_metrics: bool,
        truetype_printer_metrics: bool,
    ) -> Self {
        Self {
            registered_faces: resources
                .faces
                .iter()
                .filter_map(|(id, face)| {
                    face.with_printer_metrics(printer_metrics)
                        .map(|face| (*id, face))
                })
                .collect(),
            registered_named: resources
                .named
                .iter()
                .filter_map(|(name, face)| {
                    face.with_printer_metrics(printer_metrics)
                        .map(|face| (name.clone(), face))
                })
                .collect(),
            printer_metrics,
            truetype_printer_metrics,
            resources,
            aliases: BTreeMap::new(),
            selected_name: None,
            downloaded: BTreeMap::new(),
            resolved: BTreeMap::new(),
        }
    }

    fn checked_name(&mut self, name: &str) -> Result<String, String> {
        let name = font_name(name)?;
        if !self.downloaded.contains_key(&name)
            && !self.resources.named.contains_key(&name)
            && !self.resolved.contains_key(&name)
        {
            let face = match &self.resources.resolver {
                Some(resolve) => resolve(&name)?,
                None => resolve_rom_font(&name)?,
            }
            .ok_or_else(|| format!("unresolved named font {name:?}"))?;
            let face = face
                .with_printer_metrics(self.printer_metrics)
                .unwrap_or(face);
            self.resolved.insert(name.clone(), face);
        }
        Ok(name)
    }

    pub(crate) fn install(
        &mut self,
        download: &'d crate::render::font_downloads::Download,
    ) -> Result<(), String> {
        use crate::render::font_downloads::Download;
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
                let face = face
                    .with_printer_metrics(self.printer_metrics)
                    .unwrap_or(face);
                (name, face)
            }
        };
        self.downloaded.insert(name.clone(), face);
        Ok(())
    }

    pub(crate) fn alias(&mut self, id: char, name: &str) -> Result<(), String> {
        let name = self.checked_name(name)?;
        self.aliases.insert(id, name);
        Ok(())
    }

    pub(crate) fn select_named(&mut self, name: &str) -> Result<(), String> {
        self.selected_name = Some(self.checked_name(name)?);
        Ok(())
    }

    pub(crate) fn has_named_selection(&self) -> bool {
        self.selected_name.is_some()
    }

    /// The selected profile determines bitmap placement independently of whether
    /// the resource came from the API, a resolver, or an in-job download.
    pub(crate) fn scalable_layout(&self, id: char) -> bool {
        self.get(id).map_or_else(
            || resident::is_scalable(id),
            |face| face.compact().is_none() && !face.bitmap_printer_metrics,
        )
    }

    pub(crate) fn get(&self, id: char) -> Option<&Face<'_>> {
        let name = if id == NAMED_FONT {
            self.selected_name.as_ref()
        } else {
            self.aliases.get(&id)
        };
        match name {
            Some(name) => self
                .downloaded
                .get(name)
                .or_else(|| self.registered_named.get(name))
                .or_else(|| self.resources.named.get(name).map(Arc::as_ref))
                .or_else(|| self.resolved.get(name)),
            None => self
                .registered_faces
                .get(&id)
                .or_else(|| self.resources.faces.get(&id).map(Arc::as_ref)),
        }
    }
}

// Virtual filenames only: Zebra Programming Guide ^CW p. 168 / ^A@ p. 62.
// https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
// Restrict lookup to explicit font extensions on supported drives.
pub(crate) fn font_name(name: &str) -> Result<String, String> {
    let name = name.trim().to_ascii_uppercase();
    let (device, file) = name.split_once(':').unwrap_or(("R", &name));
    if !matches!(device, "R" | "E" | "B" | "A" | "Z") {
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

/// Resolve a recovered ZD621 ROM font by virtual printer filename.
/// Names are case-insensitive. Returns `None` for valid names outside the bundled
/// catalog, including other devices. Invalid paths return an error. No I/O occurs.
/// The bundle preserves measured encodings; it is not a complete ROM image.
/// EPL6/EPL7 have only size-one captures, so other requested sizes are rejected.
///
/// ```
/// use zpl::fonts::{Fonts, resolve_rom_font};
/// let mut fonts = Fonts::new();
/// fonts.set_resolver(|path| {
///     // Resolve application-owned paths here, then delegate other names.
///     resolve_rom_font(path)
/// });
/// assert!(resolve_rom_font("z:e12.fnt")?.is_some());
/// # Ok::<(), String>(())
/// ```
/// Zebra ^WD documents Z: as the resident font namespace:
/// <https://docs.zebra.com/us/en/printers/software/zpl-pg/zpl-commands/%5Ewd.html>.
pub fn resolve_rom_font(name: &str) -> Result<Option<Face<'static>>, String> {
    let name = font_name(name)?;
    Ok(
        zpl_bitmap_fonts::zd621::font_by_name(&name).map(|font| Face {
            source: Source::Compact(font),
            baseline: font.cell_metrics().map_or(0., |m| {
                (f64::from(m.baseline) - 1.) / f64::from(m.cell_height)
            }),
            missing_advance: None,
            bitmap_printer_metrics: false,
        }),
    )
}

// A direct ^A@ selection has its own slot, distinct from ^GS and all public IDs.
pub(crate) const NAMED_FONT: char = '\0';

impl<'a> Face<'a> {
    // Build a request-local metrics view while sharing immutable glyph/provider
    // storage. A registered face represents a font already installed on the device.
    fn with_printer_metrics(&self, enabled: bool) -> Option<Self> {
        if !enabled {
            return None;
        }
        let (source, height) = match &self.source {
            Source::Bitmap(settings, glyphs) => (
                Source::Bitmap(*settings, Arc::clone(glyphs)),
                settings.height,
            ),
            Source::Provider(metrics, provider) => (
                Source::Provider(*metrics, Arc::clone(provider)),
                metrics.height,
            ),
            _ => return None,
        };
        Some(Self {
            source,
            baseline: self.baseline - 1. / height as f64,
            missing_advance: self.missing_advance,
            bitmap_printer_metrics: true,
        })
    }

    /// Construct a format-independent bitmap provider face for a path resolver.
    /// See [`Fonts::insert_bitmap_font`] for metrics and lifetime requirements.
    pub fn provider(font: Arc<dyn BitmapFont + 'a>) -> Result<Self, String> {
        let metrics = font.metrics();
        if !(1..=4096).contains(&metrics.width)
            || !(1..=4096).contains(&metrics.height)
            || !metrics.baseline.is_finite()
            || !(0. ..=metrics.height as f64).contains(&metrics.baseline)
        {
            return Err("invalid bitmap font metrics".into());
        }
        Ok(Self {
            baseline: metrics.baseline / metrics.height as f64,
            source: Source::Provider(metrics, font),
            missing_advance: None,
            bitmap_printer_metrics: false,
        })
    }

    pub(crate) fn compact(&self) -> Option<&'static zpl_bitmap_fonts::Font> {
        match self.source {
            Source::Compact(face) => Some(face),
            _ => None,
        }
    }

    /// Construct a Unicode-keyed bitmap face; see [`Fonts::insert_bitmap`].
    pub fn bitmap(settings: Settings, glyphs: Vec<Glyph>, baseline: f64) -> Result<Self, String> {
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
            source: Source::Bitmap(settings, Arc::new(glyphs)),
            missing_advance: None,
            bitmap_printer_metrics: false,
        })
    }
    /// Construct a borrowed TrueType face; see [`Fonts::insert_truetype`].
    pub fn truetype(data: &'a [u8], hinting: Hinting) -> Result<Self, String> {
        let font = Font::parse(data).map_err(|e| e.to_string())?;
        // ZPL scalable-font cells use a 3/4-height baseline, independently of
        // the font's typographic ascender. See Programming Guide Table 29,
        // p. 1582, and the controlled Heros FO captures in zpl-comparison:
        // https://codyps.github.io/zpl-comparison/fonts/categories/conformance.html
        // Using hhea.ascender here shifts FO text (Heros has a 1.105-em
        // ascender); FT hides the error by subtracting this same baseline.
        let baseline = 0.75;
        Ok(Self {
            source: Source::TrueType(font, hinting),
            missing_advance: None,
            bitmap_printer_metrics: false,
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
    pub(crate) fn bitmap_ft_offset(&self, h: f64, rotation: u8) -> Option<(f64, f64)> {
        if !self.bitmap_printer_metrics {
            return None;
        }
        let height = match &self.source {
            Source::Bitmap(settings, _) => settings.height,
            Source::Provider(metrics, _) => metrics.height,
            _ => return None,
        };
        let scale = h / height as f64;
        // ~DB baseline dot placement, independently captured by the compact
        // FO/FT controls in downloaded-bitmap-zd621-v1.
        Some(match rotation {
            b'R' => (scale, 0.),
            b'I' => (1., scale),
            b'B' => (1. - scale, 1.),
            _ => (0., 1. - scale),
        })
    }

    pub(crate) fn is_truetype(&self) -> bool {
        matches!(self.source, Source::TrueType(..))
    }

    pub(crate) fn baseline(&self, h: f64) -> f64 {
        self.baseline
            * match self.source {
                Source::TrueType(..) => h.round(),
                _ => h,
            }
    }

    pub(crate) fn dimensions(&self, w: f64, h: f64) -> Result<(f64, f64), String> {
        if let Source::Compact(face) = self.source {
            // ^A@ bitmap dimensions round independently to native multiples.
            // https://docs.zebra.com/us/en/printers/software/zpl-pg/zpl-commands/%5Ea-.html
            if !w.is_finite() || !h.is_finite() || w < 0. || h < 0. {
                return Err("font dimensions must be nonnegative and finite".into());
            }
            let Some(m) = face.cell_metrics() else {
                return if w <= 1. && h <= 1. {
                    Ok((1., 1.))
                } else {
                    Err(format!(
                        "ROM font {} has no calibrated sizing metrics; only size 1,1 is supported",
                        face.name
                    ))
                };
            };
            let (nw, nh) = (f64::from(m.cell_width), f64::from(m.cell_height));
            let sx = (w / nw).round().max(1.);
            let sy = (h / nh).round().max(1.);
            return Ok((
                nw * if w == 0. { sy } else { sx },
                nh * if h == 0. { sx } else { sy },
            ));
        }
        let (nw, nh) = match &self.source {
            Source::Compact(_) => unreachable!(),
            Source::Bitmap(s, _) => (
                if s.width == 0 { s.height } else { s.width } as f64,
                s.height as f64,
            ),
            Source::Provider(m, _) => (m.width as f64, m.height as f64),
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
        if self.bitmap_printer_metrics {
            // ZD621 ~DB captures: downloaded-bitmap-zd621-v1. Quantize each
            // axis independently, using the downloaded cell rather than its alias.
            return Ok((nw * (w / nw).round().max(1.), nh * (h / nh).round().max(1.)));
        }
        Ok((w, h))
    }

    pub(crate) fn glyph(
        &self,
        c: char,
        w: f64,
        h: f64,
        environment: Environment,
    ) -> Result<(Cow<'_, Glyph>, f64, f64), String> {
        self.dimensions(w, h)?;
        match &self.source {
            Source::Compact(_) => Err("ROM glyphs require measured encoding lookup".into()),
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
            Source::Provider(metrics, font) => {
                let glyph = font
                    .glyph(c)?
                    .ok_or_else(|| format!("unsupported custom font glyph {c:?}"))?;
                bitmap_font::validate_glyphs(std::slice::from_ref(glyph.as_ref()))?;
                if glyph.codepoint != c as u32 {
                    return Err("bitmap font returned a different codepoint".into());
                }
                Ok((glyph, w / metrics.width as f64, h / metrics.height as f64))
            }
            Source::TrueType(font, hinting) => {
                let index = font
                    .glyph_index(c)
                    .ok_or_else(|| format!("unsupported custom font glyph {c:?}"))?;
                let size =
                    Size::new(w.round() as u16, h.round() as u16).map_err(|e| e.to_string())?;
                let instance = font
                    .instance(size, *hinting, environment)
                    .map_err(|e| e.to_string())?;
                let outline = instance.outline(index).map_err(|e| e.to_string())?;
                let advance = instance.layout_advance(index).map_err(|e| e.to_string())?;
                let (mode, turns) = match environment {
                    Environment::Zd621V93 { quarter_turns } => (ScanMode::Zd621V93, quarter_turns),
                    _ => (ScanMode::Center, 0),
                };
                let glyph = rasterize(&outline, c as u32, advance, turns, mode)
                    .map_err(|e| e.to_string())?;
                Ok((
                    Cow::Owned(crate::output::raster::truetype::unrotate_glyph(
                        glyph, turns,
                    )),
                    1.,
                    1.,
                ))
            }
        }
    }
}

pub(crate) fn resolve_glyph<'a>(
    selection: resident::Selection,
    custom: Option<&'a Face<'a>>,
    key: crate::fonts::resident::GlyphKey,
    w: f64,
    h: f64,
    environment: Environment,
) -> Result<(resident::GlyphView<'a>, f64, f64), String> {
    if let Some(custom) = custom {
        if let Some(face) = custom.compact() {
            let (nw, nh) = face.cell_metrics().map_or((1., 1.), |m| {
                (f64::from(m.cell_width), f64::from(m.cell_height))
            });
            return Ok((
                resident::glyph_from(selection.compact_set(face), key)?,
                w / nw,
                h / nh,
            ));
        }
        let crate::fonts::resident::GlyphKey::Unicode(c) = key else {
            return Err("invalid custom font key".into());
        };
        let (g, sx, sy) = custom.glyph(c, w, h, environment)?;
        return Ok((
            resident::GlyphView {
                advance: g.advance,
                left: g.left,
                top: g.top,
                width: g.width,
                height: g.height,
                pixels: resident::Pixels::Custom(g),
            },
            sx,
            sy,
        ));
    }
    let (glyphs, sx, sy) = resident::selected_for_char(selection, key, w, h);
    Ok((resident::glyph_from(glyphs, key)?, sx, sy))
}
