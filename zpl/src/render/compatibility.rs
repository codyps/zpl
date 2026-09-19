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
    /// Ignore label-top adjustment in HTTP previews. Physical-print ^LT
    /// semantics remain the default (Zebra guide p. 294).
    pub preview_ignores_label_top: bool,
    /// Ignore inverted print orientation in HTTP previews. Default: honor
    /// ^PO (Zebra guide p. 315); this option describes previews only.
    pub preview_ignores_print_orientation: bool,
    /// With explicit CODABLOCK F/E columns, fit the actual data instead of
    /// padding to the requested row count. Captured ZD621 sizing choice.
    pub codablock_f_fit_rows: bool,
    /// Include a trailing space in ^FB alignment when it fits. A final line
    /// without room for that space is fully justified. Observed in ZD621
    /// previews; default: align only the actual text (^FB pp. 185–187).
    pub block_center_includes_trailing_space: bool,
    /// Interpret CODABLOCK F/E row height as dots, as in ZD621 previews,
    /// instead of the module multiplier described by ^BB (guide p. 90).
    pub codablock_f_row_height_in_dots: bool,
    /// For ^FM MicroPDF417 at I/B orientations, omit the two ten-module side
    /// row-address patterns from the origin adjustment. Captured ZD621 choice;
    /// default: the complete symbol extent required by ^FO (guide p. 201).
    pub macro_micropdf417_reverse_origin_omits_side_raps: bool,
    /// Report an overlong UPC/EAN field as INVALID-S instead of the INVALID-L
    /// specified by ^CV (p. 167), as seen in ZD621 EAN-8 previews.
    pub validation_retail_long_is_short: bool,
    /// Report forced legacy Data Matrix capacity failures as INVALID-P instead
    /// of INVALID-L (^BX p. 144), as observed on the captured firmware.
    pub validation_legacy_small_is_parameter: bool,
    /// Override Macro PDF417's three file-ID codewords. `None` derives an ID
    /// from the payload. ZD621 HTTP previews consistently use [0, 0, 36].
    /// This is an implementation choice allowed by USS PDF417 Appendix G.4.
    pub macro_pdf417_file_id: Option<[u16; 3]>,
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
    /// Treat CODABLOCK A's row height operand as dots, without multiplying by
    /// the module width as specified by the Zebra guide ^BB (p. 90).
    pub codablock_a_row_height_in_dots: bool,
    /// Wrap CODABLOCK A's weighted checksum sums at 16 bits before modulo 43,
    /// as observed on long/padded ZD621 symbols. Default: mathematical sums.
    pub codablock_a_wrapping_checks: bool,
    /// Keep Code 128 bars at the field origin when interpretation is above,
    /// placing the text above that origin. Default: include the interpretation
    /// in the field's upper-left extent (^FO p. 201; ^BC pp. 94–95).
    pub code128_above_text_keeps_bar_origin: bool,
}
