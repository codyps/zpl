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
    /// Restart hanging indentation after each explicit FB carriage return/line
    /// feed. Disabled: indent every line after the first in the field, matching
    /// the guide's second-and-remaining-lines wording (^FB pp. 186–187).
    pub block_hard_break_resets_indent: bool,
    /// Pivot rotated preset-font FO fields at the final dot rather than the
    /// matrix boundary (^FO p. 201, Table 31 p. 1584). Captured P/Q controls
    /// use a height-scaled final row; R retains the vertical matrix boundary.
    /// All three lose one dot of horizontal advance for I/B. FT retains its
    /// baseline origin.
    pub preset_font_fo_last_dot: bool,
    /// Resolve unavailable resident IDs 1–9, I–O and W–Z using the current CF
    /// font for A fields, and font A for CF. Measured on ZD621 V93.21.33Z;
    /// the guide lists A–Z/0–9 (^A pp. 60–61) and invalid CF fallback (^CF
    /// p. 154), without specifying unavailable field-font resolution.
    /// Disabled: report these unavailable fonts as unsupported. P–V are
    /// separate preset fonts and are never substituted by this option.
    pub unavailable_fonts_use_default: bool,
    /// Match FE token parsing on the ZD621: doubled markers escape one marker,
    /// directions are case-insensitive, negative lengths extend to the end,
    /// and a space or command-delimiter operand selects the default # marker. Disabled:
    /// preserve doubled markers and use the documented parameter forms.
    pub concatenation_printer_syntax: bool,
    /// Keep FE active across intervening commands until field data. Disabled:
    /// require FE immediately before FD as documented (^FE pp. 191–192).
    pub concatenation_retains_delimiter: bool,
    /// For FE backward extraction, count the start from the end, then read
    /// forward. Disabled: select the requested characters ending at that
    /// position, matching the guide example b,1,4 -> Data (^FE p. 192).
    pub concatenation_backward_reads_forward: bool,
    /// Assign inline FN data only to preceding unresolved references, consuming
    /// the binding field when references exist. The ZD621 preview does not
    /// reuse that value for subsequent references. Disabled: share the last
    /// supplied value with all data-less fields of that number (^FN p. 200).
    pub numbered_fields_forward_only: bool,
    /// Preserve PA properties whose operands are omitted, as the ZD621 does.
    /// Disabled: use the documented zero default for each missing operand
    /// (Zebra Programming Guide ^PA, p. 315).
    pub advanced_text_omitted_flags_persist: bool,
    /// Treat Unicode 6.3 directional isolates U+2066–2069 as unassigned class-L
    /// characters with the selected PA fallback, matching the captured printer.
    /// Disabled: use their standard isolate semantics without visible glyphs.
    pub bidi_isolates_as_missing_glyphs: bool,
    /// Omit Unicode paired-bracket resolution (UAX #9 rule N0), matching the
    /// measured ZD621 ordering around RTL text and following digits. Disabled:
    /// resolve paired brackets using the standard Unicode bidirectional rules.
    /// https://www.unicode.org/reports/tr9/#N0
    pub bidi_skips_paired_bracket_resolution: bool,
    /// Retain SN values whose rightmost numeric run exceeds the documented
    /// twelve-digit limit (^SN pp. 341–342), without suppressing their zeros.
    /// Disabled: report unsupported input rather than silently skipping indexing.
    pub serial_overlong_keeps_value: bool,
    /// Replace nonnumeric EAN-8/EAN-13/UPC-A field bytes with zero on the ZD621.
    /// Disabled: reject nonnumeric data (Zebra guide pp. 83, 109, 142).
    pub retail_non_digits_as_zero: bool,
    /// Discard a supplied retail check digit and recompute it on the ZD621.
    /// Disabled: apply the documented left truncation to the data width.
    pub retail_ignore_supplied_check_digit: bool,
    /// Match the ZD621's overlong EAN data windows, including preservation of
    /// EAN-13's first digit. Disabled: truncate data on the left as documented.
    pub retail_printer_overlong_data: bool,

    /// Clamp FB line pitch to zero when negative spacing exceeds font height.
    /// Disabled: retain signed line pitch (^FB p. 186).
    pub block_negative_pitch_clamps_to_zero: bool,
    /// Center overflowing FB lines with trailing space, reject negative rotated
    /// inline ink starts, then place each surviving glyph at the label edge.
    /// Disabled: retain ordinary centered coordinates (^FB pp. 186–187).
    pub block_center_overflow_clamps_to_origin: bool,
    /// Keep an intact word's fit when moving it past a hanging indent; continue
    /// text without wrapping when indent exceeds block width. Disabled: honor
    /// the signed remaining width on every line (^FB p. 186).
    pub block_indent_printer_layout: bool,
    /// Use captured FB soft-marker fitting, retained width decisions across
    /// indentation, and overflowing remainder lines. Also accept nonalphanumeric
    /// markers, including the silent `\(` break. Disabled: use documented
    /// alphanumeric markers with normal width and indentation (^FB p. 187).
    pub block_soft_hyphen_printer_layout: bool,
    /// Apply CI0's native backslash-to-cent glyph replacement in CI28 too.
    /// Disabled: CI28 backslashes retain their Unicode glyph (^CI p. 157).
    /// CI0 always uses its documented character mapping (^CI p. 159).
    pub utf8_uses_legacy_backslash: bool,
    /// Allow escaped FB backslashes in CI0/27/28. The guide (^FB p. 187,
    /// Item 1) requires CI13.
    /// Disabled: reject escaped backslashes in other encodings.
    pub block_backslash_without_ci13: bool,
    /// Use the measured right-justified FO block anchors: R preserves line
    /// alignment within the block; B omits the final
    /// line height from its shift; I uses one dot minus block width.
    /// Disabled: use ordinary text-field justification (^FO p. 201, ^FB p. 187).
    pub block_fo_right_justification_printer_layout: bool,
    /// Clamp the field origin after label home/shift, then clamp each rotated
    /// glyph's ink origin independently after FO/FT. Disabled: clip negative coordinates at the canvas.
    /// Measured on ZD621; ^FO p. 201, ^FT p. 205 and ^FB pp. 186–187.
    pub text_clamps_negative_origins: bool,
    /// Round centered FB line positions down before rotating their paths.
    /// Disabled: keep fractional centers through rasterization (^FB p. 187).
    pub block_center_rounds_down: bool,
    /// Emit one character when a block cannot fit a character plus hyphen,
    /// adding a hyphen only if both fit. Separators can consume blank rows.
    /// Disabled: suppress blocks narrower than the font (^FB p. 186) and
    /// reject remaining unrepresentable hyphenation. Measured on ZD621.
    pub block_narrow_printer_layout: bool,
    /// Clamp a graphic's top-left origin after home, shift and FT placement.
    /// FO bitmaps apply justification after clamping. With the last-row
    /// baseline enabled, FT y <= graphic height starts at row zero.
    /// Disabled: clip negative coordinates at the canvas. Measured on ZD621
    /// V93.21.33Z; ^FO p. 201, ^FT p. 205, ^LS p. 296.
    pub graphic_clamps_negative_origin: bool,
    /// Use nominal graphic height minus one as the FT baseline, including
    /// blank bitmap rows. Disabled: use nominal graphic height (^FT p. 205 Table 7).
    pub graphic_ft_last_row_baseline: bool,
    /// Use soft-hyphen advance, strict fit, and retained hyphen space for every
    /// chunk of an automatically split word (^FB p. 187). Disabled: use a
    /// normal hyphen, permit exact fits and keep a fitting remainder whole.
    pub block_hyphenation_printer_layout: bool,
    /// Paint eth (U+00F0) for automatic hyphens under CI27 while measuring the
    /// selected hyphen. Disabled: paint the selected hyphen normally. Literal
    /// soft hyphens and CI0/CI28 are unaffected; measured on ZD621 V93.21.33Z.
    pub block_hyphenation_ci27_uses_eth: bool,
    /// Distribute integer justification remainders to the earliest word gaps
    /// (^FB p. 187). Vertical overprint flow rounds its extra positions upward.
    /// Disabled: round accumulated positions to the nearest dot. Native controls
    /// cover one through six gaps; see field-block-rounding-zd621-v1 and
    /// font0-common-zd621-v1.
    pub block_justification_rounds_up: bool,
    /// Ignore FO/FT right justification for GS symbol fields. Disabled: honor
    /// the requested field justification (^FO p. 201, ^FT p. 205).
    pub graphic_symbol_ignores_justification: bool,
    /// Use the captured native GS baseline (row 23 of 24), rather than the
    /// 3/4-height baseline in Table 29, p. 1582. Scaled/rotated dot placement
    /// is independently selected by bitmap_font_ft_dot_origin. FO is unaffected.
    pub graphic_symbol_last_row_baseline: bool,
    /// Selecting bitmap fonts A through H with CF but no size resets to the
    /// native size. See resident-h-zd621-v1 and the earlier resident-font suites.
    /// Disabled: retain previous CF dimensions (^CF p. 154). A commands inherit CF.
    pub bitmap_cf_font_only_resets_size: bool,
    /// Apply captured bitmap-font FT dot offsets after scaling and rotation.
    /// FO and proportional font 0 are unaffected. Disabled: use the scaled
    /// native baseline geometrically (^FT p. 205 Table 7). The resident-font
    /// suites, including resident-h-zd621-v1, cover scales 1/2/3 and unequal axes.
    pub bitmap_font_ft_dot_origin: bool,
    /// Remove the farthest bar-height dot for R at x <= 0 and I at y <= 0.
    /// This uses the final bar ink position, after ^FO/^FT, home and shift.
    /// Captured for ^B1/^B2/^B3/^BA/^BC in barcode-boundary-zd621-v1;
    /// nominal field placement is specified by ^FO/^FT pp. 201/205.
    /// Negative-ink translation is controlled independently by the next option.
    pub linear_barcode_rotated_edge_loses_dot: bool,
    /// Clamp bars and individual caption glyphs at negative label edges, then
    /// union black ink (reverse printing toggles overlapping components twice).
    /// R/I retain resident-A blank bottom rows; N/B clamp visible glyph ink.
    /// Captured for ^B1/^B2/^B3/^BA/^BC in
    /// barcode-edges-zd621-v1 and barcode-padding-zd621-v1; unlike nominal
    /// ^FO/^FT placement (pp. 201/205).
    /// Disabled: clip the positioned field at the canvas boundary.
    pub linear_barcode_clamps_negative_ink: bool,
    /// Reproduce ZD621 Code 93 extended C-check interpretation, including
    /// shift lookahead, resident control glyphs and malformed repeated tails.
    /// Disabled: print the documented ZPL substitutes for checksum values
    /// 43–46. Raw exhaustive controls: code93-checks-zd621-v1; ^BA pp. 87–89.
    pub code93_extended_checksum_preview: bool,
    /// Quantize Code 11 wide elements to floor(module * ratio), and extra-wide
    /// elements to floor(module * ratio * 5/3), as captured in code11-widths-zd621-v1.
    /// Disabled: retain the nominal 2W-X extra-wide geometry.
    pub code11_printer_element_widths: bool,
    /// Include Code 11 checksum digits and captured triangular start/stop
    /// interpretation glyphs. ^B1 p. 66; linear-caption-zd621-v1 controls.
    pub code11_interpretation_symbols: bool,
    /// Include the captured hollow-box Code 93 interpretation delimiters.
    /// ^BA pp. 87–89; linear-caption-zd621-v1 controls.
    pub code93_interpretation_symbols: bool,
    /// Captured Codabar interpretation includes the selected start/stop letters.
    /// ^BK pp. 118–119; see linear-caption-zd621-v1 controls.
    pub codabar_interpretation_delimiters: bool,
    /// Center POSTNET/PLANET captions over complete bar pitches, including
    /// the final gap. Bar geometry is unchanged; ^B5/^BZ preview controls.
    pub postal_interpretation_full_pitch: bool,
    /// Captured ^B1/^B2/^B5/^BA/^BK interpretation uses module-scaled resident A
    /// despite preceding explicit font commands. ^BC remains configurable.
    pub linear_interpretation_ignores_font: bool,
    /// Captured ZD621 203-DPI below-bar UPC/EAN digit groups, font selection,
    /// four-dot gap, and bar-width rotation pivot. ^BU pp. 142–143 describes
    /// the A/OCR-B switch; exact placement is pinned by retail-caption-zd621-v1.
    /// Other resolutions and above-bar captions retain their general layout.
    pub retail_interpretation_printer_layout: bool,
    /// ZD621 normal/bottom-up linear barcode ^FT origins include the last
    /// bar row. Applies to ^B1/2/3/5/8/9/A/C/E/I/J/K/L/M/P/S/U and ^BZ
    /// POSTNET/PLANET. R/I retain their opposite boundary; ^FO is unaffected.
    /// Independent of caption visibility; see linear-ft-zd621-v1 controls
    /// and ^FT p. 205 Table 7. Matrix/stacked families remain independent.
    pub linear_barcode_ft_uses_last_bar_row: bool,
    /// Truncate standalone Code 39 wide elements to whole dots. The ZD621
    /// captures in code39-ratios-zd621-v1 use floor(module * ratio), including
    /// 9 * 2.4 = 21 dots. The ^BY example (p. 148) instead rounds that width
    /// to 22 dots; with this override disabled, use nearest-dot rounding.
    pub code39_floor_wide_elements: bool,
    /// Include start/stop asterisks and the enabled Mod-43 check digit in
    /// Code 39 interpretation, as in captured ZD621 ^B3 captions. This does
    /// not alter encoded bars. Default: display the field data alone.
    pub code39_interpretation_symbols: bool,
    /// Use Code 39's module-scaled resident-A interpretation despite a
    /// preceding explicit ^A font selection, matching captured ZD621 previews.
    /// Default: honor the explicit font, as for the other linear barcodes.
    pub code39_interpretation_ignores_font: bool,
    /// Use measured ZD621 rounded-box geometry: a minimum two-dot border
    /// and an independent inner rounding percentage. With the integer curve enabled, both corner radii
    /// are at least two dots. The nominal profile uses constant-distance outlines.
    /// Zebra ^GB, pp. 210–211, defines the rounding scale but not these details.
    pub rounded_box_printer_geometry: bool,
    /// Use the integer corner curve measured in ZD621 ^GB previews instead of
    /// cubic circular arcs. Rounded-box border and radius rules remain separately selectable.
    pub rounded_box_printer_curve: bool,
    /// Use the measured ZD621 circle curve, minimum border, and span endpoints
    /// for ^GC and equal-axis ^GE.
    /// See circles-zd621-v1; ^GC does not specify pixel-level scan conversion.
    pub circle_printer_curve: bool,
    /// Use the measured unequal-axis ^GE curve and border geometry.
    /// See the pixel-exact ellipses-zd621-v1 printer controls.
    pub ellipse_printer_curve: bool,
    /// Reproduce ZD621 A1/B1 separator templates next to a four-module data
    /// bar, including A1 ink beneath a bar and a solid four-module space.
    /// ISO/IEC 24724:2011 §7.2.8 instead requires light dots beneath bars
    /// and alternating dark/light dots beneath the finder spaces.
    pub databar_expanded_wide_bar_separator: bool,
    /// Reproduce the malformed ZD621 DataBar Expanded long-weight/no-date
    /// preview: nonstandard headers with the final digit repeated. ISO/IEC
    /// 24724:2011 §7.2.5.4.4 instead specifies methods 56/57 and date 38400.
    /// Keep disabled when a standards-conforming, decodable symbol is needed.
    pub databar_expanded_no_date_preview: bool,
    /// Encode TLC39 additional-field separators as asterisks, as in ZD621
    /// previews. Default retains GS separators for TCIF supplementary fields.
    pub tlc39_asterisk_separator: bool,
    /// Extend TLC39's linkage flag to intercept tilted scans across Code 39,
    /// as in US20010045461A1 ¶0067/Fig. 2 and ZD621 previews. Default uses
    /// the equal-height flag in the patent's original Fig. 1 embodiment.
    pub tlc39_extended_link_flag: bool,
    /// Use the ZD621 TLC39 component layout: at least six MicroPDF417 rows,
    /// one dot above the 2D ink and one Code 39 module between components.
    /// The TCIF patent US20010045461A1 ¶0027 leaves spacing/alignment open.
    pub tlc39_printer_layout: bool,
    /// Reserve Byte-compaction capacity for TLC39 data containing additional
    /// fields, while retaining the actual Text/Numeric encoding. Measured in
    /// tlc39-zd621-v1 length-* controls; ^BT pp. 140–141 does not prescribe
    /// this conservative sizing. Default: fit the actual codeword count.
    pub tlc39_additional_data_byte_capacity: bool,
    /// Suppress mode 4/6 MaxiCode fields shorter than six bytes, as in ZD621
    /// previews. The ^BD description (Zebra guide p. 107) gives no such limit.
    pub maxicode_standard_minimum_six_bytes: bool,
    /// End MaxiCode input at NUL, matching the printer preview. ISO/IEC
    /// 16023:2000 Annex A otherwise permits NUL in code set E.
    pub maxicode_nul_terminates_data: bool,
    /// Emit a final A/B latch before MaxiCode padding, as in ZD621 previews.
    /// ISO/IEC 16023:2000 §4.4.4.8 permits padding directly in either set.
    pub maxicode_terminal_latch: bool,
    /// Use the captured ZD621 dot template for MaxiCode modules and finder rings.
    /// Includes the full 199-dot symbol baseline for ^FT. Default: nominal
    /// physical geometry from ISO/IEC 16023:2000 §4.11.
    pub maxicode_printer_dot_geometry: bool,
    /// Draw only the fixed finder/orientation marks for MaxiCode mode 5,
    /// matching ZD621 V93.21.33Z HTTP preview. This deliberately reproduces
    /// an undecodable preview, not the full mode-5 symbol specified by ^BD.
    pub maxicode_mode5_preview_omits_data: bool,
    /// Use tilde as the default ECC200 escape, observed on the ZD621 despite
    /// the guide's modern-firmware underscore note (^BX pp. 145–147).
    /// An explicit ^BX g operand takes precedence.
    pub data_matrix_default_tilde_escape: bool,
    /// Check EDIFACT transitions before the fourth character and retain equal-length
    /// tails, as captured on the ZD621. Default: Annex P's four-character
    /// boundary checks and ASCII tie choice.
    pub data_matrix_edifact_printer_transitions: bool,
    /// Truncate fractional default Aztec parity-codeword requirements, as in ZD621
    /// boundary captures. Default: round up to meet the requested minimum
    /// percentage (ISO/IEC 24778 §11.2; Zebra ^BO p. 124).
    pub aztec_floor_default_error_correction: bool,
    /// Keep runs of bytes outside Aztec's text tables in one binary shift
    /// (up to 2078 bytes). The ZD621 uses an extended count for 32–62 bytes;
    /// default encodation can save one bit by using two short binary shifts.
    pub aztec_preserve_binary_runs: bool,
    /// Give ^BR UPC/EAN components uniform 74X (EAN-8: 60X) bars and a
    /// seven-module left margin, as captured on the ZD621. Default: nominal
    /// GS1 proportions with five-module guard extensions and no origin margin.
    pub databar_retail_printer_dimensions: bool,
    /// Require eleven uncompressed UPC-A digits for ^BR UPC-E. The printer
    /// rejects the compressed six/seven/eight-digit form accepted by ^B9.
    pub databar_upce_requires_upca_data: bool,
    /// Interpret ^BR composite linear height as modules instead of the dots
    /// specified on guide p. 135. Observed with module widths one through three.
    pub composite_height_in_modules: bool,
    /// Include ten modules before the GS1-128 component of ^BR composites.
    /// Captured preview placement; default: anchor the complete symbol at ^FO.
    pub composite_linear_quiet_zone: bool,
    /// Include the ZD621's trailing ink margin for inverted, right-justified
    /// ^FO text: measured bitmap margin plus two dots (E uses six native
    /// margin dots; other bitmap fonts use their gap); font 0's trailing bearing less
    /// one dot, clamped to zero. Default: anchor the right ink edge (^FO p. 201).
    pub right_justified_inverted_text_uses_ink_margin: bool,
    /// Use the captured interpretation gaps: six dots below bars, eight dots
    /// after the font cell above. ^BC permits a preceding font command (p. 94)
    /// but does not prescribe these raster gaps. Default: three-dot gaps.
    pub barcode_interpretation_printer_layout: bool,
    /// Anchor rotated ^FO barcodes using bar height, excluding interpretation
    /// text and retail guard extensions. Captured above/below interpretation
    /// controls cover ^B2/^BC/^BE/^BU in all four orientations. The nominal
    /// ^FO upper-left field extent (p. 201) includes the complete drawing.
    pub barcode_fo_uses_bar_height: bool,
    /// Anchor reversed ^FO Code 128 fields at the bar width, excluding explicit
    /// captions that extend beyond either edge. The nominal ^FO field extent
    /// (p. 201) includes the complete drawing. See resident-f-zd621-v1.
    pub code128_fo_uses_bar_width: bool,
    /// Place above-bar interpretation above the bar origin, rather than
    /// pushing the bars down. Covers non-Code-128 barcodes; Code 128 retains
    /// its separately selectable `code128_above_text_keeps_bar_origin` flag.
    /// See the measured ^FO/^B2/^BE/^BU above-interpretation controls.
    pub barcode_above_text_keeps_bar_origin: bool,
    /// Shift automatic barcode interpretation and explicit ^BC A through H
    /// bitmap captions one dot along the reversed I/B reading direction.
    /// N/R and explicitly selected proportional font 0 are unaffected.
    /// Raw explicit-font controls: resident-e-zd621-v1, resident-f-zd621-v1,
    /// resident-g-zd621-v1 and resident-h-zd621-v1.
    pub barcode_reverse_interpretation_shift: bool,
    /// Ignore label-top adjustment in HTTP previews. Physical-print ^LT
    /// semantics remain the default (Zebra guide p. 294).
    pub preview_ignores_label_top: bool,
    /// Ignore inverted print orientation in HTTP previews. Default: honor
    /// ^PO (Zebra guide p. 315); this option describes previews only.
    pub preview_ignores_print_orientation: bool,
    /// Ignore label mirroring in ZD621 HTTP previews. Disabled: mirror the
    /// entire printable area horizontally for ^PMY (Zebra guide p. 319).
    pub preview_ignores_print_mirror: bool,
    /// ZD621 vertical ^FP text advances by font height, ignoring its gap.
    /// Disabled: add the inter-character gap specified by ^FP (guide p. 202).
    pub field_vertical_ignores_gap: bool,
    /// Captured ZD621 rotated right-justified ^FP anchor offsets.
    /// Disabled: use the field interaction anchors in guide pp. 1606–1611.
    pub field_direction_printer_anchors: bool,
    /// In ^FB, vertical ^FP overprints each line and reverse ^FP starts one
    /// advance before the line origin. Reverse alignment and justification
    /// follow the captured printer positions, including forward remainders.
    /// Disabled: use ordinary ^FP glyph flow and ^FB word spacing (Programming
    /// Guide ^FP p. 202, ^FB pp. 186–188; field-block-direction-zd621-v1).
    pub block_field_direction_printer_layout: bool,
    /// Measured ZD621 TB line leading (Programming Guide p. 356).
    /// Disabled: advance by the selected font height.
    pub bounded_text_printer_pitch: bool,
    /// Measured ZD621 TB rectangle anchors and proportional-font dot offsets.
    pub bounded_text_printer_anchors: bool,
    /// A subsequent ^A cancels a previously selected TB block on the ZD621.
    /// Disabled: retain the block while changing the font (guide p. 356).
    pub bounded_text_font_cancels_block: bool,
    /// In horizontal/reverse ^FB, omit the ^FP gap after ASCII word spaces.
    /// Disabled: apply the gap to every character, as in plain text.
    pub block_spaces_ignore_character_gap: bool,
    /// With explicit CODABLOCK F/E columns, fit the actual data instead of
    /// padding to the requested row count. Captured ZD621 sizing choice.
    pub codablock_f_fit_rows: bool,
    /// Include a trailing space in automatic ^FB line alignment when it fits;
    /// explicit paragraph breaks do not add it. A final non-overflow line
    /// without room for that space is fully justified. See field-block-overflow-
    /// zd621-v1; default: align actual text only (^FB pp. 185–187).
    pub block_center_includes_trailing_space: bool,
    /// Interpret CODABLOCK F/E row height as dots, as in ZD621 previews,
    /// instead of the module multiplier described by ^BB (guide p. 90).
    pub codablock_f_row_height_in_dots: bool,
    /// For ^FM MicroPDF417 at I/B orientations, omit the two ten-module side
    /// row-address patterns from the origin adjustment. Captured ZD621 choice;
    /// default: the complete symbol extent required by ^FO (guide p. 201).
    pub macro_micropdf417_reverse_origin_omits_side_raps: bool,
    /// Report an overlong EAN field as INVALID-S instead of the INVALID-L
    /// specified by ^CV (p. 167). UPC-A retains INVALID-L. See the native
    /// validation controls in retail-data-zd621-v1.
    pub validation_retail_long_is_short: bool,
    /// Report forced legacy Data Matrix capacity failures as INVALID-P instead
    /// of INVALID-L (^BX p. 144), as observed on the captured firmware.
    pub validation_legacy_small_is_parameter: bool,
    /// Override Macro PDF417's three file-ID codewords. `None` derives an ID
    /// from the payload. ZD621 HTTP previews consistently use [0, 0, 36].
    /// This is an implementation choice allowed by USS PDF417 Appendix G.4.
    pub macro_pdf417_file_id: Option<[u16; 3]>,
    /// Allow captured ZD621 Model 1 versions 15–40 above the standard limit.
    /// Disabled: ISO/IEC 18004:2000 Annex M limits Model 1 to version 14.
    pub qr_model1_extended_versions: bool,
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
