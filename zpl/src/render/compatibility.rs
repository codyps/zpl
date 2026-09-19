//! Independently selectable printer preview behavior. `Compatibility::default()`
//! disables empirical overrides, as does [`super::profiles::SPECIFICATION`].
//! [`super::Options::default()`] instead selects the ZD621 printer profile.
//! Neither profile implies complete ZPL implementation coverage.
//!
//! Page references below use the [Zebra ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf).
//! Retail dimensions follow [ISO/IEC 15420:2009](https://www.iso.org/standard/46143.html).

/// Departures and dot-quantization choices observed on real printer previews.
///
/// Start with [`super::profiles::ZD621_203_DPI`] to reproduce the captured
/// firmware, then override individual fields as needed. A profile is only an
/// initial value: the renderer does not reapply it after you change an option.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Compatibility {
    /// Offset QR `^FO` ink by the current `^BY` height minus one dot.
    /// Default: the upper-left origin specified by `^FO` (Zebra guide p. 201).
    pub qr_fo_uses_by_height: bool,
    /// Include three QR modules minus one dot below the symbol at `^FT`.
    /// Default: the barcode base specified by `^FT` (p. 205, Table 7).
    pub qr_ft_includes_margin: bool,
    /// Use the ZD621's stepped horizontal runs for `^GD`, which can extend
    /// outside the specified box. Default: a diagonal band clipped to that box
    /// (p. 213). The guide does not prescribe a scan-conversion algorithm.
    pub diagonal_dot_runs: bool,
    /// Ignore the `^BY` ratio for POSTNET, PLANET and Intelligent Mail; instead
    /// use a truncated 2.5-module pitch. `^BZ` documents ratio support (p. 150).
    pub postal_fixed_pitch: bool,
    /// Round Intelligent Mail tracker boundaries outward before rasterization.
    /// Default: retain fractional thirds of the requested height. This is a
    /// firmware quantization choice, not a documented ZPL requirement.
    pub intelligent_mail_outward_rounding: bool,
    /// Override standalone UPC/EAN guard extension in dots. `None` uses five
    /// modules (ISO/IEC 15420:2009, 4.3.3); the captured ZD621 uses 13 dots.
    /// This explicit dot count is not scaled when the caller changes DPI.
    pub retail_guard_extension_dots: Option<u16>,
    /// Uppercase raw Code 93 letters and discard unsupported bytes, as the
    /// captured firmware does. Default: require the ZPL alphabet and explicit
    /// full-ASCII shift substitutes (Zebra guide pp. 87–89), returning errors
    /// instead of silently changing the field data.
    pub code93_normalize_input: bool,
}
