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

pub(super) fn encode(input: &qr::Input<'_>, level: usize, mask: usize) -> Result<Matrix, String> {
    let mut chosen = None;
    let mut msg = Vec::new();
    for v in 1..=14 {
        if matches!(v, 1 | 10) {
            msg = input.message(v)?;
        }
        let (n, k, _) = BLOCKS[v - 1][level];
        if msg.len() <= n * k * 8 - 4 {
            chosen = Some((v, msg));
            break;
        }
    }
    let (v, mut msg) = chosen.ok_or("QR Model 1 data exceeds version 14 capacity")?;
    let (n, k, ec) = BLOCKS[v - 1][level];
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
    while words.len() < TOTAL[v - 1] {
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
                let m = encode(&input, level, 3).unwrap();
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
