//! Original QR Model 1 encoder, ISO/IEC 18004:2000 Annex M, pp. 97–112.
//! Capacity/RS tables M.4–M.5; placement M.7; format mask M.9.
//! https://qr.redelmann.ch/media/standard_qr.pdf
use super::*;

// (blocks, data words per block, parity words per block), levels L/M/Q/H.
const BLOCKS: [[(usize, usize, usize); 4]; 14] = [
    [(1, 19, 7), (1, 16, 10), (1, 13, 13), (1, 9, 17)],
    [(1, 36, 10), (1, 30, 16), (1, 24, 22), (1, 16, 30)],
    [(1, 57, 15), (1, 44, 28), (1, 36, 36), (1, 24, 48)],
    [(1, 80, 20), (1, 60, 40), (1, 50, 50), (1, 34, 66)],
    [(1, 108, 26), (1, 82, 52), (1, 68, 66), (2, 23, 44)],
    [(1, 136, 34), (2, 53, 32), (2, 43, 42), (2, 29, 56)],
    [(1, 170, 42), (2, 66, 40), (2, 54, 52), (3, 24, 46)],
    [(2, 104, 24), (2, 80, 48), (2, 64, 64), (3, 29, 56)],
    [(2, 123, 30), (2, 93, 60), (3, 52, 50), (3, 34, 68)],
    [(2, 145, 34), (2, 111, 68), (3, 61, 58), (4, 31, 58)],
    [(2, 168, 40), (4, 64, 40), (4, 52, 52), (5, 29, 54)],
    [(2, 192, 46), (4, 73, 46), (4, 61, 58), (5, 33, 62)],
    [(3, 144, 36), (4, 83, 52), (4, 69, 66), (6, 32, 58)],
    [(3, 163, 40), (4, 92, 60), (5, 62, 60), (6, 35, 66)],
];
const TOTAL: [usize; 14] = [
    26, 46, 72, 100, 134, 170, 212, 256, 306, 358, 416, 476, 542, 610,
];

// Captured ZD621 extensions beyond ISO/IEC 18004:2000 Annex M's v14 limit.
// Every data prefix and RS block was recovered and independently verified;
// full printer comparisons and hashes: tests/fixtures/qr-model1-extended-zd621-v1.
const PRINTER_BLOCKS: [[(usize, usize, usize); 4]; 26] = [
    [(3, 184, 44), (4, 103, 68), (6, 58, 56), (7, 33, 64)],
    [(3, 203, 50), (5, 92, 60), (6, 64, 62), (8, 33, 62)],
    [(4, 168, 42), (5, 102, 66), (7, 60, 60), (9, 33, 60)],
    [(4, 185, 46), (6, 94, 60), (8, 59, 56), (10, 32, 60)],
    [(4, 204, 50), (6, 103, 66), (10, 51, 50), (11, 32, 60)],
    [(5, 177, 44), (7, 96, 62), (11, 50, 50), (12, 32, 60)],
    [(5, 193, 48), (7, 104, 68), (12, 50, 50), (12, 34, 66)],
    [(6, 175, 42), (8, 99, 64), (13, 50, 50), (13, 34, 66)],
    [(6, 189, 46), (9, 96, 60), (13, 54, 54), (15, 32, 62)],
    [(6, 203, 50), (10, 92, 60), (15, 51, 50), (15, 35, 66)],
    [(7, 187, 46), (10, 99, 64), (16, 52, 50), (16, 34, 68)],
    [(7, 200, 50), (11, 99, 60), (18, 49, 48), (18, 33, 64)],
    [(8, 188, 46), (11, 102, 68), (18, 52, 52), (18, 36, 68)],
    [(8, 201, 48), (12, 102, 64), (19, 53, 52), (21, 33, 62)],
    [(9, 190, 46), (13, 99, 64), (20, 54, 52), (21, 35, 66)],
    [(9, 200, 50), (14, 97, 64), (23, 50, 48), (23, 34, 64)],
    [(10, 193, 46), (14, 103, 68), (23, 52, 52), (23, 36, 68)],
    [(10, 203, 50), (16, 98, 60), (23, 56, 54), (32, 27, 52)],
    [(12, 179, 44), (18, 93, 56), (25, 55, 52), (26, 35, 68)],
    [(14, 162, 40), (18, 97, 60), (28, 51, 50), (28, 35, 66)],
    [(12, 200, 48), (19, 97, 60), (31, 48, 48), (31, 32, 64)],
    [(13, 193, 48), (20, 97, 60), (32, 50, 48), (32, 34, 64)],
    [(13, 204, 50), (21, 97, 60), (33, 50, 50), (33, 34, 66)],
    [(14, 199, 48), (21, 101, 64), (33, 53, 52), (35, 33, 66)],
    [(15, 194, 48), (23, 98, 60), (36, 51, 50), (36, 35, 66)],
    [(16, 192, 46), (25, 92, 60), (38, 50, 50), (38, 34, 66)],
];
const PRINTER_TOTAL: [usize; 26] = [
    684, 760, 842, 926, 1016, 1108, 1206, 1306, 1412, 1520, 1634, 1750, 1872, 1996, 2126, 2258,
    2396, 2536, 2682, 2830, 2984, 3140, 3302, 3466, 3636, 3808,
];
fn parameters(version: usize, level: usize) -> (usize, usize, usize) {
    if version <= 14 {
        BLOCKS[version - 1][level]
    } else {
        PRINTER_BLOCKS[version - 15][level]
    }
}
fn total_words(version: usize) -> usize {
    if version <= 14 {
        TOTAL[version - 1]
    } else {
        PRINTER_TOTAL[version - 15]
    }
}

pub(super) fn encode(
    input: &qr::Input<'_>,
    level: usize,
    mask: usize,
    extended: bool,
) -> Result<Matrix, String> {
    let mut chosen = None;
    let mut msg = Vec::new();
    let max_version = if extended { 40 } else { 14 };
    for v in 1..=max_version {
        if matches!(v, 1 | 10 | 27) {
            msg = input.message(v)?;
        }
        let (n, k, _) = parameters(v, level);
        if msg.len() <= n * k * 8 - 4 {
            chosen = Some((v, msg));
            break;
        }
    }
    let (v, mut msg) =
        chosen.ok_or_else(|| format!("QR Model 1 data exceeds version {max_version} capacity"))?;
    let (n, k, ec) = parameters(v, level);
    let capacity = n * k * 8 - 4;
    msg.extend(std::iter::repeat_n(false, 4.min(capacity - msg.len())));
    while (msg.len() + 4) % 8 != 0 {
        msg.push(false);
    }
    let mut words = vec![bits::value(&msg[..4])];
    words.extend(msg[4..].chunks(8).map(bits::value));
    let mut pad = 0;
    while words.len() < n * k {
        words.push([236, 17][pad % 2]);
        pad += 1;
    }
    let mut parity = Vec::new();
    for block in words.chunks(k) {
        parity.extend(reed_solomon::parity(block, ec, 0x11d, 0));
    }
    words.extend(parity);
    pad = 0;
    while words.len() < total_words(v) {
        words.push([236, 17][pad % 2]);
        pad += 1;
    }
    place(v, level, mask, &words)
}
fn place(v: usize, level: usize, mask: usize, words: &[usize]) -> Result<Matrix, String> {
    let size = 17 + 4 * v;
    let mut m = Matrix::new(size, size);
    let mut fixed = Matrix::new(size, size);
    let set = |m: &mut Matrix, f: &mut Matrix, x, y, bit| {
        m.set(x, y, bit);
        f.set(x, y, true);
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
                    let d = dx.abs().max(dy.abs());
                    set(&mut m, &mut fixed, x as usize, y as usize, d != 2 && d != 4);
                }
            }
        }
    }
    for y in size - 2..size {
        for x in size - 2..size {
            set(&mut m, &mut fixed, x, y, x == size - 1 && y == size - 1);
        }
    }
    // M.3.2: alternating 2x4 edge blocks, including the parity-dependent offset.
    for start in (if v.is_multiple_of(2) { 13 } else { 17 }..size - 5).step_by(8) {
        for i in 0..4 {
            for j in 0..2 {
                set(&mut m, &mut fixed, size - 2 + j, start + i, j == 1);
                set(&mut m, &mut fixed, start + i, size - 2 + j, j == 1);
            }
        }
    }
    let f = ([1, 0, 3, 2][level] << 3) | mask;
    let mut r = f;
    for _ in 0..10 {
        r = (r << 1) ^ if r & (1 << 9) != 0 { 0x537 } else { 0 };
    }
    let format = ((f << 10) | r) ^ 0x2825;
    for i in 0..15 {
        let bit = format & (1 << i) != 0;
        let (x, y) = match i {
            0..=5 => (8, i),
            6 => (8, 7),
            7 => (8, 8),
            8 => (7, 8),
            _ => (14 - i, 8),
        };
        set(&mut m, &mut fixed, x, y, bit);
        let (x, y) = if i < 8 {
            (size - 1 - i, 8)
        } else {
            (8, size - 15 + i)
        };
        set(&mut m, &mut fixed, x, y, bit);
    }
    set(&mut m, &mut fixed, 8, size - 8, true);
    let mut index = 0;
    let mut block = |x: usize, y: usize, w: usize, h: usize| -> Result<(), String> {
        if (y..y + h).any(|j| (x..x + w).any(|i| fixed.get(i, j))) {
            return Ok(());
        }
        let word = *words
            .get(index)
            .ok_or("QR Model 1 placement exceeded capacity")?;
        for j in 0..h {
            for i in 0..w {
                let bit = word & (1 << (j * w + i)) != 0;
                m.set(x + i, y + j, bit ^ qr::mask_bit(mask, x + i, y + j));
            }
        }
        index += 1;
        Ok(())
    };
    // M.7/Figs. M.6–M.7: two vertical columns, horizontal middle columns,
    // then four vertical columns. Bit 0 is always at the upper left.
    block(size - 2, size - 4, 2, 2)?;
    for y in (9..=size - 8).rev().step_by(4) {
        block(size - 2, y, 2, 4)?;
    }
    for y in (9..=size - 4).rev().step_by(4) {
        block(size - 4, y, 2, 4)?;
    }
    for x in (9..=size - 8).rev().step_by(4) {
        for y in (7..=size - 2).rev().step_by(2) {
            block(x, y, 4, 2)?;
        }
        for y in [4, 2, 0] {
            block(x, y, 4, 2)?;
        }
    }
    for x in [7, 4, 2, 0] {
        for y in (9..=size - 12).rev().step_by(4) {
            block(x, y, 2, 4)?;
        }
    }
    if index != words.len() {
        return Err("QR Model 1 placement did not fill capacity".into());
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independently_decodes_model1_matrices() {
        for (level, _) in BLOCKS[0].iter().enumerate() {
            for len in [1, 20, 40, 80, 120, 160, 200] {
                let data = vec![b'a'; len];
                let mut source = format!("LM,B{len:04}").into_bytes();
                source.extend_from_slice(&data);
                let (_, input) = qr::Input::parse(&source).unwrap();
                let m = encode(&input, level, 3, false).unwrap();
                // rxing 0.9.3's generic DataBlock deinterleaver does not handle
                // Model 1 multi-block symbols. Those are checked against printer
                // modules in barcode_modes_preview instead (ISO Annex M.6).
                let v = (m.w - 17) / 4;
                if BLOCKS[v - 1][level].0 > 1 {
                    continue;
                }
                let mut sample = rxing::common::BitMatrix::new(m.w as u32, m.h as u32).unwrap();
                for y in 0..m.h {
                    for x in 0..m.w {
                        if m.get(x, y) {
                            sample.set(x as u32, y as u32);
                        }
                    }
                }
                let result = rxing::qrcode::cpp_port::decoder::Decode(&sample)
                    .unwrap_or_else(|e| panic!("level {level} len {len} size {}: {e:?}", m.w));
                assert_eq!(result.text(), "a".repeat(len));
            }
        }
    }
}
