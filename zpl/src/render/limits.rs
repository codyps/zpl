//! Configurable resource budgets for ZPL interpretation and scene construction.
use crate::output::{self, Scene};

/// Budgets may be raised or lowered independently. Defaults preserve the bounded
/// library behavior; [`Self::unlimited`] disables resource-policy ceilings.
/// ZPL semantics, finite coordinates, integer overflow and allocation constraints
/// still apply. Output adapters have their own [`output::Limits`].
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Raw input and expanded stored-format bytes, each counted separately.
    pub input_bytes: usize,
    pub labels: usize,
    /// Total path segments across all labels, also bounding temporary graphics.
    pub segments: usize,
    pub stored_graphic_segments: usize,
    /// Pixels per label, including dimensions changed by PW/LL.
    pub pixels: usize,
    pub dimension: u32,
    /// Absolute numeric operand magnitude before command-specific validation.
    pub number_abs: f64,
    /// Decoded bytes per FD/FV/SN field, numbered field or FE expansion.
    pub field_bytes: usize,
    /// Decoded bytes per GF/DG bitmap, checked before decompression.
    pub graphic_bytes: usize,
    /// Total font-download bytes per render, including replacements and bitmap
    /// row/glyph allocation overhead.
    pub font_bytes: usize,
    pub stored_formats: usize,
    pub recall_depth: usize,
    pub recall_calls: usize,
    /// Absolute scene-coordinate magnitude; coordinates must also be finite.
    pub coordinate_abs: f64,
}

impl Default for Limits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Limits {
    pub const DEFAULT: Self = Self {
        input_bytes: 1_048_576,
        labels: 64,
        segments: output::MAX_SEGMENTS,
        stored_graphic_segments: output::MAX_SEGMENTS,
        pixels: output::MAX_PIXELS,
        dimension: u32::MAX,
        number_abs: 1_000_000.,
        field_bytes: 4096,
        graphic_bytes: 25_000,
        font_bytes: 16 * 1024 * 1024,
        stored_formats: 256,
        recall_depth: 8,
        recall_calls: 4096,
        coordinate_abs: 1e9,
    };

    pub const fn unlimited() -> Self {
        Self {
            input_bytes: usize::MAX,
            labels: usize::MAX,
            segments: usize::MAX,
            stored_graphic_segments: usize::MAX,
            pixels: usize::MAX,
            dimension: u32::MAX,
            number_abs: f64::MAX,
            field_bytes: usize::MAX,
            graphic_bytes: usize::MAX,
            font_bytes: usize::MAX,
            stored_formats: usize::MAX,
            recall_depth: usize::MAX,
            recall_calls: usize::MAX,
            coordinate_abs: f64::MAX,
        }
    }

    pub(super) fn scene(self, width: u32, height: u32, dpi: u32) -> Result<Scene, String> {
        if width > self.dimension
            || height > self.dimension
            || u64::from(width) * u64::from(height) > self.pixels as u64
        {
            return Err("canvas exceeds configured renderer limit".into());
        }
        Scene::new_with_limits(
            width,
            height,
            dpi,
            output::Limits {
                pixels: self.pixels,
                segments: self.segments,
                coordinate_abs: self.coordinate_abs,
                ..output::Limits::unlimited()
            },
        )
        .map_err(|e| e.to_string())
    }
}
