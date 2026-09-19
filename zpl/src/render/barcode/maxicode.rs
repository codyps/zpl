//! Original ISO/IEC 16023 encoder. See docs/barcodes.md for source specifications.
use super::*;
fn set_a(c: u8) -> Option<usize> {
    match c {
        13 => Some(0),
        65..=90 => Some((c - 64) as usize),
        28..=30 | 32 | 34..=58 => Some(c as usize),
        _ => None,
    }
}
fn character(c: u8) -> (usize, usize) {
    if let Some(v) = set_a(c) {
        return (0, v);
    }
    if (96..=122).contains(&c) {
        return (59, (c - 96) as usize);
    }
    const B: &[u8] = &[
        123, 0, 125, 126, 127, 59, 60, 61, 62, 63, 91, 92, 93, 94, 95, 32, 44, 46, 47, 58, 64, 33,
        124,
    ];
    if c != 0 {
        if let Some(i) = B.iter().position(|&v| v == c) {
            return (59, 32 + i);
        }
    }
    if c <= 26 {
        return (62, c as usize);
    }
    if c == 27 {
        return (62, 30);
    }
    if c == 31 {
        return (62, 35);
    }
    if c >= 192 {
        return (
            if c >= 224 { 61 } else { 60 },
            if c % 32 < 27 {
                (c % 32) as usize
            } else {
                (c % 32 + 5) as usize
            },
        );
    }
    const C: &[u8] = &[
        170, 172, 177, 178, 179, 181, 185, 186, 188, 189, 190, 128, 129, 130, 131, 132, 133, 134,
        135, 136, 137,
    ];
    const D: &[u8] = &[
        161, 168, 171, 175, 176, 180, 183, 184, 187, 191, 138, 139, 140, 141, 142, 143, 144, 145,
        146, 147, 148,
    ];
    const E: &[u8] = &[
        159, 160, 162, 163, 164, 165, 166, 167, 169, 173, 174, 182, 149, 150, 151, 152, 153, 154,
        155, 156, 157, 158,
    ];
    for (shift, start, table) in [(60, 37, C), (61, 37, D), (62, 36, E)] {
        if let Some(i) = table.iter().position(|&v| v == c) {
            return (shift, start + i);
        }
    }
    unreachable!("MaxiCode character tables cover all 256 byte values")
}
// ISO/IEC 16023:2000 Annex A pp. 24–25, Annex F.1–F.4 pp. 32–33.
// Original implementation of the recommended run-based switching rules.
fn compact(data: &[u8]) -> (Vec<usize>, usize) {
    fn value(c: u8, set: usize) -> Option<usize> {
        if set == 0 {
            return set_a(c);
        }
        if set == 1 {
            if (28..=30).contains(&c) {
                return Some(c as usize);
            }
            if (96..=122).contains(&c) {
                return Some((c - 96) as usize);
            }
            return b"{\0}~\x7f;<=>?[\\]^_ ,./:@!|"
                .iter()
                .position(|&v| v == c && c != 0)
                .map(|v| v + 32);
        }
        if c == 32 {
            return Some(59);
        }
        if (28..=30).contains(&c) {
            return Some(c as usize + if set == 4 { 4 } else { 0 });
        }
        if set == 4 && c <= 26 {
            return Some(c as usize);
        }
        let (shift, v) = character(c);
        (shift == set + 58).then_some(v)
    }
    let (mut out, mut set, mut i) = (Vec::new(), 0, 0);
    while i < data.len() {
        let tail = &data[i..];
        if tail.len() >= 9 && tail[..9].iter().all(u8::is_ascii_digit) {
            let n = tail[..9]
                .iter()
                .fold(0usize, |n, v| n * 10 + (v - b'0') as usize);
            out.push(31);
            out.extend((0..5).rev().map(|k| (n >> (6 * k)) & 63));
            i += 9;
            continue;
        }
        let run = |target| {
            tail.iter()
                .take_while(|&&c| value(c, target).is_some())
                .count()
        };
        if set == 1 && data[i] != b' ' && run(0) >= 4 {
            out.push(63);
            set = 0;
        }
        if let Some(v) = value(data[i], set) {
            out.push(v);
            i += 1;
            continue;
        }
        let target = (0..5).find(|&s| value(data[i], s).is_some()).unwrap();
        let count = run(target);
        if target < 2 {
            if set >= 2 || target == 1 && count >= 2 || target == 0 && count >= 4 {
                out.push(if target == 0 && set >= 2 { 58 } else { 63 });
                set = target;
            } else {
                let count = if target == 0 { count.min(3) } else { 1 };
                out.push(if count == 1 { 59 } else { 54 + count });
                for &c in &tail[..count] {
                    out.push(value(c, target).unwrap());
                }
                i += count;
            }
        } else {
            let shift = target + 58;
            out.push(shift);
            if count >= if set < 2 { 4 } else { 2 } {
                out.push(shift);
                set = target;
            } else {
                out.push(value(data[i], target).unwrap());
                i += 1;
            }
        }
    }
    (out, set)
}

fn encode(b: &Barcode, data: &[u8]) -> Result<Matrix, String> {
    let mode = b.integer(0, 2, 2, 6)?;
    let number = b.integer(1, 1, 1, 8)?;
    let total = b.integer(2, 1, 1, 8)?;
    if number > total {
        return Err("MaxiCode symbol number exceeds total".into());
    }
    let mut words = vec![0usize; 144];
    let mut body = data;
    if mode <= 3 {
        let header = if mode == 2 { 15 } else { 12 };
        if data.len() < header {
            return Err("MaxiCode structured carrier header is too short".into());
        }
        let decimal = |v: &[u8]| -> Result<u64, String> {
            Ok(digits(v)?.iter().fold(0, |n, &v| n * 10 + v as u64))
        };
        let service = decimal(&data[..3])?;
        let country = decimal(&data[3..6])?;
        let postal = if mode == 2 {
            decimal(&data[6..15])? | (9 << 30)
        } else {
            let mut n = 0;
            for &c in &data[6..12] {
                if !c.is_ascii_uppercase() && !c.is_ascii_digit() {
                    return Err(
                        "MaxiCode mode 3 postal code requires six uppercase letters/digits".into(),
                    );
                }
                n = n * 64 + set_a(c).unwrap() as u64;
            }
            n
        };
        let packed = mode as u64 | (postal << 4) | (country << 40) | (service << 50);
        for (i, w) in words[..10].iter_mut().enumerate() {
            *w = ((packed >> (6 * i)) & 63) as usize;
        }
        body = &data[header..];
    } else {
        words[0] = mode;
    }
    if b.compatibility.maxicode_nul_terminates_data {
        body = body.split(|&c| c == 0).next().unwrap_or_default();
    }
    let secondary = if mode == 5 { 68 } else { 84 };
    let capacity = secondary + if mode >= 4 { 9 } else { 0 };
    // Even all-numeric data cannot exceed this bound. Limit the compaction
    // search before allocating suffixes for arbitrary input lengths.
    if body.len() > capacity * 9 / 6 {
        return Err("MaxiCode data exceeds symbol capacity".into());
    }
    let mut stream = Vec::new();
    if total > 1 {
        stream.extend([33, (number - 1) * 8 + total - 1]);
    }
    let (encoded, final_set) = compact(body);
    stream.extend(encoded);
    if final_set >= 2 || b.compatibility.maxicode_terminal_latch {
        stream.push(63);
    }
    if stream.len() > capacity {
        return Err("MaxiCode data exceeds symbol capacity".into());
    }
    stream.resize(capacity, 33);
    let offset = if mode >= 4 {
        words[1..10].copy_from_slice(&stream[..9]);
        9
    } else {
        0
    };
    words[20..20 + secondary].copy_from_slice(&stream[offset..]);
    let check = reed_solomon::parity(&words[..10], 10, 0x43, 1);
    words[10..20].copy_from_slice(&check);
    for lane in 0..2 {
        let data: Vec<_> = (0..secondary / 2)
            .map(|i| words[20 + 2 * i + lane])
            .collect();
        for (i, c) in reed_solomon::parity(&data, (124 - secondary) / 2, 0x43, 1)
            .into_iter()
            .enumerate()
        {
            words[20 + secondary + 2 * i + lane] = c;
        }
    }
    let mut matrix = Matrix::new(30, 33);
    for (y, row) in maxicode_modules::MODULES.iter().enumerate() {
        for (x, &n) in row.iter().enumerate() {
            let dark = if n == 865 {
                true
            } else if n == 0 {
                false
            } else {
                let bit = (n - 1) as usize;
                words[bit / 6] & (1 << (5 - bit % 6)) != 0
            };
            matrix.set(x, y, dark);
        }
    }
    matrix.set(28, 0, true);
    matrix.set(29, 0, true);
    Ok(matrix)
}
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    if b.compatibility.maxicode_standard_minimum_six_bytes
        && matches!(b.integer(0, 2, 2, 6)?, 4 | 6)
        && data.len() < 6
    {
        return Ok(Path::default());
    }
    let mut m = encode(b, data)?;
    if b.compatibility.maxicode_mode5_preview_omits_data && b.integer(0, 2, 2, 6)? == 5 {
        for (y, row) in maxicode_modules::MODULES.iter().enumerate() {
            for (x, &n) in row.iter().enumerate() {
                m.set(x, y, n == 865 || y == 0 && x >= 28);
            }
        }
    }
    if b.compatibility.maxicode_printer_dot_geometry {
        return Ok(printer_geometry(&m, b.dpi));
    }
    // ISO/IEC 16023:2000 4.11, Tables 6–8: nominal L=25.50 mm.
    let dots_per_mm = b.dpi as f64 / 25.4;
    let pitch = (25.50 / 29.) * dots_per_mm;
    let radius = pitch / 3f64.sqrt();
    let ink_x = (pitch - 0.12 * dots_per_mm) / 3f64.sqrt();
    let ink_y = radius - 0.06 * dots_per_mm;
    let dy = pitch * 3f64.sqrt() / 2.;
    let mut p = Path::default();
    for y in 0..33 {
        for x in 0..30 {
            if m.get(x, y) {
                let cx = (x as f64 + 0.5 + (y % 2) as f64 / 2.) * pitch;
                let cy = y as f64 * dy + radius;
                for k in 0..6 {
                    let angle = (k as f64 * 60. - 90.).to_radians();
                    let point = Point::new(cx + ink_x * angle.cos(), cy + ink_y * angle.sin());
                    p.segments.push(if k == 0 {
                        crate::output::Segment::Move(point)
                    } else {
                        crate::output::Segment::Line(point)
                    });
                }
                p.segments.push(crate::output::Segment::Close);
            }
        }
    }
    let cx = 14.5 * pitch;
    let cy = 16. * dy + radius;
    for r in [0.51, 1.18, 1.86, 2.53, 3.20, 3.87] {
        let r = r * dots_per_mm;
        p.ellipse(cx - r, cy - r, 2. * r, 2. * r);
    }
    Ok(p)
}
/// The ZD621's fixed dot stencil, measured from independent mode 2–6 preview
/// captures (tests/fixtures/maxicode-zd621-v1). It is independent of payload.
/// Six-by-seven-dot hexagons use a 7×6 pitch; odd rows shift three dots.
/// Squared ring thresholds lie between the last inside and first outside
/// pixel-centre distances in the captured concentric finder. These are printer
/// calibration values, not the nominal millimetre radii in ISO 16023 §4.11.
fn printer_geometry(m: &Matrix, dpi: u32) -> Path {
    let scale = dpi as f64 / 203.;
    let mut p = Path::default();
    for y in 0..33 {
        for x in 0..30 {
            if !m.get(x, y) {
                continue;
            }
            for (dy, width) in [2, 4, 6, 6, 6, 4, 2].into_iter().enumerate() {
                p.rect(
                    (1 + x * 7 + (y % 2) * 3 + (6 - width) / 2) as f64 * scale,
                    (1 + y * 6 + dy) as f64 * scale,
                    width as f64 * scale,
                    scale,
                );
            }
        }
    }
    for y in 0..64 {
        let mut start = None;
        for x in 0..=64 {
            let d = (x as f64 - 32.5).powi(2) + (y as f64 - 32.5).powi(2);
            let dark = x < 64
                && [(16.5, 81.5), (229.5, 402.5), (689.5, 967.5)]
                    .iter()
                    .any(|&(lo, hi)| d > lo && d < hi);
            if dark && start.is_none() {
                start = Some(x);
            }
            if !dark {
                if let Some(first) = start.take() {
                    p.rect(
                        (69 + first) as f64 * scale,
                        (68 + y) as f64 * scale,
                        (x - first) as f64 * scale,
                        scale,
                    );
                }
            }
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn specification_dimensions() {
        fn extent(segments: &[crate::output::Segment]) -> (f64, f64) {
            use crate::output::Segment;
            let (mut min_x, mut min_y, mut max_x, mut max_y) = (
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            );
            let mut point = |p: &Point| {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
            };
            for s in segments {
                match s {
                    Segment::Move(p) | Segment::Line(p) => point(p),
                    Segment::Cubic(a, b, c) => {
                        point(a);
                        point(b);
                        point(c);
                    }
                    _ => {}
                }
            }
            (max_x - min_x, max_y - min_y)
        }
        for dpi in [203, 300, 600] {
            let b = Barcode::new(
                "BD",
                &["4"],
                2.,
                2.,
                90.,
                dpi,
                crate::render::profiles::SPECIFICATION.compatibility,
            )
            .unwrap();
            let path = render(&b, b"A").unwrap();
            let unit = dpi as f64 / 25.4;
            let (width, height) = extent(&path.segments[..7]);
            assert!((width / unit - (25.50 / 29. - 0.12)).abs() < 1e-9);
            assert!((height / unit - (2. * 25.50 / 29. / 3f64.sqrt() - 0.12)).abs() < 1e-9);
            // Each ellipse has a move, four cubics, and a close. Tables 7/8
            // define the six radii, independent of symbol data and printer DPI.
            let rings = &path.segments[path.segments.len() - 36..];
            for (segments, radius) in rings
                .as_chunks::<6>()
                .0
                .iter()
                .zip([0.51, 1.18, 1.86, 2.53, 3.20, 3.87])
            {
                let (width, height) = extent(segments);
                assert!((width / unit - 2. * radius).abs() < 1e-9);
                assert!((height / unit - 2. * radius).abs() < 1e-9);
            }
        }
    }
    #[test]
    fn full_byte_alphabet_round_trip() {
        let b = Barcode::new(
            "BD",
            &["4"],
            2.,
            2.,
            90.,
            203,
            crate::render::profiles::SPECIFICATION.compatibility,
        )
        .unwrap();
        for first in (0u16..256).step_by(16) {
            let data: Vec<_> = (first..first + 16).map(|v| v as u8).collect();
            let m = encode(&b, &data).unwrap();
            let mut other = anyd::output::BitMatrix::new(30, 33, 0);
            for y in 0..33 {
                for x in 0..30 {
                    other.set(x, y, m.get(x, y));
                }
            }
            let result = anyd::codes::maxicode::MaxiCodeDecoder::new()
                .decode_matrix(&other)
                .unwrap();
            assert_eq!(result.payload_bytes(), data);
        }
    }
    #[test]
    fn independent_decode() {
        for mode in [2, 3, 4, 5, 6] {
            let b = Barcode::new(
                "BD",
                &[&mode.to_string()],
                2.,
                2.,
                90.,
                203,
                crate::render::profiles::SPECIFICATION.compatibility,
            )
            .unwrap();
            let data = match mode {
                2 => b"001840123450000Hello MaxiCode".as_slice(),
                3 => b"001826ABC123Hello MaxiCode".as_slice(),
                _ => b"Hello MaxiCode".as_slice(),
            };
            let m = encode(&b, data).unwrap();
            let mut other = anyd::output::BitMatrix::new(30, 33, 0);
            for y in 0..33 {
                for x in 0..30 {
                    other.set(x, y, m.get(x, y));
                }
            }
            let result = anyd::codes::maxicode::MaxiCodeDecoder::new()
                .decode_matrix(&other)
                .unwrap();
            assert!(result.text().unwrap().ends_with("Hello MaxiCode"));
        }
    }
}
