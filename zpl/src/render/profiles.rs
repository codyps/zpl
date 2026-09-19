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
        qr_fo_uses_by_height: false,
        qr_ft_includes_margin: false,
        diagonal_dot_runs: false,
        postal_fixed_pitch: false,
        intelligent_mail_outward_rounding: false,
        retail_guard_extension_dots: None,
        code93_normalize_input: false,
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
        qr_fo_uses_by_height: true,
        qr_ft_includes_margin: true,
        diagonal_dot_runs: true,
        postal_fixed_pitch: true,
        intelligent_mail_outward_rounding: true,
        retail_guard_extension_dots: Some(13),
        code93_normalize_input: true,
    },
};
