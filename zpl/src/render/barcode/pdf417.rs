//! Original PDF417 encoding, prime-field RS and row construction.
//! USS PDF417 §§2.2–2.6; Zebra programming guide printed pp. 79–82.
//! <https://www.expresscorp.com/wp-content/uploads/2023/02/USS-PDF-417.pdf>
#[path = "pdf417_compaction.rs"]
mod high_level;
use super::*;

pub(super) fn compact_macro(data: &[u8]) -> Vec<usize> {
    high_level::encode_macro(data)
}

pub(super) fn compact_micro(data: &[u8]) -> Vec<usize> {
    high_level::encode_micro(data)
}
pub(super) fn compact_tlc(data: &[u8]) -> Vec<usize> {
    high_level::encode_tlc(data)
}

// Shared byte packing for composite components.
pub(super) fn compact(data: &[u8]) -> Vec<usize> {
    let mut out = vec![if data.len().is_multiple_of(6) {
        924
    } else {
        901
    }];
    for chunk in data.chunks(6) {
        if chunk.len() == 6 {
            let mut n = chunk.iter().fold(0u64, |n, &v| (n << 8) | v as u64);
            let mut digits = [0; 5];
            for d in digits.iter_mut().rev() {
                *d = (n % 900) as usize;
                n /= 900;
            }
            out.extend(digits);
        } else {
            out.extend(chunk.iter().map(|&v| v as usize));
        }
    }
    out
}
pub(super) fn parity(data: &[usize], count: usize) -> Vec<usize> {
    let mut generator = vec![1usize];
    let mut root = 1;
    for _ in 0..count {
        root = root * 3 % 929;
        let mut next = vec![0; generator.len() + 1];
        for (i, &v) in generator.iter().enumerate() {
            next[i] = (next[i] + v) % 929;
            next[i + 1] = (next[i + 1] + 929 - v * root % 929) % 929;
        }
        generator = next;
    }
    let mut work = data.to_vec();
    work.resize(data.len() + count, 0);
    for i in 0..data.len() {
        let lead = work[i];
        for j in 1..generator.len() {
            work[i + j] = (work[i + j] + 929 - lead * generator[j] % 929) % 929;
        }
    }
    work[data.len()..]
        .iter()
        .map(|&v| (929 - v) % 929)
        .collect()
}
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    let ec = b.integer(2, 0, 0, 8)?;
    let count = 1 << (ec + 1);
    let payload = field_escapes(data);
    render_payload(b, &payload, ec, count)
}

pub(super) fn field_escapes(data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'\\' && data.get(i + 1) == Some(&b'&') {
            payload.extend_from_slice(b"\r\n");
            i += 2;
        } else if data[i] == b'\\' && data.get(i + 1) == Some(&b'\\') {
            payload.push(b'\\');
            i += 2;
        } else {
            payload.push(data[i]);
            i += 1;
        }
    }
    payload
}

fn render_payload(b: &Barcode, payload: &[u8], ec: usize, count: usize) -> Result<Path, String> {
    let stream = high_level::encode(payload);
    let needed = stream.len() + 1 + count;
    let mut cols = b.integer(3, 0, 0, 30)?;
    let mut rows = b.integer(4, 0, 0, 90)?;
    if cols == 0 {
        cols = if rows != 0 {
            needed.div_ceil(rows)
        } else {
            // Nominal Y=3X, width:height=2:1, including start/stop and row
            // indicators: (17c + 69)c = 6n. ZD621 chooses dimensions before
            // applying explicit/BY row height. Round the positive root.
            (((69. * 69. + 408. * needed as f64).sqrt() - 69.) / 34.).round() as usize
        };
        cols = cols.max(needed.div_ceil(90)).max(1);
    }
    if rows == 0 {
        rows = needed.div_ceil(cols).max(3);
    }
    if cols > 30 || !(3..=90).contains(&rows) || rows * cols < needed || rows * cols > 928 {
        return Err("PDF417 data does not fit requested rows/columns".into());
    }
    let capacity = rows * cols - count;
    let mut values = vec![capacity];
    values.extend(stream);
    values.resize(capacity, 900);
    values.extend(parity(&values, count));
    let truncated = b.flag(5, false)?;
    let m = matrix(&values, cols, rows, ec, truncated);
    // Explicit h is dots on the tested firmware (despite contradictory prose
    // about multiplying by X in the guide). Omitted h divides the BY height.
    let row_height = (b.height / rows as f64).floor().max(1.);
    b.matrix(&m, b.module, b.num(1, row_height, 1., 32000.)?)
}

pub(super) fn matrix(
    values: &[usize],
    cols: usize,
    rows: usize,
    ec: usize,
    truncated: bool,
) -> Matrix {
    let width = 17 * cols + if truncated { 35 } else { 69 };
    let mut m = Matrix::new(width, rows);
    for y in 0..rows {
        let group = y % 3;
        let common = 30 * (y / 3);
        let a = (rows - 1) / 3;
        let q = ec * 3 + (rows - 1) % 3;
        let z = cols - 1;
        let left = common + [a, q, z][group];
        let right = common + [z, a, q][group];
        let mut out = Vec::new();
        append_pattern(&mut out, 0x1fea8, 17);
        append_pattern(&mut out, pdf417_patterns::PATTERNS[group][left], 17);
        for &v in &values[y * cols..(y + 1) * cols] {
            append_pattern(&mut out, pdf417_patterns::PATTERNS[group][v], 17);
        }
        if truncated {
            out.push(true);
        } else {
            append_pattern(&mut out, pdf417_patterns::PATTERNS[group][right], 17);
            append_pattern(&mut out, 0x3fa29, 18);
        }
        for (x, &v) in out.iter().enumerate() {
            m.set(x, y, v);
        }
    }
    m
}
