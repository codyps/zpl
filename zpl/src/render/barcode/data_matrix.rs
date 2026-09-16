//! Original ECC200 ASCII encodation, interleaved RS and Utah placement.
use super::*;
// Full columns, rows, data-region columns/rows, data bytes, parity bytes, blocks.
const SIZES: &[(usize, usize, usize, usize, usize, usize, usize)] = &[
    (10, 10, 8, 8, 3, 5, 1),
    (12, 12, 10, 10, 5, 7, 1),
    (14, 14, 12, 12, 8, 10, 1),
    (16, 16, 14, 14, 12, 12, 1),
    (18, 18, 16, 16, 18, 14, 1),
    (20, 20, 18, 18, 22, 18, 1),
    (22, 22, 20, 20, 30, 20, 1),
    (24, 24, 22, 22, 36, 24, 1),
    (26, 26, 24, 24, 44, 28, 1),
    (32, 32, 14, 14, 62, 36, 1),
    (36, 36, 16, 16, 86, 42, 1),
    (40, 40, 18, 18, 114, 48, 1),
    (44, 44, 20, 20, 144, 56, 1),
    (48, 48, 22, 22, 174, 68, 1),
    (52, 52, 24, 24, 204, 84, 2),
    (64, 64, 14, 14, 280, 112, 2),
    (72, 72, 16, 16, 368, 144, 4),
    (80, 80, 18, 18, 456, 192, 4),
    (88, 88, 20, 20, 576, 224, 4),
    (96, 96, 22, 22, 696, 272, 4),
    (104, 104, 24, 24, 816, 336, 6),
    (120, 120, 18, 18, 1050, 408, 6),
    (132, 132, 20, 20, 1304, 496, 8),
    (144, 144, 22, 22, 1558, 620, 10),
    (18, 8, 16, 6, 5, 7, 1),
    (32, 8, 14, 6, 10, 11, 1),
    (26, 12, 24, 10, 16, 14, 1),
    (36, 12, 16, 10, 22, 18, 1),
    (36, 16, 16, 14, 32, 24, 1),
    (48, 16, 22, 14, 49, 28, 1),
];
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    b.require(2, "0", &["200"])?;
    b.integer(5, 6, 0, 6)?;
    let shape = b.integer(7, 1, 1, 2)?;
    let columns = b.integer(3, 0, 0, 144)?;
    let rows = b.integer(4, 0, 0, 144)?;
    let escape = b.param(6, "_");
    if escape.len() != 1 {
        return Err("Data Matrix escape must be one byte".into());
    }
    let mut code = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let c = data[i];
        if c == escape.as_bytes()[0] {
            match data.get(i + 1) {
                Some(b'1') => {
                    code.push(232);
                    i += 2;
                    continue;
                }
                Some(x) if *x == c => {
                    code.push(c as usize + 1);
                    i += 2;
                    continue;
                }
                _ => return Err("unsupported Data Matrix control escape".into()),
            }
        }
        if i + 1 < data.len() && c.is_ascii_digit() && data[i + 1].is_ascii_digit() {
            code.push(130 + ((c - b'0') * 10 + data[i + 1] - b'0') as usize);
            i += 2;
        } else {
            if c >= 128 {
                code.extend([235, c as usize - 127]);
            } else {
                code.push(c as usize + 1);
            }
            i += 1;
        }
    }
    let &(w, h, rw, rh, capacity, ec, blocks) = SIZES
        .iter()
        .filter(|&&(w, h, _, _, cap, _, _)| {
            cap >= code.len()
                && (shape == 1) == (w == h)
                && (columns == 0 || columns == w)
                && (rows == 0 || rows == h)
        })
        .min_by_key(|&&(w, h, ..)| w * h)
        .ok_or("Data Matrix data does not fit requested dimensions")?;
    if code.len() < capacity {
        code.push(129);
    }
    while code.len() < capacity {
        let random = (149 * (code.len() + 1)) % 253 + 1;
        code.push((129 + random - 1) % 254 + 1);
    }
    let mut parity = Vec::new();
    for block in 0..blocks {
        let values: Vec<_> = code.iter().skip(block).step_by(blocks).copied().collect();
        parity.push(reed_solomon::parity(&values, ec / blocks, 0x12d, 1));
    }
    for i in 0..ec / blocks {
        for p in &parity {
            code.push(p[i]);
        }
    }
    let nw = w / (rw + 2);
    let nh = h / (rh + 2);
    let mapping = place(&code, nw * rw, nh * rh);
    let mut matrix = Matrix::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let lx = x % (rw + 2);
            let ly = y % (rh + 2);
            let black = if ly == 0 {
                lx % 2 == 0
            } else if ly == rh + 1 || lx == 0 {
                true
            } else if lx == rw + 1 {
                ly % 2 == 1
            } else {
                mapping.get(x / (rw + 2) * rw + lx - 1, y / (rh + 2) * rh + ly - 1)
            };
            matrix.set(x, y, black);
        }
    }
    let scale = b.num(1, 0., 0., 32000.)?;
    let scale = if scale == 0. {
        (b.height / h as f64).round().max(1.)
    } else {
        scale
    };
    b.matrix(&matrix, scale, scale)
}
fn place(code: &[usize], w: usize, h: usize) -> Matrix {
    let mut m = Matrix::new(w, h);
    let mut occupied = vec![false; w * h];
    let mut word = 0;
    fn write(
        m: &mut Matrix,
        occupied: &mut [bool],
        code: &[usize],
        word: usize,
        positions: &[(isize, isize)],
    ) {
        for (i, &(mut r, mut c)) in positions.iter().enumerate() {
            if r < 0 {
                r += m.h as isize;
                c += 4 - ((m.h + 4) % 8) as isize;
            }
            if c < 0 {
                c += m.w as isize;
                r += 4 - ((m.w + 4) % 8) as isize;
            }
            let index = r as usize * m.w + c as usize;
            occupied[index] = true;
            m.cells[index] = code.get(word).copied().unwrap_or(0) & (1 << (7 - i)) != 0;
        }
    }
    let (hh, ww) = (h as isize, w as isize);
    let (mut r, mut c) = (4isize, 0isize);
    loop {
        let corner = if r == hh && c == 0 {
            Some([
                (hh - 1, 0),
                (hh - 1, 1),
                (hh - 1, 2),
                (0, ww - 2),
                (0, ww - 1),
                (1, ww - 1),
                (2, ww - 1),
                (3, ww - 1),
            ])
        } else if r == hh - 2 && c == 0 && !w.is_multiple_of(4) {
            Some([
                (hh - 3, 0),
                (hh - 2, 0),
                (hh - 1, 0),
                (0, ww - 4),
                (0, ww - 3),
                (0, ww - 2),
                (0, ww - 1),
                (1, ww - 1),
            ])
        } else if r == hh + 4 && c == 2 && w.is_multiple_of(8) {
            Some([
                (hh - 1, 0),
                (hh - 1, ww - 1),
                (0, ww - 3),
                (0, ww - 2),
                (0, ww - 1),
                (1, ww - 3),
                (1, ww - 2),
                (1, ww - 1),
            ])
        } else if r == hh - 2 && c == 0 && w % 8 == 4 {
            Some([
                (hh - 3, 0),
                (hh - 2, 0),
                (hh - 1, 0),
                (0, ww - 2),
                (0, ww - 1),
                (1, ww - 1),
                (2, ww - 1),
                (3, ww - 1),
            ])
        } else {
            None
        };
        if let Some(p) = corner {
            write(&mut m, &mut occupied, code, word, &p);
            word += 1;
        }
        loop {
            if r < hh && c >= 0 && !occupied[r as usize * w + c as usize] {
                write(
                    &mut m,
                    &mut occupied,
                    code,
                    word,
                    &[
                        (r - 2, c - 2),
                        (r - 2, c - 1),
                        (r - 1, c - 2),
                        (r - 1, c - 1),
                        (r - 1, c),
                        (r, c - 2),
                        (r, c - 1),
                        (r, c),
                    ],
                );
                word += 1;
            }
            r -= 2;
            c += 2;
            if r < 0 || c >= ww {
                break;
            }
        }
        r += 1;
        c += 3;
        loop {
            if r >= 0 && c < ww && !occupied[r as usize * w + c as usize] {
                write(
                    &mut m,
                    &mut occupied,
                    code,
                    word,
                    &[
                        (r - 2, c - 2),
                        (r - 2, c - 1),
                        (r - 1, c - 2),
                        (r - 1, c - 1),
                        (r - 1, c),
                        (r, c - 2),
                        (r, c - 1),
                        (r, c),
                    ],
                );
                word += 1;
            }
            r += 2;
            c -= 2;
            if r >= hh || c < 0 {
                break;
            }
        }
        r += 3;
        c += 1;
        if r >= hh && c >= ww {
            break;
        }
    }
    if !occupied[w * h - 1] {
        m.set(w - 1, h - 1, true);
        m.set(w - 2, h - 2, true);
    }
    m
}
