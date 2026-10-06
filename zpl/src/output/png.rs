use super::*;
use crate::output::raster::{rasterize_into_with_limits, PackedRaster};
#[derive(Debug, Default, Clone, Copy)]
pub struct Png;
impl Adapter for Png {
    fn encode(&self, scene: &Scene) -> Result<Vec<u8>, OutputError> {
        self.encode_with_limits(scene, Limits::default())
    }
}
impl Png {
    /// Encode with caller-selected output budgets.
    pub fn encode_with_limits(
        &self,
        scene: &Scene,
        limits: Limits,
    ) -> Result<Vec<u8>, OutputError> {
        // PNG §11.2.2: dimensions are limited to 2^31-1 by the file format.
        // https://www.w3.org/TR/png-3/#11IHDR
        if scene.width > i32::MAX as u32 || scene.height > i32::MAX as u32 {
            return Err(OutputError("PNG dimensions must be at most 2^31-1"));
        }
        let mut raster = PackedRaster::default();
        rasterize_into_with_limits(scene, &mut raster, limits)?;
        raster_diff::Png::encode_mono_with_limit(
            scene.width,
            scene.height,
            &raster.pixels,
            scene.dpi,
            limits.pixels,
        )
        .map_err(|e| OutputError(e.0))
    }
}
impl Png {
    pub fn encode_rgb(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, OutputError> {
        raster_diff::Png::encode_rgb(width, height, pixels).map_err(|e| OutputError(e.0))
    }
}
