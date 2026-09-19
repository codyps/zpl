//! Original QR Model 2 bitstream, error correction and module placement.
use super::*;

pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    b.require(0, "N", &["N"])?;
    b.require(1, "2", &["1", "2"])?;
    b.require(3, "Q", &["L", "M", "Q", "H"])?;
    let scale = b.num(2, b.scale(), 1., 100.)?;
    let mask = b.integer(4, 7, 0, 7)?;
    let level = match data.first() {
        Some(b'L') => 0,
        Some(b'M') => 1,
        Some(b'Q') => 2,
        Some(b'H') => 3,
        _ => return Err("QR field requires LA, MA, QA, HA, or manual-mode switches".into()),
    };
    if data.get(2) != Some(&b',') {
        return Err("invalid QR field switches".into());
    }
    let (mode, payload) = match data.get(1) {
        Some(b'A') => {
            let d = &data[3..];
            (
                if d.iter().all(u8::is_ascii_digit) {
                    1
                } else if d.iter().all(|v| ALPHABET.contains(v)) {
                    2
                } else {
                    4
                },
                d,
            )
        }
        Some(b'M') => match data.get(3) {
            Some(b'N') => (1, &data[4..]),
            Some(b'A') => (2, &data[4..]),
            Some(b'B') if data.len() >= 8 => {
                let n = ascii(&data[4..8])?
                    .parse::<usize>()
                    .map_err(|_| "invalid QR byte count")?;
                if n != data.len() - 8 {
                    return Err("QR byte count mismatch".into());
                }
                (4, &data[8..])
            }
            _ => return Err("unsupported QR manual character mode".into()),
        },
        _ => return Err("unsupported QR input mode".into()),
    };
    let matrix = if b.param(1, "2") == "1" {
        super::qr_model1::encode(payload, mode, level, mask)?
    } else {
        encode(payload, mode, level, mask)?
    };
    b.matrix(&matrix, scale, scale)
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
pub(super) fn message(data: &[u8], mode: usize, v: usize) -> Result<Vec<bool>, String> {
    let mut out = Vec::new();
    bits::push(&mut out, mode, 4);
    bits::push(
        &mut out,
        data.len(),
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
            _ => {
                if v < 10 {
                    8
                } else {
                    16
                }
            }
        },
    );
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
        _ => {
            for &v in data {
                bits::push(&mut out, v as usize, 8);
            }
        }
    }
    Ok(out)
}
fn encode(data: &[u8], mode: usize, level: usize, mask: usize) -> Result<Matrix, String> {
    let mut chosen = None;
    for v in 1..=40 {
        let m = message(data, mode, v)?;
        let capacity = raw_modules(v) / 8 - EC[level][v - 1] * BLOCKS[level][v - 1];
        if m.len() <= capacity * 8 {
            chosen = Some((v, capacity, m));
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
