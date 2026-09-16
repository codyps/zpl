//! Directional, pixel-exact comparison of binary rasters. No alignment or resizing.
use crate::{Png, Raster, MAX_PIXELS};
pub const REFERENCE_ONLY: [u8; 3] = [216, 27, 96];
pub const CANDIDATE_ONLY: [u8; 3] = [0, 166, 214];
pub const BOTH_BLACK: [u8; 3] = [64, 64, 64];
pub const BOTH_WHITE: [u8; 3] = [255, 255, 255];
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    pub width: u32,
    pub height: u32,
    pub dimensions_match: bool,
    pub reference_only: usize,
    pub candidate_only: usize,
    pub both_black: usize,
    pub both_white: usize,
    pub bounds: Option<Bounds>,
    pub pixels: Vec<u8>,
}
impl Diff {
    pub fn different_pixels(&self) -> usize {
        self.reference_only + self.candidate_only
    }
    pub fn matches(&self) -> bool {
        self.dimensions_match && self.different_pixels() == 0
    }
    /// Intersection over union of black pixels. Two blank images have IoU 1.
    pub fn ink_iou(&self) -> f64 {
        let union = self.both_black + self.different_pixels();
        if union == 0 {
            1.
        } else {
            self.both_black as f64 / union as f64
        }
    }
    /// Nearest-neighbor scaling preserves individual diff pixels.
    pub fn png(&self, scale: u32) -> Result<Vec<u8>, String> {
        let w = self
            .width
            .checked_mul(scale)
            .ok_or("scaled width overflows")?;
        let h = self
            .height
            .checked_mul(scale)
            .ok_or("scaled height overflows")?;
        if w == 0
            || h == 0
            || scale == 0
            || (w as usize)
                .checked_mul(h as usize)
                .is_none_or(|n| n > MAX_PIXELS)
        {
            return Err("invalid or excessive output scale".into());
        }
        if self.pixels.len() != (self.width as usize * self.height as usize) * 3 {
            return Err("invalid diff buffer".into());
        }
        if scale == 1 {
            return Png::encode_rgb(w, h, &self.pixels).map_err(|e| e.to_string());
        }
        let mut pixels = Vec::with_capacity(w as usize * h as usize * 3);
        for row in self.pixels.chunks_exact(self.width as usize * 3) {
            for _ in 0..scale {
                for pixel in row.chunks_exact(3) {
                    for _ in 0..scale {
                        pixels.extend_from_slice(pixel)
                    }
                }
            }
        }
        Png::encode_rgb(w, h, &pixels).map_err(|e| e.to_string())
    }
}
/// Compare at the same top-left origin. `pad` explicitly permits a white canvas
/// outside either image; dimensions still count as different for `matches()`.
pub fn compare(reference: &Raster, candidate: &Raster, pad: bool) -> Result<Diff, String> {
    for r in [reference, candidate] {
        if r.width == 0
            || r.height == 0
            || (r.width as usize).checked_mul(r.height as usize) != Some(r.pixels.len())
            || r.pixels.iter().any(|&p| p != 0 && p != 255)
        {
            return Err("comparison requires valid binary rasters".into());
        }
    }
    let dimensions_match =
        (reference.width, reference.height) == (candidate.width, candidate.height);
    if !dimensions_match && !pad {
        return Err("image dimensions differ; use --pad to compare on a white canvas".into());
    }
    let width = reference.width.max(candidate.width);
    let height = reference.height.max(candidate.height);
    let size = (width as usize)
        .checked_mul(height as usize)
        .filter(|&n| n <= MAX_PIXELS)
        .ok_or("diff canvas exceeds image limit")?;
    let mut d = Diff {
        width,
        height,
        dimensions_match,
        reference_only: 0,
        candidate_only: 0,
        both_black: 0,
        both_white: 0,
        bounds: None,
        pixels: Vec::with_capacity(size * 3),
    };
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (width, height, 0, 0);
    for y in 0..height {
        for x in 0..width {
            let black = |r: &Raster| {
                x < r.width
                    && y < r.height
                    && r.pixels[y as usize * r.width as usize + x as usize] == 0
            };
            let (a, b) = (black(reference), black(candidate));
            let color = match (a, b) {
                (true, false) => {
                    d.reference_only += 1;
                    REFERENCE_ONLY
                }
                (false, true) => {
                    d.candidate_only += 1;
                    CANDIDATE_ONLY
                }
                (true, true) => {
                    d.both_black += 1;
                    BOTH_BLACK
                }
                (false, false) => {
                    d.both_white += 1;
                    BOTH_WHITE
                }
            };
            d.pixels.extend(color);
            if a != b {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    if d.different_pixels() > 0 {
        d.bounds = Some(Bounds {
            x: min_x,
            y: min_y,
            width: max_x - min_x + 1,
            height: max_y - min_y + 1,
        })
    }
    Ok(d)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn raster(w: u32, p: &[u8]) -> Raster {
        Raster {
            width: w,
            height: p.len() as u32 / w,
            pixels: p.to_vec(),
        }
    }
    #[test]
    fn directional_colors_counts_and_swap() {
        let a = raster(2, &[0, 0, 255, 255]);
        let b = raster(2, &[0, 255, 0, 255]);
        let d = compare(&a, &b, false).unwrap();
        assert_eq!(
            (
                d.both_black,
                d.reference_only,
                d.candidate_only,
                d.both_white
            ),
            (1, 1, 1, 1)
        );
        assert_eq!(
            d.pixels,
            [BOTH_BLACK, REFERENCE_ONLY, CANDIDATE_ONLY, BOTH_WHITE].concat()
        );
        assert_eq!(
            d.bounds,
            Some(Bounds {
                x: 0,
                y: 0,
                width: 2,
                height: 2
            })
        );
        assert_eq!(d.ink_iou(), 1. / 3.);
        let swapped = compare(&b, &a, false).unwrap();
        assert_eq!(&swapped.pixels[3..6], &CANDIDATE_ONLY);
        assert!(!d.matches());
    }
    #[test]
    fn equality_blank_and_dimensions() {
        let blank = raster(2, &[255; 4]);
        let d = compare(&blank, &blank, false).unwrap();
        assert!(d.matches());
        assert_eq!(d.bounds, None);
        assert_eq!(d.ink_iou(), 1.);
        let bigger = raster(3, &[255; 6]);
        assert!(compare(&blank, &bigger, false).is_err());
        let d = compare(&blank, &bigger, true).unwrap();
        assert_eq!(d.different_pixels(), 0);
        assert!(!d.matches());
        let black = raster(3, &[255, 255, 0, 255, 255, 0]);
        let d = compare(&blank, &black, true).unwrap();
        assert_eq!(d.candidate_only, 2);
        assert_eq!(
            d.bounds,
            Some(Bounds {
                x: 2,
                y: 0,
                width: 1,
                height: 2
            })
        );
    }
    #[test]
    fn invalid_inputs_and_scaling() {
        let bad = raster(1, &[127]);
        assert!(compare(&bad, &bad, false).is_err());
        let a = raster(1, &[0]);
        let d = compare(&a, &a, false).unwrap();
        assert!(d.png(0).is_err());
        assert!(d.png(u32::MAX).is_err());
        let png = d.png(3).unwrap();
        let r = Raster::decode_png(&png).unwrap();
        assert_eq!((r.width, r.height), (3, 3));
        assert_eq!(r.pixels, vec![0; 9]);
    }
}
