use super::*;
use crate::output::raster::{rasterize_into, PackedRaster};
#[derive(Debug, Default, Clone, Copy)]
pub struct Png;
impl Adapter for Png {
    fn encode(&self, scene: &Scene) -> Result<Vec<u8>, OutputError> {
        let mut raster = PackedRaster::default();
        rasterize_into(scene, &mut raster)?;
        raster_diff::Png::encode_mono(scene.width, scene.height, &raster.pixels, scene.dpi)
            .map_err(|e| OutputError(e.0))
    }
}
impl Png {
    pub fn encode_rgb(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, OutputError> {
        raster_diff::Png::encode_rgb(width, height, pixels).map_err(|e| OutputError(e.0))
    }
}
