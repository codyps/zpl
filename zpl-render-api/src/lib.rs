//! Byte-oriented rendering for the HTTP service. No printer or persistent state.
//! HTTP compatibility scope: https://labelary.com/service.html sections 1–3, 6.6.
use wasm_bindgen::prelude::*;
use zpl::{
    output::{raster::rasterize, Pdf, Scene},
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
    ..Limits::DEFAULT
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
    let ppm = match dpi {
        152 => 6000,
        203 => 8000,
        304 => 12000,
        609 => 24000,
        _ => unreachable!(),
    };
    match compressed_png(&scene, ppm) {
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

/// LabelZoom v2 ZPL conversion subset. Zero width/height leaves that axis at the
/// source's PW/LL size (4x6-inch fallback); nonzero axes fix the output viewport.
/// PNG returns the first label; PDF contains all labels in source order.
/// https://docs.labelzoom.com/reference/supported-formats/#multi-label-jobs
/// https://docs.labelzoom.com/reference/conversion-parameters/#parameter-reference
#[wasm_bindgen]
pub fn render_labelzoom(
    input: &[u8],
    width: u32,
    height: u32,
    dpi: u32,
    pdf: bool,
) -> RenderResponse {
    if input.len() > MAX_INPUT_BYTES {
        return RenderResponse::error(413, "ZPL exceeds the 1 MiB request limit");
    }
    if !matches!(dpi, 152 | 203 | 300 | 600)
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || width > dpi.saturating_mul(15)
        || height > dpi.saturating_mul(15)
        || u64::from(width) * u64::from(height) > MAX_PIXELS as u64
    {
        return RenderResponse::error(400, "Invalid or excessive label dimensions or density");
    }
    let options = Options {
        width: if width == 0 { dpi * 4 } else { width },
        height: if height == 0 { dpi * 6 } else { height },
        dpi,
        ..SPECIFICATION
    };
    let mut document = match render_with_limits(input, options, LIMITS) {
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
    if labels == 0 {
        return RenderResponse::error(400, "ZPL contains no labels");
    }
    for scene in &mut document.labels {
        if width != 0 {
            scene.width = width;
        }
        if height != 0 {
            scene.height = height;
        }
        // Check combinations of an overridden axis and a source-defined axis
        // before any raster allocation or PDF encoding, including later pages.
        if scene.width > dpi * 15
            || scene.height > dpi * 15
            || scene.width > MAX_DIMENSION
            || scene.height > MAX_DIMENSION
            || u64::from(scene.width) * u64::from(scene.height) > MAX_PIXELS as u64
        {
            return RenderResponse::error(413, "Label exceeds the service canvas limit");
        }
    }
    let width = document.labels[0].width;
    let height = document.labels[0].height;
    let encoded = if pdf {
        Pdf.encode_pages_with_limits(
            &document.labels,
            zpl::output::Limits {
                pixels: MAX_PIXELS,
                segments: LIMITS.segments,
                pages: LIMITS.labels,
                ..zpl::output::Limits::DEFAULT
            },
        )
        .map_err(|error| error.to_string())
    } else {
        document.labels.truncate(1);
        // Exact requested DPI, independent of Labelary's density-class metadata.
        compressed_png(
            &document.labels[0],
            (f64::from(dpi) / 0.0254).round() as u32,
        )
    };
    match encoded {
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

fn compressed_png(scene: &Scene, ppm: u32) -> Result<Vec<u8>, String> {
    let raster = rasterize(scene).map_err(|e| e.to_string())?;
    let mut body = Vec::new();
    {
        // PNG grayscale and pHYs preserve the raster and physical resolution.
        // https://www.w3.org/TR/png-3/#11IHDR and #11pHYs
        let mut encoder = png::Encoder::new(&mut body, scene.width, scene.height);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
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
