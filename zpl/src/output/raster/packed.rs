//! One-bit destination for PNG; spans use the same scan converter as `Raster`.
use super::{OutputError, Paint, Range, RasterOutput};

#[derive(Default)]
pub(crate) struct PackedRaster {
    pub(crate) pixels: Vec<u8>,
    width: u32,
    height: u32,
    stride: usize,
}

impl RasterOutput for PackedRaster {
    fn reset(&mut self, width: u32, height: u32) -> Result<(), OutputError> {
        let stride = width.div_ceil(8) as usize;
        let len = stride
            .checked_mul(height as usize)
            .filter(|_| width != 0 && height != 0)
            .ok_or(OutputError("invalid or excessive image dimensions"))?;
        self.pixels
            .try_reserve(len.saturating_sub(self.pixels.len()))
            .map_err(|_| OutputError("raster allocation failed"))?;
        self.width = width;
        self.height = height;
        self.stride = stride;
        self.pixels.resize(len, 255);
        self.pixels.fill(255);
        Ok(())
    }

    fn paint_span(&mut self, y: u32, x: Range<u32>, paint: Paint) -> Result<(), OutputError> {
        if y >= self.height || x.start >= x.end || x.end > self.width {
            return Err(OutputError("invalid raster span"));
        }
        let row = &mut self.pixels[y as usize * self.stride..][..self.stride];
        let first = (x.start / 8) as usize;
        let last = ((x.end - 1) / 8) as usize;
        // PNG §7.2 packs pixels from the most significant bit of each byte.
        // https://www.w3.org/TR/png-3/#7Scanlines
        let start_mask = 255 >> (x.start % 8);
        let end_mask = 255 << (7 - (x.end - 1) % 8);
        let apply = |byte: &mut u8, mask: u8| match paint {
            Paint::Black => *byte &= !mask,
            Paint::White => *byte |= mask,
            Paint::Invert => *byte ^= mask,
        };
        if first == last {
            apply(&mut row[first], start_mask & end_mask);
        } else {
            apply(&mut row[first], start_mask);
            apply(&mut row[last], end_mask);
            let middle = &mut row[first + 1..last];
            match paint {
                Paint::Black => middle.fill(0),
                Paint::White => middle.fill(255),
                Paint::Invert => middle.iter_mut().for_each(|byte| *byte = !*byte),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::raster::Raster;

    #[test]
    fn spans_match_unpacked_compositing_at_every_bit_boundary() {
        // PNG §7.2 row packing; span/compositing contract in docs/local-renderer.md.
        for width in 1..=33 {
            let mut packed = PackedRaster::default();
            let mut expanded = Raster {
                width: 0,
                height: 0,
                pixels: Vec::new(),
            };
            packed.reset(width, 2).unwrap();
            expanded.reset(width, 2).unwrap();
            for start in 0..width {
                for end in start + 1..=width {
                    for paint in [
                        Paint::Black,
                        Paint::Invert,
                        Paint::Invert,
                        Paint::White,
                        Paint::Invert,
                    ] {
                        let y = (start + end) % 2;
                        packed.paint_span(y, start..end, paint).unwrap();
                        expanded.paint_span(y, start..end, paint).unwrap();
                        for y in 0..2 {
                            for x in 0..width {
                                let bit = packed.pixels[y * packed.stride + x as usize / 8]
                                    & (128 >> (x % 8));
                                assert_eq!(
                                    if bit == 0 { 0 } else { 255 },
                                    expanded.pixels[y * width as usize + x as usize]
                                );
                            }
                        }
                    }
                }
            }
            packed.reset(width, 2).unwrap();
            assert!(packed.pixels.iter().all(|&byte| byte == 255));
        }
    }
}
