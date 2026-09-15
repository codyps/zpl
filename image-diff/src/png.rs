use crate::compression::{crc32, zlib_store};
use crate::{ImageError, Raster, MAX_PIXELS};
#[derive(Debug, Default, Clone, Copy)]
pub struct Png;
fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}
impl Png {
    pub fn encode_gray(raster: &Raster, dpi: u32) -> Result<Vec<u8>, ImageError> {
        let size = (raster.width as usize).checked_mul(raster.height as usize);
        if raster.width == 0
            || raster.height == 0
            || dpi == 0
            || size != Some(raster.pixels.len())
            || raster.pixels.len() > MAX_PIXELS
        {
            return Err(ImageError("invalid or excessive grayscale raster"));
        }
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut header = Vec::new();
        header.extend_from_slice(&raster.width.to_be_bytes());
        header.extend_from_slice(&raster.height.to_be_bytes());
        header.extend_from_slice(&[8, 0, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &header);
        let ppm = (dpi as f64 / 0.0254).round().min(u32::MAX as f64) as u32;
        let mut phys = Vec::new();
        phys.extend_from_slice(&ppm.to_be_bytes());
        phys.extend_from_slice(&ppm.to_be_bytes());
        phys.push(1);
        chunk(&mut out, b"pHYs", &phys);
        let mut rows = Vec::with_capacity(raster.pixels.len() + raster.height as usize);
        for row in raster.pixels.chunks_exact(raster.width as usize) {
            rows.push(0);
            rows.extend_from_slice(row);
        }
        chunk(&mut out, b"IDAT", &zlib_store(&rows));
        chunk(&mut out, b"IEND", &[]);
        Ok(out)
    }
}

impl Png {
    /// Encode RGB8 pixels without rasterizing paths (for diagnostics and overlays).
    pub fn encode_rgb(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, ImageError> {
        let size = (width as usize)
            .checked_mul(height as usize)
            .filter(|&n| n > 0 && n <= MAX_PIXELS)
            .ok_or(ImageError("invalid or excessive image dimensions"))?;
        if width == 0 || height == 0 || pixels.len() != size * 3 {
            return Err(ImageError("RGB buffer does not match dimensions"));
        }
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut header = Vec::new();
        header.extend(width.to_be_bytes());
        header.extend(height.to_be_bytes());
        header.extend([8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &header);
        let mut rows = Vec::with_capacity(pixels.len() + height as usize);
        for row in pixels.chunks_exact(width as usize * 3) {
            rows.push(0);
            rows.extend(row);
        }
        chunk(&mut out, b"IDAT", &zlib_store(&rows));
        chunk(&mut out, b"IEND", &[]);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grayscale_round_trip_preserves_pixels_and_resolution() {
        let raster = Raster {
            width: 3,
            height: 2,
            pixels: vec![0, 255, 0, 255, 0, 255],
        };
        let png = Png::encode_gray(&raster, 203).unwrap();
        assert_eq!(
            Raster::decode_png_with_threshold(&png, None).unwrap(),
            raster
        );
        let offset = png.windows(4).position(|w| w == b"pHYs").unwrap();
        assert_eq!(
            &png[offset + 4..offset + 13],
            &[0, 0, 31, 56, 0, 0, 31, 56, 1]
        );
        assert!(Png::encode_gray(&raster, 0).is_err());
        let invalid = Raster {
            width: 3,
            height: 2,
            pixels: vec![0],
        };
        assert!(Png::encode_gray(&invalid, 203).is_err());
        let empty = Raster {
            width: 0,
            height: 0,
            pixels: vec![],
        };
        assert!(Png::encode_gray(&empty, 203).is_err());
    }
}
