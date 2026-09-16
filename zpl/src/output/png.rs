use super::*;
use crate::output::raster::rasterize;
#[derive(Debug, Default, Clone, Copy)]
pub struct Png;
impl Adapter for Png {
    fn encode(&self, scene: &Scene) -> Result<Vec<u8>, OutputError> {
        raster_diff::Png::encode_gray(&rasterize(scene)?, scene.dpi).map_err(|e| OutputError(e.0))
    }
}
impl Png {
    pub fn encode_rgb(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, OutputError> {
        raster_diff::Png::encode_rgb(width, height, pixels).map_err(|e| OutputError(e.0))
    }
}
