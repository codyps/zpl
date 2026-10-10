//! Bounded TrueType outline loading and hint execution for monochrome labels.
//!
//! This original implementation supports quadratic `glyf` fonts. It does not
//! select printer character mappings or silently ignore unsupported instructions.
//! File formats: OpenType `head`, `maxp`, `hhea`, `hmtx`, `cmap`, `loca`, `glyf`:
//! <https://learn.microsoft.com/en-us/typography/opentype/spec/otff>.
//! Hinting: <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions>.

mod hint;
mod read;

const MAX_COORD: i32 = 1 << 24;
fn bounded(value: i64) -> Result<i32> {
    if value.unsigned_abs() > MAX_COORD as u64 {
        Err(invalid("TrueType numeric limit exceeded"))
    } else {
        Ok(value as i32)
    }
}

pub use read::Font;
use std::{error, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;
fn invalid(message: &str) -> Error {
    Error(message.into())
}

/// Independent device sizes in dots per em. Zero dimensions are not accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub x: u16,
    pub y: u16,
}
impl Size {
    pub fn new(x: u16, y: u16) -> Result<Self> {
        if x == 0 || y == 0 || x > 4096 || y > 4096 {
            return Err(invalid("TrueType size must be in 1..=4096 dots per em"));
        }
        Ok(Self { x, y })
    }
}

/// Coordinates use signed 26.6 pixels, Y up, relative to the glyph baseline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
    pub on_curve: bool,
}

#[derive(Clone, Debug)]
pub struct Outline {
    pub contours: Vec<Vec<Point>>,
    /// Advance from the hinted phantom points, in 26.6 dots.
    pub advance: i32,
    /// Scaled `hmtx` advance before instructions, in 26.6 dots.
    pub linear_advance: i32,
    /// TrueType SCANCTRL / SCANTYPE state, retained for the scan converter.
    pub scan_control: u16,
    pub scan_type: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hinting {
    None,
    Native,
}

/// Rendering environment visible to font programs through GETINFO.
/// Device compatibility is explicit; selecting it does not assert font parity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Environment {
    #[default]
    Standard,
    /// October 2, 2026 constructed-font observations on ZD621 V93.21.33Z and
    /// ZQ610 Plus V100.21.21Z: reported rotation false; stretch only in N/I.
    Zebra203 { quarter_turns: u8 },
    /// Measured downloaded-TTF scaling on ZD621 V93.21.33Z (203 DPI).
    /// Uses one point-size quantization rule for all sizes, not a strike table.
    Zd621V93 { quarter_turns: u8 },
}

impl Environment {
    pub(super) fn cvt_axis(self, size: Size) -> u16 {
        if matches!(self, Self::Zd621V93 { .. }) {
            size.x.max(size.y)
        } else {
            size.y
        }
    }

    pub(super) fn cvt_ratio(self, dots: u16, base: u16) -> f64 {
        let ratio = self.ppem(dots, false) / self.ppem(base, false);
        if matches!(self, Self::Zd621V93 { .. }) {
            // Fractional RCVT witnesses on both axes: projection uses a 16.16
            // matrix after scaling the CVT at the larger device dimension.
            // See docs/font-small-sizes.md, "CVT scaling correction", and the
            // cvt-axis-validation-v2-20261003 printer-capture fixtures.
            (ratio * 65536.).round() / 65536.
        } else {
            ratio
        }
    }

    pub(super) fn ppem(self, dots: u16, layout: bool) -> f64 {
        if matches!(self, Self::Zd621V93 { .. }) {
            // Native controlled-Heros advances distinguish continuous layout
            // scaling from the earlier ceil-to-sixteenths hypothesis. The shared
            // 204/203 ratio preserves all constructed-font spacing witnesses and
            // predicts H at 16, j/space at 24, and W/m at 65 and 96 dots.
            // This is an empirical rule, not a claim about firmware internals.
            // See tests/fixtures/external-fonts-zd621-v1/README.md.
            if layout {
                return f64::from(dots.max(10)) * 204. / 203.;
            }
            // Font-program distances retain the calibrated sixteenths-of-a-
            // point size and the 8-dot/mm matrix rounded up to 16.16:
            // ceil((203.2 / 72) * 65536) == 184958. This shared model predicts
            // the 1..64 sweep; see docs/swiss-font-rendering.md and fixtures.
            // Point/CVT coordinates use the two rounded coefficients in scale().
            let numerator = u32::from(dots.max(10)) * 1152;
            let points = numerator / 203;
            f64::from(points) * 184958. / (16. * 65536.)
        } else {
            f64::from(dots)
        }
    }
    pub(super) fn scale(self, value: i32, dots: u16, units: u16) -> Result<i32> {
        if matches!(self, Self::Zd621V93 { .. }) {
            let points = i64::from(dots.max(10)) * 1152 / 203;
            // The native scaler retains a rational multiplier when the point
            // size cancels the DPI denominator through a binary shift. Its
            // numerator is 508 * 32; the denominator must remain integral.
            // GC, RCVT and WCVTF captures distinguish this from always rounding
            // a 16.16 multiplier, including odd em sizes and signed half-ties.
            // See tests/fixtures/truetype-raster-zd621-v1/README.md.
            let binary_scale = (points / 45) as u64;
            if points % 45 == 0
                && binary_scale.is_power_of_two()
                && (u64::from(units) * 32) % binary_scale == 0
            {
                let denominator = (u64::from(units) * 32 / binary_scale) as i32;
                // The shift path uses signed arithmetic (ties toward +infinity).
                // Division rounds the magnitude, then restores the input sign.
                // Large WCVTF witnesses also expose a wrapping 32-bit product
                // and rounding addition; keep wrapping explicit and bounded.
                let result = if (denominator as u32).is_power_of_two() {
                    value.wrapping_mul(16_256).wrapping_add(denominator / 2)
                        >> denominator.trailing_zeros()
                } else {
                    value
                        .wrapping_abs()
                        .wrapping_mul(16_256)
                        .wrapping_add(denominator / 2)
                        / denominator
                        * value.signum()
                };
                return bounded(i64::from(result));
            }
            // Two rounded coefficients, not one floating-point multiplication:
            // sixteenths of a point -> 16.16 ppem at 203.2 DPI -> 16.16
            // multiplier for 26.6 coordinates. Rounding only the DPI matrix
            // moves the stretched controlled-font W by one pixel. Rounding
            // only the final multiplier fails the 1024-unit 32-dot GC witness.
            // See tests/fixtures/truetype-raster-zd621-v1/README.md. This is
            // an empirical device rule, not a TrueType specification requirement.
            let ppem = (points * 508 * 65_536 + 1440) / 2880;
            let coefficient = (ppem * 64 + i64::from(units) / 2) / i64::from(units);
            let result = (i64::from(value).abs() * coefficient + 32_768) / 65_536;
            return bounded(result * i64::from(value.signum()));
        }
        bounded((f64::from(value) * self.ppem(dots, false) * 64. / f64::from(units)).round() as i64)
    }
}

pub struct Instance<'a> {
    font: &'a Font<'a>,
    size: Size,
    hinting: Hinting,
    environment: Environment,
    vm: hint::Vm,
}
impl<'a> Instance<'a> {
    /// Horizontal hmtx spacing, independent of glyph phantom-point instructions.
    /// The ZD621 profile applies the separately measured layout scale. This does
    /// not implement ZPL character mapping, shaping, or field layout.
    pub fn layout_advance(&self, glyph: u16) -> Result<u32> {
        let (advance, _) = self.font.metric(glyph)?;
        let dots = (f64::from(advance) * self.environment.ppem(self.size.x, true)
            / f64::from(self.font.units))
        .round();
        if !(0. ..=4096.).contains(&dots) {
            return Err(invalid("TrueType layout advance limit exceeded"));
        }
        Ok(dots as u32)
    }
    pub fn outline(&self, glyph: u16) -> Result<Outline> {
        let (mut outline, origin) =
            self.font
                .outline(self, glyph, &mut Vec::new(), &mut read::Budget::default())?;
        // Normalize the baseline origin once, after all components are placed.
        for point in outline.contours.iter_mut().flatten() {
            point.x = bounded(point.x as i64 - origin as i64)?;
        }
        Ok(outline)
    }
    pub fn glyph(&self, character: char) -> Result<Outline> {
        let glyph = self
            .font
            .glyph_index(character)
            .ok_or_else(|| invalid("character absent from TrueType cmap"))?;
        self.outline(glyph)
    }
}
