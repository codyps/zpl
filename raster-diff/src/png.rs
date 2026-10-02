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
    /// Encode packed one-bit grayscale pixels, most significant bit first.
    /// Each row occupies `width.div_ceil(8)` bytes; unused low bits are ignored.
    /// Zero is black and one is white, as specified by PNG §§7.2 and 11.2.1:
    /// <https://www.w3.org/TR/png-3/#7Scanlines>.
    pub fn encode_mono(
        width: u32,
        height: u32,
        pixels: &[u8],
        dpi: u32,
    ) -> Result<Vec<u8>, ImageError> {
        let size = (width as usize)
            .checked_mul(height as usize)
            .filter(|&n| n > 0 && n <= MAX_PIXELS);
        let stride = width.div_ceil(8) as usize;
        if width == 0
            || height == 0
            || dpi == 0
            || size.is_none()
            || stride.checked_mul(height as usize) != Some(pixels.len())
        {
            return Err(ImageError("invalid or excessive monochrome raster"));
        }
        Ok(encode_gray_rows(width, height, dpi, 1, pixels, stride))
    }

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
        Ok(encode_gray_rows(
            raster.width,
            raster.height,
            dpi,
            8,
            &raster.pixels,
            raster.width as usize,
        ))
    }
}

fn encode_gray_rows(
    width: u32,
    height: u32,
    dpi: u32,
    depth: u8,
    pixels: &[u8],
    stride: usize,
) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[depth, 0, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);
    let ppm = (dpi as f64 / 0.0254).round().min(u32::MAX as f64) as u32;
    let mut phys = Vec::new();
    phys.extend_from_slice(&ppm.to_be_bytes());
    phys.extend_from_slice(&ppm.to_be_bytes());
    phys.push(1);
    chunk(&mut out, b"pHYs", &phys);
    let mut rows = Vec::with_capacity(pixels.len() + height as usize);
    for row in pixels.chunks_exact(stride) {
        rows.push(0);
        rows.extend_from_slice(row);
    }
    chunk(&mut out, b"IDAT", &zlib_store(&rows));
    chunk(&mut out, b"IEND", &[]);
    out
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
    fn monochrome_rows_preserve_bit_order_padding_and_resolution() {
        // PNG §§7.2, 11.2.1: high bit first, each row independently padded.
        // https://www.w3.org/TR/png-3/#7Scanlines
        for width in [1u32, 7, 8, 9, 15, 16, 17, 65] {
            let stride = width.div_ceil(8) as usize;
            let bytes: Vec<_> = (0..stride * 3).map(|i| (i * 37 + 0x95) as u8).collect();
            let png = Png::encode_mono(width, 3, &bytes, 203).unwrap();
            let decoded = Raster::decode_png_with_threshold(&png, None).unwrap();
            assert_eq!((decoded.width, decoded.height), (width, 3));
            for y in 0..3 {
                for x in 0..width as usize {
                    assert_eq!(
                        decoded.pixels[y * width as usize + x],
                        if bytes[y * stride + x / 8] & (128 >> (x % 8)) == 0 {
                            0
                        } else {
                            255
                        }
                    );
                }
            }
            assert_eq!(png[24], 1); // IHDR bit depth
            let offset = png.windows(4).position(|w| w == b"pHYs").unwrap();
            assert_eq!(
                &png[offset + 4..offset + 13],
                &[0, 0, 31, 56, 0, 0, 31, 56, 1]
            );
        }
        for (w, h, bytes, dpi) in [
            (0, 1, &[][..], 203),
            (1, 0, &[][..], 203),
            (1, 1, &[0][..], 0),
            (9, 1, &[0][..], 203),
            (1, 1, &[0, 0][..], 203),
            (u32::MAX, u32::MAX, &[][..], 203),
        ] {
            assert!(Png::encode_mono(w, h, bytes, dpi).is_err());
        }
    }

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
