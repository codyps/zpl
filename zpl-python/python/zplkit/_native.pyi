from typing import Literal

class Compatibility:
    def __init__(
        self,
        *,
        inline_graphic_implicit_separator: bool = ...,
        qr_malformed_header_uses_defaults: bool = ...,
        code39_normalize_input: bool = ...,
        barcode_module_width_through_12: bool = ...,
        box_zero_thickness_as_one: bool = ...,
        preview_width_quantum: int | None = ...,
        preview_max_width: int | None = ...,
        preview_width_latched_at_first_draw: bool = ...,
        preview_ignores_label_length: bool = ...,
        qr_printer_mask_selection: bool = ...,
        qr_printer_segmentation: bool = ...,
        qr_updates_barcode_module_width: bool = ...,
        pdf417_integer_grid_layout: bool = ...,
        pdf417_punctuation_latches: bool = ...,
        retail_caption_clamps_negative_inline_origin: bool = ...,
        code93_control_interpretation: bool = ...,
        text_nul_processing: bool = ...,
        text_control_processing: bool = ...,
        text_tab_stops: bool = ...,
        text_esc_del_processing: bool = ...,
        remap_space: bool = ...,
        font0_fo_floor_baseline: bool = ...,
        font0_minimum_dimensions: bool = ...,
        block_preserves_extra_spaces: bool = ...,
        block_hard_break_resets_indent: bool = ...,
        preset_font_fo_last_dot: bool = ...,
        font_s_block_metrics: bool = ...,
        block_utf8_formatting_visible: bool = ...,
        unavailable_fonts_use_default: bool = ...,
        concatenation_printer_syntax: bool = ...,
        concatenation_retains_delimiter: bool = ...,
        concatenation_backward_reads_forward: bool = ...,
        numbered_fields_forward_only: bool = ...,
        advanced_text_omitted_flags_persist: bool = ...,
        bidi_isolates_as_missing_glyphs: bool = ...,
        bidi_skips_paired_bracket_resolution: bool = ...,
        serial_overlong_keeps_value: bool = ...,
        serial_ci13_zero_uses_source: bool = ...,
        retail_non_digits_as_zero: bool = ...,
        retail_ignore_supplied_check_digit: bool = ...,
        retail_printer_overlong_data: bool = ...,
        block_negative_pitch_clamps_to_zero: bool = ...,
        block_center_overflow_clamps_to_origin: bool = ...,
        block_indent_printer_layout: bool = ...,
        block_soft_hyphen_printer_layout: bool = ...,
        utf8_uses_legacy_backslash: bool = ...,
        block_backslash_without_ci13: bool = ...,
        block_fo_right_justification_printer_layout: bool = ...,
        text_clamps_negative_origins: bool = ...,
        block_center_rounds_down: bool = ...,
        block_narrow_printer_layout: bool = ...,
        graphic_clamps_negative_origin: bool = ...,
        graphic_ft_last_row_baseline: bool = ...,
        block_hyphenation_printer_layout: bool = ...,
        block_hyphenation_ci27_uses_eth: bool = ...,
        block_justification_rounds_up: bool = ...,
        graphic_symbol_ignores_justification: bool = ...,
        graphic_symbol_last_row_baseline: bool = ...,
        bitmap_cf_font_only_resets_size: bool = ...,
        bitmap_font_maximum_dimensions: bool = ...,
        bitmap_font_ft_dot_origin: bool = ...,
        supplied_bitmap_font_metrics: bool = ...,
        supplied_truetype_printer_metrics: bool = ...,
        linear_barcode_rotated_edge_loses_dot: bool = ...,
        linear_barcode_clamps_negative_ink: bool = ...,
        code93_extended_checksum_preview: bool = ...,
        code11_printer_element_widths: bool = ...,
        code11_interpretation_symbols: bool = ...,
        code93_interpretation_symbols: bool = ...,
        codabar_interpretation_delimiters: bool = ...,
        postal_interpretation_full_pitch: bool = ...,
        linear_interpretation_ignores_font: bool = ...,
        retail_interpretation_printer_layout: bool = ...,
        linear_barcode_ft_uses_last_bar_row: bool = ...,
        code39_floor_wide_elements: bool = ...,
        code39_interpretation_symbols: bool = ...,
        code39_interpretation_ignores_font: bool = ...,
        rounded_box_printer_geometry: bool = ...,
        rounded_box_printer_curve: bool = ...,
        circle_printer_curve: bool = ...,
        ellipse_printer_curve: bool = ...,
        databar_expanded_wide_bar_separator: bool = ...,
        databar_expanded_no_date_preview: bool = ...,
        tlc39_asterisk_separator: bool = ...,
        tlc39_extended_link_flag: bool = ...,
        tlc39_printer_layout: bool = ...,
        tlc39_additional_data_byte_capacity: bool = ...,
        maxicode_standard_minimum_six_bytes: bool = ...,
        maxicode_nul_terminates_data: bool = ...,
        maxicode_terminal_latch: bool = ...,
        maxicode_printer_run_boundaries: bool = ...,
        maxicode_printer_dot_geometry: bool = ...,
        maxicode_mode5_preview_omits_data: bool = ...,
        data_matrix_default_tilde_escape: bool = ...,
        data_matrix_edifact_printer_transitions: bool = ...,
        aztec_floor_default_error_correction: bool = ...,
        aztec_preserve_binary_runs: bool = ...,
        databar_retail_printer_dimensions: bool = ...,
        databar_upce_requires_upca_data: bool = ...,
        composite_height_in_modules: bool = ...,
        composite_linear_quiet_zone: bool = ...,
        right_justified_inverted_text_uses_ink_margin: bool = ...,
        barcode_interpretation_printer_layout: bool = ...,
        barcode_fo_uses_bar_height: bool = ...,
        code128_fo_uses_bar_width: bool = ...,
        barcode_above_text_keeps_bar_origin: bool = ...,
        barcode_reverse_interpretation_shift: bool = ...,
        preview_ignores_label_top: bool = ...,
        preview_ignores_print_orientation: bool = ...,
        preview_ignores_print_mirror: bool = ...,
        field_vertical_ignores_gap: bool = ...,
        field_direction_printer_anchors: bool = ...,
        block_field_direction_printer_layout: bool = ...,
        bounded_text_printer_pitch: bool = ...,
        bounded_text_printer_anchors: bool = ...,
        bounded_text_font_cancels_block: bool = ...,
        block_spaces_ignore_character_gap: bool = ...,
        codablock_f_fit_rows: bool = ...,
        block_center_includes_trailing_space: bool = ...,
        codablock_f_row_height_in_dots: bool = ...,
        macro_micropdf417_reverse_origin_omits_side_raps: bool = ...,
        validation_retail_long_is_short: bool = ...,
        validation_legacy_small_is_parameter: bool = ...,
        macro_pdf417_file_id: tuple[int, int, int] | None = ...,
        qr_model1_extended_versions: bool = ...,
        qr_fo_uses_by_height: bool = ...,
        qr_ft_includes_margin: bool = ...,
        diagonal_dot_runs: bool = ...,
        postal_fixed_pitch: bool = ...,
        intelligent_mail_outward_rounding: bool = ...,
        retail_guard_extension_dots: int | None = ...,
        code93_normalize_input: bool = ...,
        codablock_a_row_height_in_dots: bool = ...,
        codablock_a_wrapping_checks: bool = ...,
        code128_above_text_keeps_bar_origin: bool = ...,
    ) -> None: ...
    def replace(
        self,
        *,
        inline_graphic_implicit_separator: bool = ...,
        qr_malformed_header_uses_defaults: bool = ...,
        code39_normalize_input: bool = ...,
        barcode_module_width_through_12: bool = ...,
        box_zero_thickness_as_one: bool = ...,
        preview_width_quantum: int | None = ...,
        preview_max_width: int | None = ...,
        preview_width_latched_at_first_draw: bool = ...,
        preview_ignores_label_length: bool = ...,
        qr_printer_mask_selection: bool = ...,
        qr_printer_segmentation: bool = ...,
        qr_updates_barcode_module_width: bool = ...,
        pdf417_integer_grid_layout: bool = ...,
        pdf417_punctuation_latches: bool = ...,
        retail_caption_clamps_negative_inline_origin: bool = ...,
        code93_control_interpretation: bool = ...,
        text_nul_processing: bool = ...,
        text_control_processing: bool = ...,
        text_tab_stops: bool = ...,
        text_esc_del_processing: bool = ...,
        remap_space: bool = ...,
        font0_fo_floor_baseline: bool = ...,
        font0_minimum_dimensions: bool = ...,
        block_preserves_extra_spaces: bool = ...,
        block_hard_break_resets_indent: bool = ...,
        preset_font_fo_last_dot: bool = ...,
        font_s_block_metrics: bool = ...,
        block_utf8_formatting_visible: bool = ...,
        unavailable_fonts_use_default: bool = ...,
        concatenation_printer_syntax: bool = ...,
        concatenation_retains_delimiter: bool = ...,
        concatenation_backward_reads_forward: bool = ...,
        numbered_fields_forward_only: bool = ...,
        advanced_text_omitted_flags_persist: bool = ...,
        bidi_isolates_as_missing_glyphs: bool = ...,
        bidi_skips_paired_bracket_resolution: bool = ...,
        serial_overlong_keeps_value: bool = ...,
        serial_ci13_zero_uses_source: bool = ...,
        retail_non_digits_as_zero: bool = ...,
        retail_ignore_supplied_check_digit: bool = ...,
        retail_printer_overlong_data: bool = ...,
        block_negative_pitch_clamps_to_zero: bool = ...,
        block_center_overflow_clamps_to_origin: bool = ...,
        block_indent_printer_layout: bool = ...,
        block_soft_hyphen_printer_layout: bool = ...,
        utf8_uses_legacy_backslash: bool = ...,
        block_backslash_without_ci13: bool = ...,
        block_fo_right_justification_printer_layout: bool = ...,
        text_clamps_negative_origins: bool = ...,
        block_center_rounds_down: bool = ...,
        block_narrow_printer_layout: bool = ...,
        graphic_clamps_negative_origin: bool = ...,
        graphic_ft_last_row_baseline: bool = ...,
        block_hyphenation_printer_layout: bool = ...,
        block_hyphenation_ci27_uses_eth: bool = ...,
        block_justification_rounds_up: bool = ...,
        graphic_symbol_ignores_justification: bool = ...,
        graphic_symbol_last_row_baseline: bool = ...,
        bitmap_cf_font_only_resets_size: bool = ...,
        bitmap_font_maximum_dimensions: bool = ...,
        bitmap_font_ft_dot_origin: bool = ...,
        supplied_bitmap_font_metrics: bool = ...,
        supplied_truetype_printer_metrics: bool = ...,
        linear_barcode_rotated_edge_loses_dot: bool = ...,
        linear_barcode_clamps_negative_ink: bool = ...,
        code93_extended_checksum_preview: bool = ...,
        code11_printer_element_widths: bool = ...,
        code11_interpretation_symbols: bool = ...,
        code93_interpretation_symbols: bool = ...,
        codabar_interpretation_delimiters: bool = ...,
        postal_interpretation_full_pitch: bool = ...,
        linear_interpretation_ignores_font: bool = ...,
        retail_interpretation_printer_layout: bool = ...,
        linear_barcode_ft_uses_last_bar_row: bool = ...,
        code39_floor_wide_elements: bool = ...,
        code39_interpretation_symbols: bool = ...,
        code39_interpretation_ignores_font: bool = ...,
        rounded_box_printer_geometry: bool = ...,
        rounded_box_printer_curve: bool = ...,
        circle_printer_curve: bool = ...,
        ellipse_printer_curve: bool = ...,
        databar_expanded_wide_bar_separator: bool = ...,
        databar_expanded_no_date_preview: bool = ...,
        tlc39_asterisk_separator: bool = ...,
        tlc39_extended_link_flag: bool = ...,
        tlc39_printer_layout: bool = ...,
        tlc39_additional_data_byte_capacity: bool = ...,
        maxicode_standard_minimum_six_bytes: bool = ...,
        maxicode_nul_terminates_data: bool = ...,
        maxicode_terminal_latch: bool = ...,
        maxicode_printer_run_boundaries: bool = ...,
        maxicode_printer_dot_geometry: bool = ...,
        maxicode_mode5_preview_omits_data: bool = ...,
        data_matrix_default_tilde_escape: bool = ...,
        data_matrix_edifact_printer_transitions: bool = ...,
        aztec_floor_default_error_correction: bool = ...,
        aztec_preserve_binary_runs: bool = ...,
        databar_retail_printer_dimensions: bool = ...,
        databar_upce_requires_upca_data: bool = ...,
        composite_height_in_modules: bool = ...,
        composite_linear_quiet_zone: bool = ...,
        right_justified_inverted_text_uses_ink_margin: bool = ...,
        barcode_interpretation_printer_layout: bool = ...,
        barcode_fo_uses_bar_height: bool = ...,
        code128_fo_uses_bar_width: bool = ...,
        barcode_above_text_keeps_bar_origin: bool = ...,
        barcode_reverse_interpretation_shift: bool = ...,
        preview_ignores_label_top: bool = ...,
        preview_ignores_print_orientation: bool = ...,
        preview_ignores_print_mirror: bool = ...,
        field_vertical_ignores_gap: bool = ...,
        field_direction_printer_anchors: bool = ...,
        block_field_direction_printer_layout: bool = ...,
        bounded_text_printer_pitch: bool = ...,
        bounded_text_printer_anchors: bool = ...,
        bounded_text_font_cancels_block: bool = ...,
        block_spaces_ignore_character_gap: bool = ...,
        codablock_f_fit_rows: bool = ...,
        block_center_includes_trailing_space: bool = ...,
        codablock_f_row_height_in_dots: bool = ...,
        macro_micropdf417_reverse_origin_omits_side_raps: bool = ...,
        validation_retail_long_is_short: bool = ...,
        validation_legacy_small_is_parameter: bool = ...,
        macro_pdf417_file_id: tuple[int, int, int] | None = ...,
        qr_model1_extended_versions: bool = ...,
        qr_fo_uses_by_height: bool = ...,
        qr_ft_includes_margin: bool = ...,
        diagonal_dot_runs: bool = ...,
        postal_fixed_pitch: bool = ...,
        intelligent_mail_outward_rounding: bool = ...,
        retail_guard_extension_dots: int | None = ...,
        code93_normalize_input: bool = ...,
        codablock_a_row_height_in_dots: bool = ...,
        codablock_a_wrapping_checks: bool = ...,
        code128_above_text_keeps_bar_origin: bool = ...,
    ) -> Compatibility: ...
    @property
    def inline_graphic_implicit_separator(self) -> bool: ...
    @property
    def qr_malformed_header_uses_defaults(self) -> bool: ...
    @property
    def code39_normalize_input(self) -> bool: ...
    @property
    def barcode_module_width_through_12(self) -> bool: ...
    @property
    def box_zero_thickness_as_one(self) -> bool: ...
    @property
    def preview_width_quantum(self) -> int | None: ...
    @property
    def preview_max_width(self) -> int | None: ...
    @property
    def preview_width_latched_at_first_draw(self) -> bool: ...
    @property
    def preview_ignores_label_length(self) -> bool: ...
    @property
    def qr_printer_mask_selection(self) -> bool: ...
    @property
    def qr_printer_segmentation(self) -> bool: ...
    @property
    def qr_updates_barcode_module_width(self) -> bool: ...
    @property
    def pdf417_integer_grid_layout(self) -> bool: ...
    @property
    def pdf417_punctuation_latches(self) -> bool: ...
    @property
    def retail_caption_clamps_negative_inline_origin(self) -> bool: ...
    @property
    def code93_control_interpretation(self) -> bool: ...
    @property
    def text_nul_processing(self) -> bool: ...
    @property
    def text_control_processing(self) -> bool: ...
    @property
    def text_tab_stops(self) -> bool: ...
    @property
    def text_esc_del_processing(self) -> bool: ...
    @property
    def remap_space(self) -> bool: ...
    @property
    def font0_fo_floor_baseline(self) -> bool: ...
    @property
    def font0_minimum_dimensions(self) -> bool: ...
    @property
    def block_preserves_extra_spaces(self) -> bool: ...
    @property
    def block_hard_break_resets_indent(self) -> bool: ...
    @property
    def preset_font_fo_last_dot(self) -> bool: ...
    @property
    def font_s_block_metrics(self) -> bool: ...
    @property
    def block_utf8_formatting_visible(self) -> bool: ...
    @property
    def unavailable_fonts_use_default(self) -> bool: ...
    @property
    def concatenation_printer_syntax(self) -> bool: ...
    @property
    def concatenation_retains_delimiter(self) -> bool: ...
    @property
    def concatenation_backward_reads_forward(self) -> bool: ...
    @property
    def numbered_fields_forward_only(self) -> bool: ...
    @property
    def advanced_text_omitted_flags_persist(self) -> bool: ...
    @property
    def bidi_isolates_as_missing_glyphs(self) -> bool: ...
    @property
    def bidi_skips_paired_bracket_resolution(self) -> bool: ...
    @property
    def serial_overlong_keeps_value(self) -> bool: ...
    @property
    def serial_ci13_zero_uses_source(self) -> bool: ...
    @property
    def retail_non_digits_as_zero(self) -> bool: ...
    @property
    def retail_ignore_supplied_check_digit(self) -> bool: ...
    @property
    def retail_printer_overlong_data(self) -> bool: ...
    @property
    def block_negative_pitch_clamps_to_zero(self) -> bool: ...
    @property
    def block_center_overflow_clamps_to_origin(self) -> bool: ...
    @property
    def block_indent_printer_layout(self) -> bool: ...
    @property
    def block_soft_hyphen_printer_layout(self) -> bool: ...
    @property
    def utf8_uses_legacy_backslash(self) -> bool: ...
    @property
    def block_backslash_without_ci13(self) -> bool: ...
    @property
    def block_fo_right_justification_printer_layout(self) -> bool: ...
    @property
    def text_clamps_negative_origins(self) -> bool: ...
    @property
    def block_center_rounds_down(self) -> bool: ...
    @property
    def block_narrow_printer_layout(self) -> bool: ...
    @property
    def graphic_clamps_negative_origin(self) -> bool: ...
    @property
    def graphic_ft_last_row_baseline(self) -> bool: ...
    @property
    def block_hyphenation_printer_layout(self) -> bool: ...
    @property
    def block_hyphenation_ci27_uses_eth(self) -> bool: ...
    @property
    def block_justification_rounds_up(self) -> bool: ...
    @property
    def graphic_symbol_ignores_justification(self) -> bool: ...
    @property
    def graphic_symbol_last_row_baseline(self) -> bool: ...
    @property
    def bitmap_cf_font_only_resets_size(self) -> bool: ...
    @property
    def bitmap_font_maximum_dimensions(self) -> bool: ...
    @property
    def bitmap_font_ft_dot_origin(self) -> bool: ...
    @property
    def supplied_bitmap_font_metrics(self) -> bool: ...
    @property
    def supplied_truetype_printer_metrics(self) -> bool: ...
    @property
    def linear_barcode_rotated_edge_loses_dot(self) -> bool: ...
    @property
    def linear_barcode_clamps_negative_ink(self) -> bool: ...
    @property
    def code93_extended_checksum_preview(self) -> bool: ...
    @property
    def code11_printer_element_widths(self) -> bool: ...
    @property
    def code11_interpretation_symbols(self) -> bool: ...
    @property
    def code93_interpretation_symbols(self) -> bool: ...
    @property
    def codabar_interpretation_delimiters(self) -> bool: ...
    @property
    def postal_interpretation_full_pitch(self) -> bool: ...
    @property
    def linear_interpretation_ignores_font(self) -> bool: ...
    @property
    def retail_interpretation_printer_layout(self) -> bool: ...
    @property
    def linear_barcode_ft_uses_last_bar_row(self) -> bool: ...
    @property
    def code39_floor_wide_elements(self) -> bool: ...
    @property
    def code39_interpretation_symbols(self) -> bool: ...
    @property
    def code39_interpretation_ignores_font(self) -> bool: ...
    @property
    def rounded_box_printer_geometry(self) -> bool: ...
    @property
    def rounded_box_printer_curve(self) -> bool: ...
    @property
    def circle_printer_curve(self) -> bool: ...
    @property
    def ellipse_printer_curve(self) -> bool: ...
    @property
    def databar_expanded_wide_bar_separator(self) -> bool: ...
    @property
    def databar_expanded_no_date_preview(self) -> bool: ...
    @property
    def tlc39_asterisk_separator(self) -> bool: ...
    @property
    def tlc39_extended_link_flag(self) -> bool: ...
    @property
    def tlc39_printer_layout(self) -> bool: ...
    @property
    def tlc39_additional_data_byte_capacity(self) -> bool: ...
    @property
    def maxicode_standard_minimum_six_bytes(self) -> bool: ...
    @property
    def maxicode_nul_terminates_data(self) -> bool: ...
    @property
    def maxicode_terminal_latch(self) -> bool: ...
    @property
    def maxicode_printer_run_boundaries(self) -> bool: ...
    @property
    def maxicode_printer_dot_geometry(self) -> bool: ...
    @property
    def maxicode_mode5_preview_omits_data(self) -> bool: ...
    @property
    def data_matrix_default_tilde_escape(self) -> bool: ...
    @property
    def data_matrix_edifact_printer_transitions(self) -> bool: ...
    @property
    def aztec_floor_default_error_correction(self) -> bool: ...
    @property
    def aztec_preserve_binary_runs(self) -> bool: ...
    @property
    def databar_retail_printer_dimensions(self) -> bool: ...
    @property
    def databar_upce_requires_upca_data(self) -> bool: ...
    @property
    def composite_height_in_modules(self) -> bool: ...
    @property
    def composite_linear_quiet_zone(self) -> bool: ...
    @property
    def right_justified_inverted_text_uses_ink_margin(self) -> bool: ...
    @property
    def barcode_interpretation_printer_layout(self) -> bool: ...
    @property
    def barcode_fo_uses_bar_height(self) -> bool: ...
    @property
    def code128_fo_uses_bar_width(self) -> bool: ...
    @property
    def barcode_above_text_keeps_bar_origin(self) -> bool: ...
    @property
    def barcode_reverse_interpretation_shift(self) -> bool: ...
    @property
    def preview_ignores_label_top(self) -> bool: ...
    @property
    def preview_ignores_print_orientation(self) -> bool: ...
    @property
    def preview_ignores_print_mirror(self) -> bool: ...
    @property
    def field_vertical_ignores_gap(self) -> bool: ...
    @property
    def field_direction_printer_anchors(self) -> bool: ...
    @property
    def block_field_direction_printer_layout(self) -> bool: ...
    @property
    def bounded_text_printer_pitch(self) -> bool: ...
    @property
    def bounded_text_printer_anchors(self) -> bool: ...
    @property
    def bounded_text_font_cancels_block(self) -> bool: ...
    @property
    def block_spaces_ignore_character_gap(self) -> bool: ...
    @property
    def codablock_f_fit_rows(self) -> bool: ...
    @property
    def block_center_includes_trailing_space(self) -> bool: ...
    @property
    def codablock_f_row_height_in_dots(self) -> bool: ...
    @property
    def macro_micropdf417_reverse_origin_omits_side_raps(self) -> bool: ...
    @property
    def validation_retail_long_is_short(self) -> bool: ...
    @property
    def validation_legacy_small_is_parameter(self) -> bool: ...
    @property
    def macro_pdf417_file_id(self) -> list[int] | None: ...
    @property
    def qr_model1_extended_versions(self) -> bool: ...
    @property
    def qr_fo_uses_by_height(self) -> bool: ...
    @property
    def qr_ft_includes_margin(self) -> bool: ...
    @property
    def diagonal_dot_runs(self) -> bool: ...
    @property
    def postal_fixed_pitch(self) -> bool: ...
    @property
    def intelligent_mail_outward_rounding(self) -> bool: ...
    @property
    def retail_guard_extension_dots(self) -> int | None: ...
    @property
    def code93_normalize_input(self) -> bool: ...
    @property
    def codablock_a_row_height_in_dots(self) -> bool: ...
    @property
    def codablock_a_wrapping_checks(self) -> bool: ...
    @property
    def code128_above_text_keeps_bar_origin(self) -> bool: ...

class RenderLimits:
    def __init__(
        self,
        *,
        input_bytes: int = ...,
        labels: int = ...,
        segments: int = ...,
        stored_graphic_segments: int = ...,
        pixels: int = ...,
        dimension: int = ...,
        number_abs: float = ...,
        field_bytes: int = ...,
        graphic_bytes: int = ...,
        font_bytes: int = ...,
        stored_formats: int = ...,
        recall_depth: int = ...,
        recall_calls: int = ...,
        coordinate_abs: float = ...,
    ) -> None: ...
    def replace(
        self,
        *,
        input_bytes: int = ...,
        labels: int = ...,
        segments: int = ...,
        stored_graphic_segments: int = ...,
        pixels: int = ...,
        dimension: int = ...,
        number_abs: float = ...,
        field_bytes: int = ...,
        graphic_bytes: int = ...,
        font_bytes: int = ...,
        stored_formats: int = ...,
        recall_depth: int = ...,
        recall_calls: int = ...,
        coordinate_abs: float = ...,
    ) -> RenderLimits: ...
    @property
    def input_bytes(self) -> int: ...
    @property
    def labels(self) -> int: ...
    @property
    def segments(self) -> int: ...
    @property
    def stored_graphic_segments(self) -> int: ...
    @property
    def pixels(self) -> int: ...
    @property
    def dimension(self) -> int: ...
    @property
    def number_abs(self) -> float: ...
    @property
    def field_bytes(self) -> int: ...
    @property
    def graphic_bytes(self) -> int: ...
    @property
    def font_bytes(self) -> int: ...
    @property
    def stored_formats(self) -> int: ...
    @property
    def recall_depth(self) -> int: ...
    @property
    def recall_calls(self) -> int: ...
    @property
    def coordinate_abs(self) -> float: ...

class OutputLimits:
    def __init__(
        self,
        *,
        pixels: int = ...,
        segments: int = ...,
        coordinate_abs: float = ...,
        pages: int = ...,
        flattened_segments: int = ...,
        scan_work: int = ...,
    ) -> None: ...
    def replace(
        self,
        *,
        pixels: int = ...,
        segments: int = ...,
        coordinate_abs: float = ...,
        pages: int = ...,
        flattened_segments: int = ...,
        scan_work: int = ...,
    ) -> OutputLimits: ...
    @property
    def pixels(self) -> int: ...
    @property
    def segments(self) -> int: ...
    @property
    def coordinate_abs(self) -> float: ...
    @property
    def pages(self) -> int: ...
    @property
    def flattened_segments(self) -> int: ...
    @property
    def scan_work(self) -> int: ...

class Syntax:
    def __init__(
        self,
        *,
        format_prefix: int = ...,
        control_prefix: int = ...,
        delimiter: int = ...,
    ) -> None: ...
    def replace(
        self,
        *,
        format_prefix: int = ...,
        control_prefix: int = ...,
        delimiter: int = ...,
    ) -> Syntax: ...
    @property
    def format_prefix(self) -> int: ...
    @property
    def control_prefix(self) -> int: ...
    @property
    def delimiter(self) -> int: ...

class Options:
    def __init__(
        self,
        *,
        profile: Literal["zd621", "specification", "zq610-plus"] = ...,
        width: int | None = ...,
        height: int | None = ...,
        dpi: int | None = ...,
        compatibility: Compatibility | None = ...,
    ) -> None: ...
    @property
    def width(self) -> int: ...
    @property
    def height(self) -> int: ...
    @property
    def dpi(self) -> int: ...
    @property
    def compatibility(self) -> Compatibility: ...

class RenderError(ValueError):
    offset: int
    message: str

class ParseError(ValueError):
    offset: int
    kind: str

class OutputError(ValueError): ...

class Scene:
    @property
    def width(self) -> int: ...
    @property
    def height(self) -> int: ...
    @property
    def dpi(self) -> int: ...
    def png(self, *, limits: OutputLimits | None = ...) -> bytes: ...
    def svg(self, *, limits: OutputLimits | None = ...) -> bytes: ...
    def pdf(self, *, limits: OutputLimits | None = ...) -> bytes: ...
    def rasterize(self, *, limits: OutputLimits | None = ...) -> Raster: ...

class Raster:
    @property
    def width(self) -> int: ...
    @property
    def height(self) -> int: ...
    @property
    def pixels(self) -> bytes: ...

class Document:
    @property
    def labels(self) -> list[Scene]: ...
    @property
    def warnings(self) -> list[str]: ...
    def pdf(self, *, limits: OutputLimits | None = ...) -> bytes: ...

class Element:
    @property
    def kind(
        self,
    ) -> Literal[
        "before_first_command", "format_command", "control_command", "control_character"
    ]: ...
    @property
    def offset(self) -> int: ...
    @property
    def data(self) -> bytes: ...
    def __bytes__(self) -> bytes: ...

class ParseResult:
    @property
    def elements(self) -> list[Element]: ...
    @property
    def syntax(self) -> Syntax: ...

__version__: str
library_version: str

def render(
    input: str | bytes,
    options: Options | None = ...,
    *,
    limits: RenderLimits | None = ...,
) -> Document: ...
def parse(input: str | bytes, *, syntax: Syntax | None = ...) -> ParseResult: ...
