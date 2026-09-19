//! Original USS Code 49 encodation, row parity and weighted symbol checks.
//! <https://www.expresscorp.com/wp-content/uploads/2023/02/USS-49.pdf>
use super::*;
const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%";
fn message(data: &[u8], automatic: bool) -> Result<Vec<usize>, String> {
    let mut result = Vec::new();
    for &c in data {
        if let Some(n) = ALPHABET.iter().position(|&v| v == c) {
            result.push(n);
            continue;
        }
        if !automatic {
            let n = b":;<=>?"
                .iter()
                .position(|&v| v == c)
                .ok_or("invalid Code 49 internal character")?;
            result.push(43 + n);
            continue;
        }
        let (shift, n) = match c {
            0 => (43, 38),
            1..=26 => (43, c as usize + 9),
            27..=31 => (43, c as usize - 26),
            33..=35 => (43, c as usize - 27),
            38 => (43, 9),
            39 => (43, 0),
            40 => (43, 36),
            41 => (43, 37),
            42 => (43, 39),
            44 => (43, 40),
            58 => (43, 41),
            59..=64 => (44, c as usize - 58),
            91..=93 => (44, c as usize - 84),
            94 => (44, 0),
            95 => (44, 36),
            96 => (44, 37),
            97..=122 => (44, c as usize - 87),
            123..=126 => (44, c as usize - 84),
            127 => (44, 38),
            _ => return Err("Code 49 requires 7-bit ASCII".into()),
        };
        result.extend([shift, n]);
    }
    Ok(result)
}
// USS Code 49 §2.2.2: base-48 triples, including the special 4+3 tail
// needed when the digit count is congruent to two modulo five.
fn numeric(mut data: &[u8], out: &mut Vec<usize>) {
    let decimal = |s: &[u8]| s.iter().fold(0usize, |n, &c| n * 10 + (c - b'0') as usize);
    let triple = |n: usize, out: &mut Vec<usize>| out.extend([n / 2304, n / 48 % 48, n % 48]);
    while data.len() >= 5 && data.len() != 7 {
        triple(decimal(&data[..5]), out);
        data = &data[5..];
    }
    if data.len() == 7 {
        triple(100_000 + decimal(&data[..4]), out);
        data = &data[4..];
    }
    match data.len() {
        0 => (),
        1 => out.push(decimal(data)),
        3 => {
            let n = decimal(data);
            out.extend([n / 48, n % 48]);
        }
        4 => triple(100_000 + decimal(data), out),
        _ => unreachable!("numeric runs contain at least five digits"),
    }
}
fn automatic_message(data: &[u8]) -> Result<(usize, Vec<usize>), String> {
    let mut out = Vec::new();
    let mut mode = 0;
    let mut pos = 0;
    while pos < data.len() {
        let n = data[pos..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if n >= 5 {
            if pos == 0 {
                mode = 2;
            } else {
                out.push(48);
            }
            numeric(&data[pos..pos + n], &mut out);
            pos += n;
            if pos < data.len() {
                out.push(48);
            }
        } else {
            out.extend(message(&data[pos..pos + 1], true)?);
            pos += 1;
        }
    }
    if mode == 0 && matches!(out.first(), Some(43 | 44)) {
        // Initial Shift 1/2 is carried by the starting-mode indicator.
        mode = out.remove(0) - 39;
    }
    Ok((mode, out))
}
fn encode(b: &Barcode, data: &[u8]) -> Result<Matrix, String> {
    b.require(2, "N", &["N", "A", "B"])?;
    let (mode, values) = if b.param(3, "A") == "A" {
        automatic_message(data)?
    } else {
        (b.integer(3, 0, 0, 5)?, message(data, false)?)
    };
    let rows = (2..=8)
        .find(|&n| values.len() <= (n - 1) * 7 + if n <= 6 { 2 } else { 0 })
        .ok_or("Code 49 exceeds eight rows")?;
    let mut grid = vec![[48usize; 8]; rows];
    let mut cursor = 0;
    for (y, row) in grid.iter_mut().enumerate() {
        let capacity = if y + 1 < rows {
            7
        } else if rows <= 6 {
            2
        } else {
            0
        };
        for cell in &mut row[..capacity] {
            if let Some(&v) = values.get(cursor) {
                *cell = v;
                cursor += 1;
            }
        }
        if y + 1 < rows {
            row[7] = row[..7].iter().sum::<usize>() % 49;
        }
    }
    let indicator = 7 * (rows - 2) + mode;
    grid[rows - 1][6] = indicator;
    const WEIGHT: [usize; 34] = [
        1, 9, 31, 26, 2, 12, 17, 23, 37, 18, 22, 6, 27, 44, 15, 43, 39, 11, 13, 5, 41, 33, 36, 8,
        4, 32, 3, 19, 40, 25, 29, 10, 24, 30,
    ];
    for column in if rows >= 7 { 0 } else { 1 }..3 {
        let offset = 2 - column;
        let mut sum = [38, 16, 20][column] * indicator;
        for (y, row) in grid.iter().enumerate() {
            let end = if y + 1 == rows { column } else { 4 };
            for j in 0..end {
                sum += WEIGHT[y * 4 + j + offset] * (49 * row[2 * j] + row[2 * j + 1]);
            }
        }
        let check = sum % 2401;
        grid[rows - 1][2 * column] = check / 49;
        grid[rows - 1][2 * column + 1] = check % 49;
    }
    grid[rows - 1][7] = grid[rows - 1][..7].iter().sum::<usize>() % 49;
    const EVEN: [[bool; 4]; 7] = [
        [false, true, true, false],
        [true, false, true, false],
        [false, false, true, true],
        [true, true, false, false],
        [false, true, false, true],
        [true, false, false, true],
        [false, false, false, false],
    ];
    let mut m = Matrix::new(70, rows);
    for (y, row) in grid.iter().enumerate() {
        let mut bits = vec![true, false];
        for j in 0..4 {
            let index = row[j * 2] * 49 + row[j * 2 + 1];
            let pattern = if y + 1 == rows || EVEN[y][j] {
                code49_patterns::EVEN[index]
            } else {
                code49_patterns::ODD[index]
            };
            append_pattern(&mut bits, pattern as u32, 16);
        }
        bits.extend([true; 4]);
        for (x, v) in bits.into_iter().enumerate() {
            m.set(x, y, v);
        }
    }
    Ok(m)
}
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    let m = encode(b, data)?;
    let height = b.height * b.module;
    let mut p = Path::default();
    for y in 0..m.h {
        let mut row = Matrix::new(70, 1);
        for x in 0..70 {
            row.set(x, 0, m.get(x, y));
        }
        let mut path = b.matrix(&row, b.module, height)?;
        path.transform(|p| {
            Point::new(
                p.x + 10. * b.module,
                p.y + b.module + y as f64 * (height + b.module),
            )
        });
        p.segments.extend(path.segments);
    }
    for y in 0..=m.h {
        let outer = y == 0 || y == m.h;
        p.rect(
            if outer { 0. } else { 10. * b.module },
            y as f64 * (height + b.module),
            if outer { 81. } else { 70. } * b.module,
            b.module,
        );
    }
    Ok(p)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_spec_vectors_and_decoder_framing() {
        for (data, expected) in [
            ("12345", vec![5, 17, 9]),
            ("123456", vec![5, 17, 9, 6]),
            ("12345678", vec![5, 17, 9, 14, 6]),
            ("123456789", vec![5, 17, 9, 46, 16, 37]),
            ("1234567", vec![43, 45, 2, 11, 39]),
        ] {
            let mut words = Vec::new();
            numeric(data.as_bytes(), &mut words);
            assert_eq!(words, expected);
        }
        let b = Barcode::new(
            "B4",
            &["N", "8", "N", "A"],
            2.,
            2.,
            90.,
            203,
            Default::default(),
        )
        .unwrap();
        for length in 1..=81 {
            let digits: String = (0..length)
                .map(|i| (b'0' + (i % 10) as u8) as char)
                .collect();
            {
                let data = digits;
                let m = encode(&b, data.as_bytes()).unwrap();
                let mut other = anyd::output::BitMatrix::new(m.w, m.h, 0);
                for y in 0..m.h {
                    for x in 0..m.w {
                        other.set(x, y, m.get(x, y));
                    }
                }
                let decoded = anyd::codes::code49::Code49Decoder::new().decode_matrix(&other);
                if length >= 5 {
                    // anyd checks the symbol but cannot reconstruct numeric
                    // payloads. Specification vectors and exact printer images
                    // cover compaction; do not call this a decoder round trip.
                    let error = decoded.unwrap_err().to_string();
                    assert!(
                        error.contains("numeric-mode payload reconstruction is not implemented"),
                        "{data}: {error}"
                    );
                } else {
                    assert_eq!(decoded.unwrap().payload_bytes(), data.as_bytes(), "{data}");
                }
            }
        }
    }
    #[test]
    fn full_ascii_round_trip() {
        let b = Barcode::new(
            "B4",
            &["N", "8", "N", "0"],
            2.,
            2.,
            90.,
            203,
            Default::default(),
        )
        .unwrap();
        for first in (0u8..128).step_by(16) {
            let data: Vec<_> = (first..first + 16).collect();
            // Manual alphanumeric input avoids numeric-mode reconstruction,
            // which the independent decoder does not implement.
            let internal: Vec<_> = message(&data, true)
                .unwrap()
                .into_iter()
                .map(|v| {
                    if v < 43 {
                        ALPHABET[v]
                    } else {
                        b":;<=>?"[v - 43]
                    }
                })
                .collect();
            let m = encode(&b, &internal).unwrap();
            let mut other = anyd::output::BitMatrix::new(m.w, m.h, 0);
            for y in 0..m.h {
                for x in 0..m.w {
                    other.set(x, y, m.get(x, y));
                }
            }
            let result = anyd::codes::code49::Code49Decoder::new()
                .decode_matrix(&other)
                .unwrap();
            assert_eq!(result.payload_bytes(), data);
        }
    }
    #[test]
    fn independent_decode() {
        let b = Barcode::new(
            "B4",
            &["N", "8", "N", "A"],
            2.,
            2.,
            90.,
            203,
            Default::default(),
        )
        .unwrap();
        for length in 1..=49 {
            let data: Vec<_> = (0..length)
                .map(|i| ALPHABET[(i * 7 + length) % 43])
                .collect();
            let m = encode(&b, &data).unwrap();
            let mut other = anyd::output::BitMatrix::new(m.w, m.h, 0);
            for y in 0..m.h {
                for x in 0..m.w {
                    other.set(x, y, m.get(x, y));
                }
            }
            let decoded = anyd::codes::code49::Code49Decoder::new()
                .decode_matrix(&other)
                .unwrap_or_else(|e| panic!("{length}: {e}"));
            assert_eq!(decoded.text().unwrap().as_bytes(), data);
        }
    }
}
