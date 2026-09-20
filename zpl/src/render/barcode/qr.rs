//! Original QR Model 2 bitstream, error correction and module placement.
use super::*;

pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    b.require(0, "N", &["N"])?;
    b.require(1, "2", &["1", "2"])?;
    b.require(3, "Q", &["L", "M", "Q", "H"])?;
    let scale = b.num(2, b.scale(), 1., 100.)?;
    let mask = b.integer(4, 7, 0, 7)?;
    let (level, input) = Input::parse(data)?;
    let matrix = if b.param(1, "2") == "1" {
        super::qr_model1::encode(
            &input,
            level,
            mask,
            b.compatibility.qr_model1_extended_versions,
        )?
    } else {
        encode(&input, level, mask)?
    };
    b.matrix(&matrix, scale, scale)
}
/// ZPL ^BQ switches, including the D structured-append envelope and up to
/// 200 manual segments (Zebra Programming Guide, pp. 129–134).
pub(super) struct Input<'a> {
    append: Option<[usize; 3]>,
    segments: Vec<(usize, &'a [u8])>,
}
impl<'a> Input<'a> {
    pub(super) fn parse(mut data: &'a [u8]) -> Result<(usize, Self), String> {
        let append = if data.first() == Some(&b'D') {
            if data.len() < 8 || data[7] != b',' || !data[1..5].iter().all(u8::is_ascii_digit) {
                return Err("invalid QR structured-append header".into());
            }
            let decimal = |bytes: &[u8]| bytes.iter().fold(0, |n, &c| n * 10 + (c - b'0') as usize);
            let index = decimal(&data[1..3]);
            let total = decimal(&data[3..5]);
            if !(2..=16).contains(&total) || !(1..=total).contains(&index) {
                return Err("invalid QR structured-append sequence".into());
            }
            if !data[5..7].iter().all(u8::is_ascii_hexdigit) {
                return Err("invalid QR structured-append parity".into());
            }
            let parity = usize::from_str_radix(ascii(&data[5..7])?, 16)
                .map_err(|_| "invalid QR structured-append parity")?;
            data = &data[8..];
            Some([index - 1, total - 1, parity])
        } else {
            None
        };
        let level = match data.first() {
            Some(b'L') => 0,
            Some(b'M') => 1,
            Some(b'Q') => 2,
            Some(b'H') => 3,
            _ => return Err("QR field requires an error-correction switch".into()),
        };
        if data.get(2) != Some(&b',') {
            return Err("invalid QR field switches".into());
        }
        let mut input = Self {
            append,
            segments: Vec::new(),
        };
        match data[1] {
            b'A' => input.segments.push((0, &data[3..])),
            b'M' => {
                let mut remaining = &data[3..];
                loop {
                    if input.segments.len() == 200 {
                        return Err("too many QR manual segments".into());
                    }
                    let mode = match remaining.first() {
                        Some(b'N') => 1,
                        Some(b'A') => 2,
                        Some(b'B') => 4,
                        Some(b'K') => 8,
                        _ => return Err("unsupported QR manual character mode".into()),
                    };
                    let (payload, rest) = if mode == 4 {
                        if remaining.len() < 5 || !remaining[1..5].iter().all(u8::is_ascii_digit) {
                            return Err("invalid QR byte count".into());
                        }
                        let n = ascii(&remaining[1..5])?
                            .parse::<usize>()
                            .map_err(|_| "invalid QR byte count")?;
                        if n > remaining.len() - 5 {
                            return Err("QR byte count mismatch".into());
                        }
                        (&remaining[5..5 + n], &remaining[5 + n..])
                    } else if append.is_some() {
                        let end = remaining
                            .iter()
                            .position(|&c| c == b',')
                            .unwrap_or(remaining.len());
                        (&remaining[1..end], &remaining[end..])
                    } else {
                        (&remaining[1..], &[][..])
                    };
                    input.segments.push((mode, payload));
                    if rest.is_empty() {
                        break;
                    }
                    if append.is_none() || rest[0] != b',' {
                        return Err("QR byte count mismatch".into());
                    }
                    remaining = &rest[1..];
                }
            }
            _ => return Err("unsupported QR input mode".into()),
        }
        Ok((level, input))
    }
    pub(super) fn message(&self, version: usize) -> Result<Vec<bool>, String> {
        let mut out = Vec::new();
        // ISO/IEC 18004:2000 §9 pp. 55–56: mode 0011, zero-based sequence
        // and total nibbles, then caller-supplied XOR parity of the whole set.
        // https://qr.redelmann.ch/media/standard_qr.pdf
        if let Some([index, total, parity]) = self.append {
            bits::push(&mut out, 3, 4);
            bits::push(&mut out, index, 4);
            bits::push(&mut out, total, 4);
            bits::push(&mut out, parity, 8);
        }
        for &(mode, data) in &self.segments {
            out.extend(message(data, mode, version)?);
        }
        Ok(out)
    }
}

const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";
// ISO/IEC 18004 tables, cross-checked against Thonky error-correction tables.
const EC: [[usize; 40]; 4] = [
    [
        7, 10, 15, 20, 26, 18, 20, 24, 30, 18, 20, 24, 26, 30, 22, 24, 28, 30, 28, 28, 28, 28, 30,
        30, 26, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
    ],
    [
        10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28, 28,
        28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
    ],
    [
        13, 22, 18, 26, 18, 24, 18, 22, 20, 24, 28, 26, 24, 20, 30, 24, 28, 28, 26, 30, 28, 30, 30,
        30, 30, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
    ],
    [
        17, 28, 22, 16, 22, 28, 26, 26, 24, 28, 24, 28, 22, 24, 24, 30, 28, 28, 26, 28, 30, 24, 30,
        30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
    ],
];
const BLOCKS: [[usize; 40]; 4] = [
    [
        1, 1, 1, 1, 1, 2, 2, 2, 2, 4, 4, 4, 4, 4, 6, 6, 6, 6, 7, 8, 8, 9, 9, 10, 12, 12, 12, 13,
        14, 15, 16, 17, 18, 19, 19, 20, 21, 22, 24, 25,
    ],
    [
        1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21, 23,
        25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49,
    ],
    [
        1, 1, 2, 2, 4, 4, 6, 6, 8, 8, 8, 10, 12, 16, 12, 17, 16, 18, 21, 20, 23, 23, 25, 27, 29,
        34, 34, 35, 38, 40, 43, 45, 48, 51, 53, 56, 59, 62, 65, 68,
    ],
    [
        1, 1, 2, 4, 4, 4, 5, 6, 8, 8, 11, 11, 16, 16, 18, 16, 19, 21, 25, 25, 25, 34, 30, 32, 35,
        37, 40, 42, 45, 48, 51, 54, 57, 60, 63, 66, 70, 74, 77, 81,
    ],
];
fn raw_modules(v: usize) -> usize {
    let mut n = (16 * v + 128) * v + 64;
    if v >= 2 {
        let a = v / 7 + 2;
        n -= (25 * a - 10) * a - 55;
        if v >= 7 {
            n -= 36;
        }
    }
    n
}
fn count_width(mode: usize, v: usize) -> usize {
    match mode {
        1 => {
            if v < 10 {
                10
            } else if v < 27 {
                12
            } else {
                14
            }
        }
        2 => {
            if v < 10 {
                9
            } else if v < 27 {
                11
            } else {
                13
            }
        }
        8 => {
            if v < 10 {
                8
            } else if v < 27 {
                10
            } else {
                12
            }
        }
        _ => {
            if v < 10 {
                8
            } else {
                16
            }
        }
    }
}

// ISO/IEC 18004:2000 §§8.2–8.4 and Annex H: each segment carries a mode
// indicator and version-dependent count. Minimize total bits, including those
// switches; whole-field Byte encoding wastes space on mixed-case input.
fn automatic_message(data: &[u8], v: usize) -> Result<Vec<bool>, String> {
    if data.len() > 7089 {
        return Err("QR data exceeds version 40 capacity".into());
    }
    if data.iter().all(u8::is_ascii_digit) {
        return message(data, 1, v);
    }
    if data
        .iter()
        .all(|c| ALPHABET.contains(c) && !c.is_ascii_digit())
    {
        return message(data, 2, v);
    }
    if data.iter().all(|c| !ALPHABET.contains(c)) {
        return message(data, 4, v);
    }
    let mut costs = vec![usize::MAX; data.len() + 1];
    let mut next = vec![(0, 0); data.len()];
    costs[data.len()] = 0;
    for start in (0..data.len()).rev() {
        for mode in [1, 2, 4] {
            let width = count_width(mode, v);
            for end in start + 1..=data.len().min(start + (1 << width) - 1) {
                let c = data[end - 1];
                if mode == 1 && !c.is_ascii_digit() || mode == 2 && !ALPHABET.contains(&c) {
                    break;
                }
                let len = end - start;
                let payload = match mode {
                    1 => len / 3 * 10 + [0, 4, 7][len % 3],
                    2 => len / 2 * 11 + len % 2 * 6,
                    _ => len * 8,
                };
                let cost = 4 + width + payload + costs[end];
                if cost < costs[start] {
                    costs[start] = cost;
                    next[start] = (end, mode);
                }
            }
        }
    }
    let mut out = Vec::with_capacity(costs[0]);
    let mut start = 0;
    while start < data.len() {
        let (end, mode) = next[start];
        out.extend(message(&data[start..end], mode, v)?);
        start = end;
    }
    Ok(out)
}

pub(super) fn message(data: &[u8], mode: usize, v: usize) -> Result<Vec<bool>, String> {
    if mode == 0 {
        return automatic_message(data, v);
    }
    let mut out = Vec::new();
    bits::push(&mut out, mode, 4);
    let count = if mode == 8 {
        data.len() / 2
    } else {
        data.len()
    };
    bits::push(&mut out, count, count_width(mode, v));
    match mode {
        1 => {
            digits(data)?;
            for part in data.chunks(3) {
                let n = part.iter().fold(0, |n, &v| n * 10 + (v - b'0') as usize);
                bits::push(&mut out, n, [0, 4, 7, 10][part.len()]);
            }
        }
        2 => {
            for part in data.chunks(2) {
                let mut n = 0;
                for c in part {
                    n = n * 45
                        + ALPHABET
                            .iter()
                            .position(|v| v == c)
                            .ok_or("invalid QR alphanumeric character")?;
                }
                bits::push(&mut out, n, if part.len() == 2 { 11 } else { 6 });
            }
        }
        8 => {
            // ISO/IEC 18004:2000 §8.4.5, p. 24, and Table 3: Shift JIS
            // pairs become 13-bit values; the count is characters, not bytes.
            // https://qr.redelmann.ch/media/standard_qr.pdf
            let (pairs, remainder) = data.as_chunks::<2>();
            if !remainder.is_empty() {
                return Err("incomplete QR Kanji character".into());
            }
            for pair in pairs {
                let value = u16::from_be_bytes(*pair) as usize;
                if !(0x40..=0xfc).contains(&pair[1]) || pair[1] == 0x7f {
                    return Err("invalid QR Kanji character".into());
                }
                let shifted = match value {
                    0x8140..=0x9ffc => value - 0x8140,
                    0xe040..=0xebbf => value - 0xc140,
                    _ => return Err("invalid QR Kanji character".into()),
                };
                bits::push(&mut out, (shifted >> 8) * 0xc0 + (shifted & 0xff), 13);
            }
        }
        _ => {
            for &v in data {
                bits::push(&mut out, v as usize, 8);
            }
        }
    }
    Ok(out)
}
fn encode(input: &Input<'_>, level: usize, mask: usize) -> Result<Matrix, String> {
    let mut chosen = None;
    let mut msg = Vec::new();
    for v in 1..=40 {
        if matches!(v, 1 | 10 | 27) {
            msg = input.message(v)?;
        }
        let capacity = raw_modules(v) / 8 - EC[level][v - 1] * BLOCKS[level][v - 1];
        if msg.len() <= capacity * 8 {
            chosen = Some((v, capacity, msg));
            break;
        }
    }
    let (v, capacity, mut msg) = chosen.ok_or("QR data exceeds version 40 capacity")?;
    msg.extend(std::iter::repeat_n(false, 4.min(capacity * 8 - msg.len())));
    while msg.len() % 8 != 0 {
        msg.push(false);
    }
    let mut bytes: Vec<usize> = msg.chunks(8).map(bits::value).collect();
    let mut pad = 0;
    while bytes.len() < capacity {
        bytes.push([236, 17][pad % 2]);
        pad += 1;
    }
    let count = BLOCKS[level][v - 1];
    let ec = EC[level][v - 1];
    let short = capacity / count;
    let long_count = capacity % count;
    let mut blocks = Vec::new();
    let mut parity = Vec::new();
    let mut offset = 0;
    for i in 0..count {
        let n = short + usize::from(i >= count - long_count);
        let block = bytes[offset..offset + n].to_vec();
        parity.push(reed_solomon::parity(&block, ec, 0x11d, 0));
        blocks.push(block);
        offset += n;
    }
    let mut stream = Vec::new();
    for i in 0..=short {
        for block in &blocks {
            if let Some(&v) = block.get(i) {
                bits::push(&mut stream, v, 8);
            }
        }
    }
    for i in 0..ec {
        for block in &parity {
            bits::push(&mut stream, block[i], 8);
        }
    }
    let size = v * 4 + 17;
    let mut m = Matrix::new(size, size);
    let mut fixed = Matrix::new(size, size);
    let set = |m: &mut Matrix, fixed: &mut Matrix, x: usize, y: usize, value: bool| {
        m.set(x, y, value);
        fixed.set(x, y, true);
    };
    for i in 0..size {
        set(&mut m, &mut fixed, 6, i, i % 2 == 0);
        set(&mut m, &mut fixed, i, 6, i % 2 == 0);
    }
    for (cx, cy) in [(3, 3), (size - 4, 3), (3, size - 4)] {
        for dy in -4isize..=4 {
            for dx in -4isize..=4 {
                let x = cx as isize + dx;
                let y = cy as isize + dy;
                if x >= 0 && y >= 0 && x < size as isize && y < size as isize {
                    let distance = dx.abs().max(dy.abs());
                    set(
                        &mut m,
                        &mut fixed,
                        x as usize,
                        y as usize,
                        distance != 2 && distance != 4,
                    );
                }
            }
        }
    }
    if v >= 2 {
        let count = v / 7 + 2;
        let step = if v == 32 {
            26
        } else {
            ((4 * v + 2 * count + 1) / (2 * count - 2)) * 2
        };
        let mut centers = vec![6];
        for i in (0..count - 1).rev() {
            centers.push(size - 7 - i * step);
        }
        for (i, &cx) in centers.iter().enumerate() {
            for (j, &cy) in centers.iter().enumerate() {
                if (i == 0 && (j == 0 || j == centers.len() - 1))
                    || (j == 0 && i == centers.len() - 1)
                {
                    continue;
                }
                for dy in -2isize..=2 {
                    for dx in -2isize..=2 {
                        set(
                            &mut m,
                            &mut fixed,
                            (cx as isize + dx) as usize,
                            (cy as isize + dy) as usize,
                            dx.abs().max(dy.abs()) != 1,
                        );
                    }
                }
            }
        }
    }
    let format_data = ([1, 0, 3, 2][level] << 3) | mask;
    let mut remainder = format_data;
    for _ in 0..10 {
        remainder = (remainder << 1) ^ if remainder & (1 << 9) != 0 { 0x537 } else { 0 };
    }
    let format = ((format_data << 10) | remainder) ^ 0x5412;
    for i in 0..15 {
        let value = format & (1 << i) != 0;
        let (x, y) = match i {
            0..=5 => (8, i),
            6 => (8, 7),
            7 => (8, 8),
            8 => (7, 8),
            _ => (14 - i, 8),
        };
        set(&mut m, &mut fixed, x, y, value);
        let (x, y) = if i < 8 {
            (size - 1 - i, 8)
        } else {
            (8, size - 15 + i)
        };
        set(&mut m, &mut fixed, x, y, value);
    }
    set(&mut m, &mut fixed, 8, size - 8, true);
    if v >= 7 {
        let mut r = v;
        for _ in 0..12 {
            r = (r << 1) ^ if r & (1 << 11) != 0 { 0x1f25 } else { 0 };
        }
        let version = (v << 12) | r;
        for i in 0..18 {
            let a = size - 11 + i % 3;
            let b = i / 3;
            let bit = version & (1 << i) != 0;
            set(&mut m, &mut fixed, a, b, bit);
            set(&mut m, &mut fixed, b, a, bit);
        }
    }
    let mut right = size - 1;
    let mut up = true;
    let mut index = 0;
    while right > 0 {
        if right == 6 {
            right -= 1;
        }
        for vertical in 0..size {
            let y = if up { size - 1 - vertical } else { vertical };
            for x in [right, right - 1] {
                if !fixed.get(x, y) {
                    let bit = stream.get(index).copied().unwrap_or(false);
                    index += 1;
                    let flip = mask_bit(mask, x, y);
                    m.set(x, y, bit ^ flip);
                }
            }
        }
        up = !up;
        if right < 2 {
            break;
        }
        right -= 2;
    }
    Ok(m)
}

pub(super) fn mask_bit(mask: usize, x: usize, y: usize) -> bool {
    match mask {
        0 => (x + y).is_multiple_of(2),
        1 => y.is_multiple_of(2),
        2 => x.is_multiple_of(3),
        3 => (x + y).is_multiple_of(3),
        4 => (x / 3 + y / 2).is_multiple_of(2),
        5 => x * y % 2 + x * y % 3 == 0,
        6 => (x * y % 2 + x * y % 3).is_multiple_of(2),
        _ => ((x + y) % 2 + x * y % 3).is_multiple_of(2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_append_header_and_manual_segments() {
        // ISO/IEC 18004:2000 §9.2 example: symbol 3 of 7 has sequence 0x26.
        let (level, input) = Input::parse(b"D03070C,LM,N0123,AAB,B0005ab,cd").unwrap();
        assert_eq!(level, 0);
        let bits = input.message(1).unwrap();
        assert_eq!(bits::value(&bits[..20]), 0x3260c);
        let expected: Vec<_> = [(1, &b"0123"[..]), (2, &b"AB"[..]), (4, &b"ab,cd"[..])]
            .into_iter()
            .flat_map(|(mode, data)| message(data, mode, 1).unwrap())
            .collect();
        assert_eq!(&bits[20..], expected);
    }

    #[test]
    fn structured_append_rejects_invalid_envelopes_and_segments() {
        for source in [
            "D000200,LA,A",
            "D030200,LA,A",
            "D010100,LA,A",
            "D1717FF,LA,A",
            "D0102+F,LA,A",
            "D0102GG,LA,A",
            "D0102FFLA,A",
            "D0102FF,LM,B0003ab",
            "D0102FF,LM,B0001ab",
            "D0102FF,LM,N12,",
            "LM,B0001ab",
        ] {
            assert!(Input::parse(source.as_bytes()).is_err(), "{source}");
        }
        let valid = format!("D0102FF,LM,{}", ["N1"; 200].join(","));
        assert!(Input::parse(valid.as_bytes()).is_ok());
        assert!(Input::parse(format!("{valid},N1").as_bytes()).is_err());
    }

    #[test]
    fn kanji_normative_values_and_version_count_widths() {
        // ISO/IEC 18004:2000 §8.4.5 p. 24 examples cover both Shift JIS ranges.
        for (version, count_bits) in [(1, 8), (9, 8), (10, 10), (26, 10), (27, 12)] {
            let message = message(&[0x93, 0x5f, 0xe4, 0xaa], 8, version).unwrap();
            assert_eq!(bits::value(&message[..4]), 8);
            assert_eq!(bits::value(&message[4..4 + count_bits]), 2);
            assert_eq!(
                bits::value(&message[4 + count_bits..17 + count_bits]),
                0x0d9f
            );
            assert_eq!(bits::value(&message[17 + count_bits..]), 0x1aaa);
        }
    }

    #[test]
    fn kanji_rejects_incomplete_or_non_jis_pairs() {
        for data in [
            &[0x93][..],
            b"AB",
            &[0x81, 0x7f],
            &[0xeb, 0xc0],
            &[0x80, 0x40],
        ] {
            assert!(message(data, 8, 1).is_err());
        }
    }
}
