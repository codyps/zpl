//! Configuration keys mirror native fields; defaults stay in the Rust library.
use crate::{failure, BindingError};
use rustler::{Atom, Decoder, Encoder, Env, Term};
use std::collections::HashMap;

pub type Fields<'a> = HashMap<Atom, Term<'a>>;

pub fn decode<'a, T: Decoder<'a>>(value: Term<'a>, name: &str) -> Result<T, BindingError> {
    value
        .decode()
        .map_err(|_| failure("argument", format!("invalid value for {name}"), None, None))
}

// Rustler 0.38 narrows u8/u16/usize using `as`; explicitly reject overflow.
// https://docs.rs/rustler/0.38.0/src/rustler/types/primitive.rs.html
trait ConfigValue: Sized {
    fn from_term(term: Term<'_>, key: &str) -> Result<Self, BindingError>;
}
macro_rules! direct_value {
    ($($ty:ty),*) => { $(impl ConfigValue for $ty {
        fn from_term(term: Term<'_>, key: &str) -> Result<Self, BindingError> { decode(term, key) }
    })* };
}
direct_value!(bool, u32, u64, f64, Option<u32>);
fn narrow<T: TryFrom<u64>>(value: u64, key: &str) -> Result<T, BindingError> {
    value.try_into().map_err(|_| {
        failure(
            "argument",
            format!("value out of range for {key}"),
            None,
            None,
        )
    })
}
macro_rules! narrow_value {
    ($($ty:ty),*) => { $(impl ConfigValue for $ty {
        fn from_term(term: Term<'_>, key: &str) -> Result<Self, BindingError> {
            narrow(decode::<u64>(term, key)?, key)
        }
    })* };
}
narrow_value!(u8, usize);
impl ConfigValue for Option<u16> {
    fn from_term(term: Term<'_>, key: &str) -> Result<Self, BindingError> {
        decode::<Option<u64>>(term, key)?
            .map(|v| narrow(v, key))
            .transpose()
    }
}
impl ConfigValue for Option<[u16; 3]> {
    fn from_term(term: Term<'_>, key: &str) -> Result<Self, BindingError> {
        decode::<Option<(u64, u64, u64)>>(term, key)?
            .map(|(a, b, c)| Ok([narrow(a, key)?, narrow(b, key)?, narrow(c, key)?]))
            .transpose()
    }
}

macro_rules! field_encode {
    (macro_pdf417_file_id, $value:expr, $env:expr) => {
        $value.map(|[a, b, c]| (a, b, c)).encode($env)
    };
    ($field:ident, $value:expr, $env:expr) => {
        $value.encode($env)
    };
}

macro_rules! configuration {
    ($apply:ident, $fields:ident, $native:ty, {$($field:ident: $ty:ty),* $(,)?}) => {
        pub fn $apply(mut value: $native, fields: Fields<'_>) -> Result<$native, BindingError> {
            for (key, term) in fields {
                let key = key.to_term(term.get_env()).atom_to_string().unwrap();
                match key.as_str() {
                    $(stringify!($field) => value.$field = <$ty as ConfigValue>::from_term(term, &key)?,)*
                    _ => return Err(failure("argument", format!("unknown field: {key}"), None, None)),
                }
            }
            Ok(value)
        }
        pub fn $fields<'a>(env: Env<'a>, value: $native) -> Term<'a> {
            // Only compile-time field names become atoms; never atomize user input.
            let fields = vec![$((Atom::from_str(env, stringify!($field)).unwrap(), field_encode!($field, value.$field, env))),*];
            Term::map_from_pairs(env, &fields).unwrap()
        }
    }
}

configuration!(compatibility, compatibility_fields, zpl::render::compatibility::Compatibility, {
    inline_graphic_implicit_separator: bool,
    qr_malformed_header_uses_defaults: bool,
    code39_normalize_input: bool,
    barcode_module_width_through_12: bool,
    box_zero_thickness_as_one: bool,
    preview_width_quantum: Option<u32>,
    preview_max_width: Option<u32>,
    preview_width_latched_at_first_draw: bool,
    preview_ignores_label_length: bool,
    qr_printer_mask_selection: bool,
    qr_printer_segmentation: bool,
    qr_updates_barcode_module_width: bool,
    pdf417_integer_grid_layout: bool,
    pdf417_punctuation_latches: bool,
    retail_caption_clamps_negative_inline_origin: bool,
    code93_control_interpretation: bool,
    text_nul_processing: bool,
    text_control_processing: bool,
    text_tab_stops: bool,
    text_esc_del_processing: bool,
    remap_space: bool,
    font0_fo_floor_baseline: bool,
    font0_minimum_dimensions: bool,
    block_preserves_extra_spaces: bool,
    block_hard_break_resets_indent: bool,
    preset_font_fo_last_dot: bool,
    font_s_block_metrics: bool,
    block_utf8_formatting_visible: bool,
    unavailable_fonts_use_default: bool,
    concatenation_printer_syntax: bool,
    concatenation_retains_delimiter: bool,
    concatenation_backward_reads_forward: bool,
    numbered_fields_forward_only: bool,
    advanced_text_omitted_flags_persist: bool,
    bidi_isolates_as_missing_glyphs: bool,
    bidi_skips_paired_bracket_resolution: bool,
    serial_overlong_keeps_value: bool,
    serial_ci13_zero_uses_source: bool,
    retail_non_digits_as_zero: bool,
    retail_ignore_supplied_check_digit: bool,
    retail_printer_overlong_data: bool,
    block_negative_pitch_clamps_to_zero: bool,
    block_center_overflow_clamps_to_origin: bool,
    block_indent_printer_layout: bool,
    block_soft_hyphen_printer_layout: bool,
    utf8_uses_legacy_backslash: bool,
    block_backslash_without_ci13: bool,
    block_fo_right_justification_printer_layout: bool,
    text_clamps_negative_origins: bool,
    block_center_rounds_down: bool,
    block_narrow_printer_layout: bool,
    graphic_clamps_negative_origin: bool,
    graphic_ft_last_row_baseline: bool,
    block_hyphenation_printer_layout: bool,
    block_hyphenation_ci27_uses_eth: bool,
    block_justification_rounds_up: bool,
    graphic_symbol_ignores_justification: bool,
    graphic_symbol_last_row_baseline: bool,
    bitmap_cf_font_only_resets_size: bool,
    bitmap_font_maximum_dimensions: bool,
    bitmap_font_ft_dot_origin: bool,
    supplied_bitmap_font_metrics: bool,
    linear_barcode_rotated_edge_loses_dot: bool,
    linear_barcode_clamps_negative_ink: bool,
    code93_extended_checksum_preview: bool,
    code11_printer_element_widths: bool,
    code11_interpretation_symbols: bool,
    code93_interpretation_symbols: bool,
    codabar_interpretation_delimiters: bool,
    postal_interpretation_full_pitch: bool,
    linear_interpretation_ignores_font: bool,
    retail_interpretation_printer_layout: bool,
    linear_barcode_ft_uses_last_bar_row: bool,
    code39_floor_wide_elements: bool,
    code39_interpretation_symbols: bool,
    code39_interpretation_ignores_font: bool,
    rounded_box_printer_geometry: bool,
    rounded_box_printer_curve: bool,
    circle_printer_curve: bool,
    ellipse_printer_curve: bool,
    databar_expanded_wide_bar_separator: bool,
    databar_expanded_no_date_preview: bool,
    tlc39_asterisk_separator: bool,
    tlc39_extended_link_flag: bool,
    tlc39_printer_layout: bool,
    tlc39_additional_data_byte_capacity: bool,
    maxicode_standard_minimum_six_bytes: bool,
    maxicode_nul_terminates_data: bool,
    maxicode_terminal_latch: bool,
    maxicode_printer_run_boundaries: bool,
    maxicode_printer_dot_geometry: bool,
    maxicode_mode5_preview_omits_data: bool,
    data_matrix_default_tilde_escape: bool,
    data_matrix_edifact_printer_transitions: bool,
    aztec_floor_default_error_correction: bool,
    aztec_preserve_binary_runs: bool,
    databar_retail_printer_dimensions: bool,
    databar_upce_requires_upca_data: bool,
    composite_height_in_modules: bool,
    composite_linear_quiet_zone: bool,
    right_justified_inverted_text_uses_ink_margin: bool,
    barcode_interpretation_printer_layout: bool,
    barcode_fo_uses_bar_height: bool,
    code128_fo_uses_bar_width: bool,
    barcode_above_text_keeps_bar_origin: bool,
    barcode_reverse_interpretation_shift: bool,
    preview_ignores_label_top: bool,
    preview_ignores_print_orientation: bool,
    preview_ignores_print_mirror: bool,
    field_vertical_ignores_gap: bool,
    field_direction_printer_anchors: bool,
    block_field_direction_printer_layout: bool,
    bounded_text_printer_pitch: bool,
    bounded_text_printer_anchors: bool,
    bounded_text_font_cancels_block: bool,
    block_spaces_ignore_character_gap: bool,
    codablock_f_fit_rows: bool,
    block_center_includes_trailing_space: bool,
    codablock_f_row_height_in_dots: bool,
    macro_micropdf417_reverse_origin_omits_side_raps: bool,
    validation_retail_long_is_short: bool,
    validation_legacy_small_is_parameter: bool,
    macro_pdf417_file_id: Option<[u16; 3]>,
    qr_model1_extended_versions: bool,
    qr_fo_uses_by_height: bool,
    qr_ft_includes_margin: bool,
    diagonal_dot_runs: bool,
    postal_fixed_pitch: bool,
    intelligent_mail_outward_rounding: bool,
    retail_guard_extension_dots: Option<u16>,
    code93_normalize_input: bool,
    codablock_a_row_height_in_dots: bool,
    codablock_a_wrapping_checks: bool,
    code128_above_text_keeps_bar_origin: bool,
});

configuration!(render_limits, render_limits_fields, zpl::render::Limits, {
    input_bytes: usize,
    labels: usize,
    segments: usize,
    stored_graphic_segments: usize,
    pixels: usize,
    dimension: u32,
    number_abs: f64,
    field_bytes: usize,
    graphic_bytes: usize,
    font_bytes: usize,
    stored_formats: usize,
    recall_depth: usize,
    recall_calls: usize,
    coordinate_abs: f64,
});

configuration!(output_limits, output_limits_fields, zpl::output::Limits, {
    pixels: usize,
    segments: usize,
    coordinate_abs: f64,
    pages: usize,
    flattened_segments: usize,
    scan_work: u64,
});

configuration!(syntax, syntax_fields, zpl::parse::Syntax, {
    format_prefix: u8,
    control_prefix: u8,
    delimiter: u8,
});
