//! Optional service budgets. They can tighten, but never raise, renderer limits.
use crate::output::{Scene, MAX_PIXELS, MAX_SEGMENTS};

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub input_bytes: usize,
    pub labels: usize,
    pub segments: usize,
    pub stored_graphic_segments: usize,
    pub pixels: usize,
    pub dimension: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            input_bytes: 1_048_576,
            labels: 64,
            segments: MAX_SEGMENTS,
            stored_graphic_segments: MAX_SEGMENTS,
            pixels: MAX_PIXELS,
            dimension: u32::MAX,
        }
    }
}

impl Limits {
    pub(super) fn bounded(self) -> Self {
        let max = Self::default();
        Self {
            input_bytes: self.input_bytes.min(max.input_bytes),
            labels: self.labels.min(max.labels),
            segments: self.segments.min(max.segments),
            stored_graphic_segments: self
                .stored_graphic_segments
                .min(max.stored_graphic_segments),
            pixels: self.pixels.min(max.pixels),
            dimension: self.dimension,
        }
    }

    pub(super) fn scene(self, width: u32, height: u32, dpi: u32) -> Result<Scene, String> {
        let scene = Scene::new(width, height, dpi).map_err(|e| e.to_string())?;
        if width > self.dimension
            || height > self.dimension
            || u64::from(width) * u64::from(height) > self.pixels as u64
        {
            return Err("canvas exceeds configured renderer limit".into());
        }
        Ok(scene)
    }
}
