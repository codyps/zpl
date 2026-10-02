//! Byte-oriented rendering for the HTTP service. No printer or persistent state.
//! HTTP compatibility scope: https://labelary.com/service.html sections 1–3, 6.6.
use wasm_bindgen::prelude::*;
use zpl::{
    output::{raster::rasterize, Scene},
    render::{profiles::SPECIFICATION, render_with_limits, Limits},
    Options,
};

pub const MAX_INPUT_BYTES: usize = 1_048_576;
pub const MAX_PIXELS: usize = 10_000_000;
pub const MAX_DIMENSION: u32 = 4096;
pub const LIMITS: Limits = Limits {
    input_bytes: MAX_INPUT_BYTES,
    labels: 50,
    segments: 100_000,
    stored_graphic_segments: 100_000,
    pixels: MAX_PIXELS,
    dimension: MAX_DIMENSION,
};

#[wasm_bindgen]
pub struct RenderResponse {
    pub status: u16,
    pub labels: u32,
    pub width: u32,
    pub height: u32,
    pub warning_count: u32,
    body: Vec<u8>,
}

#[wasm_bindgen]
impl RenderResponse {
    /// Transfer the body once; do not retain a second copy in Wasm.
    pub fn take_body(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.body)
    }
}

impl RenderResponse {
    fn error(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            labels: 0,
            width: 0,
            height: 0,
            warning_count: 0,
            body: message.into().into_bytes(),
        }
    }
}

#[wasm_bindgen]
pub fn library_version() -> String {
    zpl::version::VERSION.into()
}

/// The URL selects a fixed output viewport. PW/LL still affect ZPL layout.
/// Density names follow Labelary's printer classes: 6/8/12/24 dpmm.
#[wasm_bindgen]
pub fn render_png(input: &[u8], width: u32, height: u32, dpi: u32, index: u32) -> RenderResponse {
    if input.len() > MAX_INPUT_BYTES {
        return RenderResponse::error(413, "ZPL exceeds the 1 MiB request limit");
    }
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_PIXELS as u64
        || !matches!(dpi, 152 | 203 | 304 | 609)
    {
        return RenderResponse::error(400, "Invalid or excessive label dimensions or density");
    }
    let options = Options {
        width,
        height,
        dpi,
        ..SPECIFICATION
    };
    let document = match render_with_limits(input, options, LIMITS) {
        Ok(document) => document,
        Err(error) => {
            let status = if error.message.contains("limit")
                || error.message.contains("too many labels")
                || error.message.contains("budget")
            {
                413
            } else {
                400
            };
            return RenderResponse::error(status, error.to_string());
        }
    };
    let labels = document.labels.len() as u32;
    let warning_count = document.warnings.len() as u32;
    // Drop all unselected scenes before rasterization and compression.
    let Some(mut scene) = document.labels.into_iter().nth(index as usize) else {
        let mut response = RenderResponse::error(404, "No label at this index");
        response.labels = labels;
        return response;
    };
    scene.width = width;
    scene.height = height;
    match compressed_png(&scene) {
        Ok(body) => RenderResponse {
            status: 200,
            labels,
            width,
            height,
            warning_count,
            body,
        },
        Err(error) => RenderResponse::error(413, error),
    }
}

fn compressed_png(scene: &Scene) -> Result<Vec<u8>, String> {
    let raster = rasterize(scene).map_err(|e| e.to_string())?;
    let mut body = Vec::new();
    {
        // PNG grayscale and pHYs preserve the raster and physical resolution.
        // https://www.w3.org/TR/png-3/#11IHDR and #11pHYs
        let mut encoder = png::Encoder::new(&mut body, scene.width, scene.height);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        // Labelary's PNG pHYs uses the requested dpmm even though its canvas
        // dimensions use truncated integer DPI. See docs/worker-api.md.
        let ppm = match scene.dpi {
            152 => 6000,
            203 => 8000,
            304 => 12000,
            609 => 24000,
            _ => return Err("Invalid service density".into()),
        };
        encoder.set_pixel_dims(Some(png::PixelDimensions {
            xppu: ppm,
            yppu: ppm,
            unit: png::Unit::Meter,
        }));
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer
            .write_image_data(&raster.pixels)
            .map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
    }
    Ok(body)
}

#[cfg(test)]
mod tests;
