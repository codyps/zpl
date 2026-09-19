//! Initial rendering options for specification behavior and identified printers.
use super::{compatibility::Compatibility, Options};

/// Specification-based rendering with every printer compatibility override disabled.
///
/// Command behavior follows the references in [`Compatibility`]; this is not a
/// claim of complete ZPL support. The 812 × 1218-dot canvas and 203 DPI are
/// application defaults, not dimensions mandated by the specification.
/// `^PW`/`^LL` and caller edits can override these initial settings.
pub const SPECIFICATION: Options = Options {
    width: 812,
    height: 1218,
    dpi: 203,
    compatibility: Compatibility {
        maxicode_standard_minimum_six_bytes: false,
        maxicode_nul_terminates_data: false,
        maxicode_terminal_latch: false,
        maxicode_printer_dot_geometry: false,
        maxicode_mode5_preview_omits_data: false,
        data_matrix_default_tilde_escape: false,
        data_matrix_edifact_printer_transitions: false,
        aztec_floor_default_error_correction: false,
        aztec_preserve_binary_runs: false,
        databar_retail_printer_dimensions: false,
        databar_upce_requires_upca_data: false,
        composite_height_in_modules: false,
        composite_linear_quiet_zone: false,
        validation_retail_long_is_short: false,
        validation_legacy_small_is_parameter: false,
        macro_pdf417_file_id: None,
        macro_micropdf417_reverse_origin_omits_side_raps: false,
        qr_fo_uses_by_height: false,
        qr_ft_includes_margin: false,
        diagonal_dot_runs: false,
        postal_fixed_pitch: false,
        intelligent_mail_outward_rounding: false,
        retail_guard_extension_dots: None,
        code93_normalize_input: false,
        codablock_a_row_height_in_dots: false,
        codablock_f_row_height_in_dots: false,
        codablock_f_fit_rows: false,
        right_justified_inverted_text_uses_ink_margin: false,
        barcode_interpretation_printer_layout: false,
        preview_ignores_label_top: false,
        preview_ignores_print_orientation: false,
        block_center_includes_trailing_space: false,
        codablock_a_wrapping_checks: false,
        code128_above_text_keeps_bar_origin: false,
    },
};

/// Zebra ZD621, 203 DPI, firmware V93.21.33Z HTTP Preview Label behavior.
///
/// Calibrated against the captures described in `docs/printer-accuracy.md`.
/// Dimensions are initial defaults; `^PW`/`^LL` still override them. This does
/// not emulate preview width adjustment, select QR masks automatically, or
/// promise pixel parity for unimplemented behavior. Other firmware is untested.
///
/// ```
/// use zpl::{Options, render::profiles::ZD621_203_DPI};
/// let mut options = Options { height: 300, ..ZD621_203_DPI };
/// options.compatibility.qr_fo_uses_by_height = false;
/// ```
pub const ZD621_203_DPI: Options = Options {
    width: 832,
    height: 1218,
    dpi: 203,
    compatibility: Compatibility {
        maxicode_standard_minimum_six_bytes: true,
        maxicode_nul_terminates_data: true,
        maxicode_terminal_latch: true,
        maxicode_printer_dot_geometry: true,
        maxicode_mode5_preview_omits_data: true,
        data_matrix_default_tilde_escape: true,
        data_matrix_edifact_printer_transitions: true,
        aztec_floor_default_error_correction: true,
        aztec_preserve_binary_runs: true,
        databar_retail_printer_dimensions: true,
        databar_upce_requires_upca_data: true,
        composite_height_in_modules: true,
        composite_linear_quiet_zone: true,
        validation_retail_long_is_short: true,
        validation_legacy_small_is_parameter: true,
        macro_pdf417_file_id: Some([0, 0, 36]),
        macro_micropdf417_reverse_origin_omits_side_raps: true,
        qr_fo_uses_by_height: true,
        qr_ft_includes_margin: true,
        diagonal_dot_runs: true,
        postal_fixed_pitch: true,
        intelligent_mail_outward_rounding: true,
        retail_guard_extension_dots: Some(13),
        code93_normalize_input: true,
        codablock_a_row_height_in_dots: true,
        codablock_f_row_height_in_dots: true,
        codablock_f_fit_rows: true,
        right_justified_inverted_text_uses_ink_margin: true,
        barcode_interpretation_printer_layout: true,
        preview_ignores_label_top: true,
        preview_ignores_print_orientation: true,
        block_center_includes_trailing_space: true,
        codablock_a_wrapping_checks: true,
        code128_above_text_keeps_bar_origin: true,
    },
};
