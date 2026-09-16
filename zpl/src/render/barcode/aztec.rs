//! Original ISO/IEC 24778 binary-shift encoder, stuffing and layer placement.
use super::*;
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    b.require(2, "N", &["N"])?;
    b.require(4, "N", &["N"])?;
    b.require(5, "1", &["1"])?;
    b.require(6, "", &[""])?;
    let scale = b.num(1, b.scale(), 1., 10.)?;
    let size = b.integer(3, 0, 0, 300)?;
    if size == 300 {
        let n = ascii(data)?
            .parse::<u8>()
            .map_err(|_| "Aztec Rune requires a value 0..255")?;
        let mut matrix = Matrix::new(11, 11);
        finder(&mut matrix, 5);
        let mut words = vec![(n >> 4) as usize, (n & 15) as usize];
        words.extend(reed_solomon::parity(&words, 5, 0x13, 1));
        let mut message = Vec::new();
        for w in words {
            bits::push(&mut message, w, 4);
        }
        for (i, bit) in message.iter_mut().enumerate() {
            *bit ^= i % 2 == 0;
        }
        mode(&mut matrix, &message, true);
        return b.matrix(&matrix, scale, scale);
    }
    let mut input = Vec::new();
    for chunk in data.chunks(2078) {
        bits::push(&mut input, 31, 5);
        if chunk.len() <= 31 {
            bits::push(&mut input, chunk.len(), 5);
        } else {
            bits::push(&mut input, 0, 5);
            bits::push(&mut input, chunk.len() - 31, 11);
        }
        for &v in chunk {
            bits::push(&mut input, v as usize, 8);
        }
    }
    let percentage = if size == 0 {
        23
    } else if size <= 99 {
        size
    } else {
        0
    };
    let mut selected = None;
    for layers in 1..=32 {
        for compact in [true, false] {
            if compact && layers > 4 {
                continue;
            }
            if size >= 100 && size != layers + if compact { 100 } else { 200 } {
                continue;
            }
            let word = if layers <= 2 {
                6
            } else if layers <= 8 {
                8
            } else if layers <= 22 {
                10
            } else {
                12
            };
            let stuffed = stuff(&input, word);
            let total = ((if compact { 88 } else { 112 }) + 16 * layers) * layers;
            let words = total / word;
            if (!compact || stuffed.len() <= 64)
                && stuffed.len() * word + input.len() * percentage / 100 + 11 <= words * word
            {
                selected = Some((layers, compact, word, total, stuffed));
                break;
            }
        }
        if selected.is_some() {
            break;
        }
    }
    let (layers, compact, word, total, mut words) =
        selected.ok_or("Aztec data does not fit requested symbol size")?;
    let data_words = words.len();
    let polynomial = match word {
        6 => 0x43,
        8 => 0x12d,
        10 => 0x409,
        _ => 0x1069,
    };
    words.extend(reed_solomon::parity(
        &words,
        total / word - data_words,
        polynomial,
        1,
    ));
    let mut message = vec![false; total % word];
    for w in words {
        bits::push(&mut message, w, word);
    }
    let base = if compact { 11 } else { 14 } + 4 * layers;
    let size = if compact {
        base
    } else {
        base + 1 + 2 * ((base / 2 - 1) / 15)
    };
    let mut matrix = Matrix::new(size, size);
    let mut map: Vec<usize> = (0..base).collect();
    if !compact {
        for i in 0..base / 2 {
            let shift = i + i / 15;
            map[base / 2 - i - 1] = size / 2 - shift - 1;
            map[base / 2 + i] = size / 2 + shift + 1;
        }
    }
    let mut offset = 0;
    for layer in 0..layers {
        let n = (layers - layer) * 4 + if compact { 9 } else { 12 };
        let lo = 2 * layer;
        let hi = base - 1 - lo;
        for i in 0..n {
            for k in 0..2 {
                let j = 2 * i + k;
                for (x, y, index) in [
                    (lo + k, lo + i, offset + j),
                    (lo + i, hi - k, offset + 2 * n + j),
                    (hi - k, hi - i, offset + 4 * n + j),
                    (hi - i, lo + k, offset + 6 * n + j),
                ] {
                    matrix.set(map[x], map[y], message[index]);
                }
            }
        }
        offset += 8 * n;
    }
    finder(&mut matrix, if compact { 5 } else { 7 });
    let mut control = Vec::new();
    bits::push(&mut control, layers - 1, if compact { 2 } else { 5 });
    bits::push(&mut control, data_words - 1, if compact { 6 } else { 11 });
    let mut values: Vec<_> = control.chunks(4).map(bits::value).collect();
    values.extend(reed_solomon::parity(
        &values,
        if compact { 5 } else { 6 },
        0x13,
        1,
    ));
    let mut control = Vec::new();
    for value in values {
        bits::push(&mut control, value, 4);
    }
    mode(&mut matrix, &control, compact);
    if !compact {
        let center = size / 2;
        for d in (0..=center).step_by(16) {
            for i in 0..size {
                if i % 2 == center % 2 {
                    matrix.set(center + d, i, true);
                    matrix.set(center - d, i, true);
                    matrix.set(i, center + d, true);
                    matrix.set(i, center - d, true);
                }
            }
        }
    }
    b.matrix(&matrix, scale, scale)
}
fn stuff(input: &[bool], width: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < input.len() {
        let mut v = 0;
        for k in 0..width {
            v = (v << 1) | usize::from(input.get(i + k).copied().unwrap_or(true));
        }
        let prefix = v >> 1;
        if prefix == 0 {
            out.push(1);
            i += width - 1;
        } else if prefix == (1 << (width - 1)) - 1 {
            out.push(v & !1);
            i += width - 1;
        } else {
            out.push(v);
            i += width;
        }
    }
    out
}
fn finder(m: &mut Matrix, radius: usize) {
    let c = m.w / 2;
    for y in c - radius + 1..c + radius {
        for x in c - radius + 1..c + radius {
            m.set(x, y, x.abs_diff(c).max(y.abs_diff(c)) % 2 == 0);
        }
    }
    for (x, y) in [
        (c - radius, c - radius),
        (c - radius + 1, c - radius),
        (c - radius, c - radius + 1),
        (c + radius, c - radius),
        (c + radius, c - radius + 1),
        (c + radius, c + radius - 1),
    ] {
        m.set(x, y, true);
    }
}
fn mode(m: &mut Matrix, bits: &[bool], compact: bool) {
    let c = m.w / 2;
    let n = if compact { 7 } else { 10 };
    let r = if compact { 5 } else { 7 };
    for i in 0..n {
        let p = c - if compact { 3 } else { 5 } + i + if compact { 0 } else { i / 5 };
        for (x, y, v) in [
            (p, c - r, i),
            (c + r, p, n + i),
            (p, c + r, 3 * n - 1 - i),
            (c - r, p, 4 * n - 1 - i),
        ] {
            m.set(x, y, bits[v]);
        }
    }
}
