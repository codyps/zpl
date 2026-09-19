//! Initial rendering options for identified printers, not runtime device modes.
use super::{compatibility::Compatibility, Options};

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
