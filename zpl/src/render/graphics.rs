use crate::output::{Path, Point, Segment};

/// A diagonal band bounded by the ^GD box (Zebra Programming Guide, p. 213).
/// The guide specifies the box and thickness, not a dot stepping algorithm.
/// Clip the band to both sides of its centerline, preserving perpendicular
/// thickness instead of letting hardware-specific horizontal runs escape.
pub(super) fn diagonal(w: f64, h: f64, thickness: f64, left: bool) -> Path {
    let mut polygon = vec![
        Point::new(0., 0.),
        Point::new(w, 0.),
        Point::new(w, h),
        Point::new(0., h),
    ];
    let limit = thickness * w.hypot(h) / 2.;
    let distance = |p: Point| {
        if left {
            h * p.x - w * p.y
        } else {
            h * p.x + w * p.y - w * h
        }
    };
    for sign in [-1., 1.] {
        let mut clipped = Vec::new();
        if let Some(&last) = polygon.last() {
            let mut previous = last;
            for &current in &polygon {
                let a = sign * distance(previous) - limit;
                let b = sign * distance(current) - limit;
                if (a <= 0.) != (b <= 0.) {
                    let t = a / (a - b);
                    clipped.push(Point::new(
                        previous.x + t * (current.x - previous.x),
                        previous.y + t * (current.y - previous.y),
                    ));
                }
                if b <= 0. {
                    clipped.push(current);
                }
                previous = current;
            }
        }
        polygon = clipped;
    }
    let mut path = Path::default();
    if let Some((&first, rest)) = polygon.split_first() {
        path.segments.push(Segment::Move(first));
        path.segments
            .extend(rest.iter().copied().map(Segment::Line));
        path.segments.push(Segment::Close);
    }
    path
}
fn hex(c: u8) -> Result<u8, String> {
    (c as char)
        .to_digit(16)
        .map(|v| v as u8)
        .ok_or_else(|| "invalid graphic hex digit".into())
}
fn base64(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let chars: Vec<_> = data
        .iter()
        .copied()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    if chars.len() % 4 != 0 {
        return Err("invalid base64 length".into());
    }
    for (i, q) in chars.as_chunks::<4>().0.iter().enumerate() {
        let mut acc = 0u32;
        let mut padding = 0;
        for (j, &c) in q.iter().enumerate() {
            let v = match c {
                b'A'..=b'Z' => c - b'A',
                b'a'..=b'z' => c - b'a' + 26,
                b'0'..=b'9' => c - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' if j >= 2 => {
                    padding += 1;
                    0
                }
                _ => return Err("invalid base64".into()),
            };
            if padding > 0 && c != b'=' {
                return Err("invalid base64 padding".into());
            }
            acc = acc * 64 + v as u32;
        }
        if padding > 0 && i + 1 != chars.len() / 4 {
            return Err("invalid base64 padding".into());
        }
        if (padding == 1 && acc & 255 != 0) || (padding == 2 && acc & 65535 != 0) {
            return Err("noncanonical base64 padding".into());
        }
        out.push((acc >> 16) as u8);
        if padding < 2 {
            out.push((acc >> 8) as u8)
        }
        if padding == 0 {
            out.push(acc as u8)
        }
    }
    Ok(out)
}
pub(super) fn decode(data: &[u8], bytes: usize, row: usize, binary: bool) -> Result<Path, String> {
    if bytes == 0 || row == 0 || !bytes.is_multiple_of(row) || bytes > 25_000 {
        return Err("invalid or excessive graphic dimensions".into());
    }
    let decoded = if binary {
        if data.len() < bytes || !data[bytes..].iter().all(u8::is_ascii_whitespace) {
            return Err("binary graphic length mismatch".into());
        }
        data[..bytes].to_vec()
    } else if data.starts_with(b":Z64:") || data.starts_with(b":B64:") {
        let end = data[5..]
            .iter()
            .position(|&c| c == b':')
            .ok_or("missing graphic checksum")?
            + 5;
        let encoded = &data[5..end];
        let crc_text = data[end + 1..].trim_ascii();
        if crc_text.len() != 4 {
            return Err("invalid graphic checksum".into());
        }
        let expected = crc_text
            .iter()
            .try_fold(0u16, |v, &c| Ok::<_, String>((v << 4) | hex(c)? as u16))?;
        let mut crc = 0u16;
        for &c in encoded {
            crc ^= (c as u16) << 8;
            for _ in 0..8 {
                crc = if crc & 0x8000 != 0 {
                    (crc << 1) ^ 0x1021
                } else {
                    crc << 1
                }
            }
        }
        if crc != expected {
            return Err("graphic CRC mismatch".into());
        }
        let raw = base64(encoded)?;
        if data[1] == b'Z' {
            super::compression::inflate(&raw, bytes)?
        } else {
            raw
        }
    } else {
        let mut nibbles = Vec::new();
        let mut repeat = 0;
        for &c in data {
            if c.is_ascii_whitespace() {
                continue;
            }
            match c {
                b'G'..=b'Y' => {
                    repeat += usize::from(c - b'G' + 1);
                    continue;
                }
                b'g'..=b'z' => {
                    repeat += usize::from(c - b'g' + 1) * 20;
                    continue;
                }
                b',' | b'!' => {
                    if repeat != 0 {
                        return Err("repeat before row shorthand".into());
                    }
                    let count = row * 2 - nibbles.len() % (row * 2);
                    nibbles.extend(std::iter::repeat_n(if c == b',' { 0 } else { 15 }, count));
                }
                b':' => {
                    if repeat != 0 || nibbles.len() < row * 2 || nibbles.len() % (row * 2) != 0 {
                        return Err("invalid repeated graphic row".into());
                    }
                    let start = nibbles.len() - row * 2;
                    nibbles.extend_from_within(start..);
                }
                _ => {
                    let value = hex(c)?;
                    let count = repeat.max(1);
                    if nibbles.len() + count > bytes * 2 {
                        return Err("graphic exceeds declared size".into());
                    }
                    nibbles.extend(std::iter::repeat_n(value, count));
                    repeat = 0;
                }
            }
            if nibbles.len() > bytes * 2 || repeat > bytes * 2 {
                return Err("graphic exceeds declared size".into());
            }
        }
        if repeat != 0 || nibbles.len() % 2 != 0 {
            return Err("incomplete graphic data".into());
        }
        nibbles
            .as_chunks::<2>()
            .0
            .iter()
            .map(|q| q[0] * 16 + q[1])
            .collect()
    };
    if decoded.len() != bytes {
        return Err("graphic byte count mismatch".into());
    }
    let mut path = Path::default();
    for (y, line) in decoded.chunks_exact(row).enumerate() {
        let mut start = None;
        for x in 0..=row * 8 {
            let black = x < row * 8 && line[x / 8] & (128 >> (x % 8)) != 0;
            match (start, black) {
                (None, true) => start = Some(x),
                (Some(a), false) => {
                    path.rect(a as f64, y as f64, (x - a) as f64, 1.);
                    start = None
                }
                _ => {}
            }
        }
    }
    Ok(path)
}
