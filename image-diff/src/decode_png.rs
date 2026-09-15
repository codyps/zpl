use super::Raster;
use crate::compression::{crc32, inflate};
const LIMIT: usize = 16 * 1024 * 1024;
fn be(data: &[u8]) -> u32 {
    u32::from_be_bytes(data.try_into().unwrap())
}
impl Raster {
    /// Decode bounded, noninterlaced PNG and threshold against white at 50%.
    /// Supports grayscale/indexed 1/2/4/8-bit and RGB/alpha 8-bit images.
    pub fn decode_png(data: &[u8]) -> Result<Self, String> {
        Self::decode_png_with_threshold(data, Some(128))
    }
    /// `None` requires exactly black/white pixels after compositing alpha on white.
    /// `Some(t)` explicitly thresholds luminance (black when less than `t`).
    pub fn decode_png_with_threshold(data: &[u8], threshold: Option<u8>) -> Result<Self, String> {
        if data.len() > LIMIT || !data.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err("invalid or excessive PNG".into());
        }
        let (mut pos, mut header, mut palette, mut alpha, mut compressed, mut ended) =
            (8, None, Vec::new(), Vec::new(), Vec::new(), false);
        while pos < data.len() {
            if data.len() - pos < 12 {
                return Err("truncated PNG chunk".into());
            }
            let size = be(&data[pos..pos + 4]) as usize;
            if size > data.len() - pos - 12 {
                return Err("truncated PNG data".into());
            }
            let kind = &data[pos + 4..pos + 8];
            let body = &data[pos + 8..pos + 8 + size];
            if crc32(&data[pos + 4..pos + 8 + size]) != be(&data[pos + 8 + size..pos + 12 + size]) {
                return Err("PNG CRC mismatch".into());
            }
            if header.is_none() && kind != b"IHDR" {
                return Err("PNG must begin with IHDR".into());
            }
            match kind {
                b"IHDR" => {
                    if header.is_some() || size != 13 {
                        return Err("invalid PNG header".into());
                    }
                    header = Some(body.to_vec());
                }
                b"PLTE" => {
                    if size == 0 || size > 768 || !size.is_multiple_of(3) {
                        return Err("invalid PNG palette".into());
                    }
                    palette = body.to_vec();
                }
                b"tRNS" => alpha = body.to_vec(),
                b"IDAT" => compressed.extend_from_slice(body),
                b"IEND" => {
                    if size != 0 || pos + 12 != data.len() {
                        return Err("invalid PNG end".into());
                    }
                    ended = true;
                    break;
                }
                _ if kind[0] & 32 == 0 => return Err("unsupported critical PNG chunk".into()),
                _ => {}
            }
            pos += size + 12;
        }
        if !ended {
            return Err("incomplete PNG".into());
        }
        let h = header.ok_or("missing PNG header")?;
        let (w, height, depth, color) = (
            be(&h[..4]) as usize,
            be(&h[4..8]) as usize,
            h[8] as usize,
            h[9],
        );
        let channels = match color {
            0 | 3 => 1,
            2 => 3,
            4 => 2,
            6 => 4,
            _ => return Err("unsupported PNG color".into()),
        };
        if w == 0
            || height == 0
            || w.checked_mul(height).is_none_or(|v| v > LIMIT)
            || !matches!(depth, 1 | 2 | 4 | 8)
            || (matches!(color, 2 | 4 | 6) && depth != 8)
            || h[10..] != [0, 0, 0]
        {
            return Err("unsupported PNG format or dimensions".into());
        }
        let stride = (w * channels * depth).div_ceil(8);
        let bpp = (channels * depth).div_ceil(8).max(1);
        let size = (stride + 1) * height;
        let raw = inflate(&compressed, size)?;
        if raw.len() != size {
            return Err("PNG decompressed length mismatch".into());
        }
        if !alpha.is_empty()
            && match color {
                0 => alpha.len() != 2,
                2 => alpha.len() != 6,
                3 => alpha.len() > palette.len() / 3,
                _ => true,
            }
        {
            return Err("invalid PNG transparency".into());
        }
        let mut pixels = vec![255; w * height];
        let mut previous = vec![0u8; stride];
        for y in 0..height {
            let start = y * (stride + 1);
            let filter = raw[start];
            let mut row = raw[start + 1..start + 1 + stride].to_vec();
            for i in 0..stride {
                let (a, b, c) = (
                    if i >= bpp { row[i - bpp] } else { 0 },
                    previous[i],
                    if i >= bpp { previous[i - bpp] } else { 0 },
                );
                let predictor = match filter {
                    0 => 0,
                    1 => a,
                    2 => b,
                    3 => ((u16::from(a) + u16::from(b)) / 2) as u8,
                    4 => {
                        let p = i32::from(a) + i32::from(b) - i32::from(c);
                        let (pa, pb, pc) = (
                            (p - i32::from(a)).abs(),
                            (p - i32::from(b)).abs(),
                            (p - i32::from(c)).abs(),
                        );
                        if pa <= pb && pa <= pc {
                            a
                        } else if pb <= pc {
                            b
                        } else {
                            c
                        }
                    }
                    _ => return Err("invalid PNG filter".into()),
                };
                row[i] = row[i].wrapping_add(predictor);
            }
            for x in 0..w {
                let sample = if depth < 8 {
                    (row[x * depth / 8] >> (8 - depth - x * depth % 8)) & ((1 << depth) - 1)
                } else {
                    row[x * channels]
                };
                let mut a = 255u32;
                let (r, g, b) = match color {
                    0 | 4 => {
                        let v = u32::from(sample) * 255 / ((1 << depth) - 1);
                        if color == 4 {
                            a = u32::from(row[x * channels + 1])
                        } else if alpha.len() == 2
                            && u16::from(sample) == u16::from_be_bytes([alpha[0], alpha[1]])
                        {
                            a = 0
                        }
                        (v, v, v)
                    }
                    3 => {
                        let i = sample as usize;
                        if i * 3 + 2 >= palette.len() {
                            return Err("missing palette entry".into());
                        }
                        a = u32::from(alpha.get(i).copied().unwrap_or(255));
                        (
                            u32::from(palette[i * 3]),
                            u32::from(palette[i * 3 + 1]),
                            u32::from(palette[i * 3 + 2]),
                        )
                    }
                    _ => {
                        let v = &row[x * channels..];
                        if color == 6 {
                            a = u32::from(v[3])
                        } else if alpha.len() == 6
                            && (0..3).all(|i| {
                                u16::from(v[i])
                                    == u16::from_be_bytes([alpha[i * 2], alpha[i * 2 + 1]])
                            })
                        {
                            a = 0
                        }
                        (u32::from(v[0]), u32::from(v[1]), u32::from(v[2]))
                    }
                };
                let rgb = [r, g, b].map(|v| v * a + 255 * (255 - a));
                pixels[y * w + x] = match threshold {
                    Some(t) => {
                        let l = (299 * r + 587 * g + 114 * b) / 1000;
                        if l * a + 255 * (255 - a) < u32::from(t) * 255 {
                            0
                        } else {
                            255
                        }
                    }
                    None if rgb == [0; 3] => 0,
                    None if rgb == [255 * 255; 3] => 255,
                    None => return Err(
                        "image contains non-binary pixels; use an explicit threshold to binarize"
                            .into(),
                    ),
                };
            }
            previous = row;
        }
        Ok(Self {
            width: w as u32,
            height: height as u32,
            pixels,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compression::zlib_store;
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
        out.extend((body.len() as u32).to_be_bytes());
        out.extend(kind);
        out.extend(body);
        let mut bytes = kind.to_vec();
        bytes.extend(body);
        out.extend(crc32(&bytes).to_be_bytes());
    }
    fn png(
        w: u32,
        h: u32,
        depth: u8,
        color: u8,
        rows: &[u8],
        extra: &[(&[u8; 4], &[u8])],
    ) -> Vec<u8> {
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut header = w.to_be_bytes().to_vec();
        header.extend(h.to_be_bytes());
        header.extend([depth, color, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &header);
        for (k, v) in extra {
            chunk(&mut out, k, v)
        }
        chunk(&mut out, b"IDAT", &zlib_store(rows));
        chunk(&mut out, b"IEND", &[]);
        out
    }
    #[test]
    fn packed_samples_palette_and_transparency() {
        let a = png(3, 1, 1, 0, &[0, 0xa0], &[]);
        assert_eq!(
            Raster::decode_png_with_threshold(&a, None).unwrap().pixels,
            [255, 0, 255]
        );
        let b = png(
            3,
            1,
            2,
            3,
            &[0, 0x10],
            &[(b"PLTE", &[255, 255, 255, 0, 0, 0])],
        );
        assert_eq!(
            Raster::decode_png_with_threshold(&b, None).unwrap().pixels,
            [255, 0, 255]
        );
        let c = png(2, 1, 4, 0, &[0, 0xf0], &[]);
        assert_eq!(
            Raster::decode_png_with_threshold(&c, None).unwrap().pixels,
            [255, 0]
        );
        let rgba = png(2, 1, 8, 6, &[0, 0, 0, 0, 0, 0, 0, 0, 255], &[]);
        assert_eq!(
            Raster::decode_png_with_threshold(&rgba, None)
                .unwrap()
                .pixels,
            [255, 0]
        );
        let gray = png(2, 1, 8, 0, &[0, 0, 255], &[(b"tRNS", &[0, 0])]);
        assert_eq!(
            Raster::decode_png_with_threshold(&gray, None)
                .unwrap()
                .pixels,
            [255, 255]
        );
    }
    #[test]
    fn all_filters() {
        let raw = [
            0, 0, 255, 0, 1, 0, 255, 1, 2, 0, 0, 0, 3, 0, 128, 129, 4, 0, 0, 0,
        ];
        let p = png(3, 5, 8, 0, &raw, &[]);
        assert_eq!(
            Raster::decode_png_with_threshold(&p, None).unwrap().pixels,
            [0, 255, 0].repeat(5)
        );
    }
    #[test]
    fn strict_mode_does_not_hide_gray_or_color() {
        let p = png(2, 1, 8, 0, &[0, 127, 128], &[]);
        assert!(Raster::decode_png_with_threshold(&p, None).is_err());
        assert_eq!(
            Raster::decode_png_with_threshold(&p, Some(128))
                .unwrap()
                .pixels,
            [0, 255]
        );
        let red = crate::Png::encode_rgb(1, 1, &[255, 0, 0]).unwrap();
        assert!(Raster::decode_png_with_threshold(&red, None).is_err());
        assert_eq!(Raster::decode_png(&red).unwrap().pixels, [0]);
    }
    #[test]
    fn truncation_crc_format_and_limits() {
        let valid = png(1, 1, 8, 0, &[0, 0], &[]);
        for n in 0..valid.len() {
            assert!(Raster::decode_png(&valid[..n]).is_err())
        }
        let mut bad = valid.clone();
        let i = bad.len() - 5;
        bad[i] ^= 1;
        assert!(Raster::decode_png(&bad).unwrap_err().contains("CRC"));
        assert!(Raster::decode_png(&png(u32::MAX, 2, 8, 0, &[], &[])).is_err());
        assert!(Raster::decode_png(&png(1, 1, 8, 0, &[0; 100], &[])).is_err());
        assert!(Raster::decode_png(&png(1, 1, 8, 0, &[9, 0], &[])).is_err());
        assert!(Raster::decode_png(&png(1, 1, 16, 0, &[0, 0, 0], &[])).is_err());
    }
}
