//! Original GS1 DataBar Expanded, ISO/IEC 24724:2011 §7, Tables 10–16.
//! <https://www.iso.org/standard/51426.html>
//! General-purpose and compressed AI methods, §7.2.5.4, pp. 26–29.
use super::*;

fn message(data: &[u8], per_row: usize, printer_no_date: bool) -> Result<Vec<usize>, String> {
    if data.len() > 74 {
        return Err("DataBar Expanded capacity exceeded".into());
    }
    if data.is_empty() {
        return Err("DataBar Expanded requires a GS1 AI element string".into());
    }
    let (mut header, consumed, length_bits) = compressed_header(data);
    let capacity = |length: usize| {
        let mut count = length.div_ceil(12).max(3) + 1;
        if count > per_row && count % per_row == 1 {
            count += 1;
        }
        (count <= 22).then_some((count - 1) * 12)
    };
    let out = if printer_no_date && data.len() == 26 && header.len() == 84 {
        // ZD621 controls kg-long/lb2-long/lb3-long/weight-max: the preview
        // departs from ISO/IEC 24724:2011 §7.2.5.4.4's no-date method 56/57.
        // It writes header 54/53 and encodes the last weight digit again in
        // a general-purpose field. This malformed preview is profile-only.
        let method = if data[17] == b'1' { 54 } else { 53 };
        let mut prefix = vec![false];
        bits::push(&mut prefix, method, 7);
        header[..8].copy_from_slice(&prefix);
        let mut out = gs1_compaction::encode_remainder(&data[25..], header, capacity)?;
        if data[17] == b'1' {
            // The metric header also carries the ordinary length parity bit:
            // 54 for nine characters, 52 when stacking pads it to ten.
            out[6] = (out.len() / 12 + 1) % 2 == 1;
        }
        out
    } else if let Some(position) = length_bits {
        let mut out = gs1_compaction::encode_remainder(&data[consumed..], header, capacity)?;
        let count = out.len() / 12 + 1;
        out[position] = count % 2 == 1;
        out[position + 1] = count > 14;
        out
    } else {
        header
    };
    Ok(out
        .as_chunks::<12>()
        .0
        .iter()
        .map(|word| bits::value(word))
        .collect())
}

/// ISO/IEC 24724:2011 §7.2.5.4 and Table 10. Only compress a checked
/// GTIN; otherwise retain the complete input in general-purpose method 00.
fn compressed_header(data: &[u8]) -> (Vec<bool>, usize, Option<usize>) {
    let number = |d: &[u8]| d.iter().fold(0usize, |v, c| v * 10 + (c - b'0') as usize);
    if data.len() < 16
        || !data.starts_with(b"01")
        || !data[2..16].iter().all(u8::is_ascii_digit)
        || mod10(&data[2..15].iter().map(|c| c - b'0').collect::<Vec<_>>()) != data[15] - b'0'
    {
        return (vec![false; 5], 0, Some(3));
    }
    let mut method = 1;
    let mut method_bits = 1;
    let mut consumed = 16;
    let mut extra = Vec::new();
    let mut variable = true;
    if data[2] == b'9' {
        if matches!(data.len(), 26 | 34)
            && data[16..].iter().all(u8::is_ascii_digit)
            && (data[16..19] == *b"310" || data[16..19] == *b"320")
        {
            let weight = number(&data[20..26]);
            let ai = &data[16..20];
            let short_weight = match ai {
                b"3103" if weight <= 32767 => Some((4, weight)),
                b"3202" if weight <= 9999 => Some((5, weight)),
                b"3203" if weight <= 22767 => Some((5, weight + 10000)),
                _ => None,
            };
            if let Some((m, w)) = short_weight.filter(|_| data.len() == 26) {
                method = m;
                method_bits = 4;
                consumed = 26;
                variable = false;
                bits::push(&mut extra, w, 15);
            } else if weight <= 99999 {
                let date = if data.len() == 26 {
                    Some((0, 38400))
                } else {
                    let ai = number(&data[26..28]);
                    let month = number(&data[30..32]);
                    let day = number(&data[32..34]);
                    if matches!(ai, 11 | 13 | 15 | 17) && (1..=12).contains(&month) && day <= 31 {
                        Some((
                            (ai - 11) / 2,
                            number(&data[28..30]) * 384 + (month - 1) * 32 + day,
                        ))
                    } else {
                        None
                    }
                };
                if let Some((kind, date)) = date {
                    method = 56 + kind * 2 + usize::from(data[17] == b'2');
                    method_bits = 7;
                    consumed = data.len();
                    variable = false;
                    bits::push(&mut extra, (data[19] - b'0') as usize * 100000 + weight, 20);
                    bits::push(&mut extra, date, 16);
                }
            }
        } else if data.len() > 20
            && data[16..18] == *b"39"
            && matches!(data[18], b'2' | b'3')
            && (b'0'..=b'3').contains(&data[19])
            && (data[18] == b'2' || data.len() > 23 && data[20..23].iter().all(u8::is_ascii_digit))
        {
            method = if data[18] == b'2' { 12 } else { 13 };
            method_bits = 5;
            consumed = if method == 12 { 20 } else { 23 };
            bits::push(&mut extra, (data[19] - b'0') as usize, 2);
            if method == 13 {
                bits::push(&mut extra, number(&data[20..23]), 10);
            }
        }
    }
    let mut out = vec![false]; // Standalone, no linked 2D component.
    bits::push(&mut out, method, method_bits);
    let length_bits = variable.then_some(out.len());
    if variable {
        out.extend([false; 2]);
    }
    if method == 1 {
        bits::push(&mut out, (data[2] - b'0') as usize, 4);
    }
    for group in data[3..15].as_chunks::<3>().0 {
        bits::push(&mut out, number(group), 10);
    }
    out.extend(extra);
    (out, consumed, length_bits)
}

fn character(value: usize) -> [usize; 8] {
    const GROUPS: [(usize, usize, usize, usize, usize); 5] = [
        (0, 12, 7, 2, 4),
        (348, 10, 5, 4, 20),
        (1388, 8, 4, 5, 52),
        (2948, 6, 3, 6, 104),
        (3988, 4, 1, 8, 204),
    ];
    let &(base, sum, om, em, count) = GROUPS.iter().rev().find(|g| value >= g.0).unwrap();
    let odd = databar_limited::compositions::<4>(sum, om, true);
    let odd = odd
        .iter()
        .filter(|v| v[0] < 5)
        .nth((value - base) / count)
        .unwrap();
    let even = databar_limited::compositions::<4>(17 - sum, em, false)[(value - base) % count];
    std::array::from_fn(|i| if i % 2 == 0 { odd[i / 2] } else { even[i / 2] })
}

struct Row {
    bits: Vec<bool>,
    separator: Vec<bool>,
}

pub(super) fn render(b: &Barcode, data: &[u8], separator: usize) -> Result<Path, String> {
    let per_row = b.integer(5, 22, 2, 22)?;
    if per_row % 2 != 0 {
        return Err("DataBar Expanded segments per row must be even".into());
    }
    b.matrix(
        &encode(
            data,
            per_row,
            separator,
            b.compatibility.databar_expanded_no_date_preview,
        )?,
        b.module,
        b.module,
    )
}

fn encode(
    data: &[u8],
    per_row: usize,
    separator: usize,
    printer_no_date: bool,
) -> Result<Matrix, String> {
    let words = message(data, per_row, printer_no_date)?;
    let count = words.len() + 1;
    const FINDERS: [&[usize]; 10] = [
        &[0, 1],
        &[0, 3, 2],
        &[0, 5, 2, 7],
        &[0, 9, 2, 7, 4],
        &[0, 9, 2, 7, 6, 11],
        &[0, 9, 2, 7, 8, 11, 10],
        &[0, 1, 2, 3, 4, 5, 6, 7],
        &[0, 1, 2, 3, 4, 5, 6, 9, 8],
        &[0, 1, 2, 3, 4, 5, 6, 9, 10, 11],
        &[0, 1, 2, 3, 4, 7, 6, 9, 8, 11, 10],
    ];
    let finders = FINDERS[count.div_ceil(2) - 2];
    let mut chars: Vec<_> = words.into_iter().map(character).collect();
    let mut checksum = 0;
    for (i, widths) in chars.iter().enumerate() {
        let pos = i + 1;
        let exponent = (finders[pos / 2] * 2 + pos % 2 - 1) * 8;
        let mut weight = (0..exponent).fold(1, |w, _| w * 3 % 211);
        for width in widths {
            checksum = (checksum + width * weight) % 211;
            weight = weight * 3 % 211;
        }
    }
    chars.insert(0, character(211 * (count - 4) + checksum));
    const PATTERNS: [[usize; 5]; 6] = [
        [1, 8, 4, 1, 1],
        [3, 6, 4, 1, 1],
        [3, 4, 6, 1, 1],
        [3, 2, 8, 1, 1],
        [2, 6, 5, 1, 1],
        [2, 2, 9, 1, 1],
    ];
    let mut rows = Vec::new();
    for (r, chunk) in chars.chunks(per_row).enumerate() {
        let start = r * per_row / 2;
        let mut elements = vec![1, 1];
        let mut ranges = Vec::new();
        for (p, pair) in chunk.chunks(2).enumerate() {
            elements.extend(pair[0]);
            let finder = finders[start + p];
            let x = elements.iter().sum::<usize>();
            ranges.push(x + if finder.is_multiple_of(2) { 0 } else { 2 });
            let mut pattern = PATTERNS[finder / 2];
            if finder % 2 == 1 {
                pattern.reverse();
            }
            elements.extend(pattern);
            if pair.len() == 2 {
                elements.extend(pair[1].iter().rev());
            }
        }
        elements.extend([1, 1]);
        let mut row = Vec::new();
        runs(&mut row, &elements);
        // The colour phase follows the triplets in the unstacked symbol.
        if start.is_multiple_of(2) {
            for bit in &mut row {
                *bit = !*bit;
            }
        }
        if r % 2 == 1 && (per_row / 2).is_multiple_of(2) {
            if chunk.len() < per_row && chunk.len().div_ceil(2) % 2 == 1 {
                row.insert(0, false);
                for x in &mut ranges {
                    *x += 1;
                }
            } else {
                row.reverse();
                for x in &mut ranges {
                    *x = row.len() - *x - 13;
                }
            }
        }
        let mut sep: Vec<_> = row.iter().map(|v| !v).collect();
        for x in ranges {
            let mut previous = false;
            for at in x..x + 13 {
                sep[at] = !row[at] && !previous;
                previous = sep[at];
            }
        }
        let width = sep.len();
        sep[..4].fill(false);
        sep[width - 4..].fill(false);
        rows.push(Row {
            bits: row,
            separator: sep,
        });
    }
    let width = rows.iter().map(|r| r.bits.len()).max().unwrap();
    let height = 34 * rows.len() + 3 * separator * (rows.len() - 1);
    let mut matrix = Matrix::new(width, height);
    let mut y = 0;
    for (i, row) in rows.iter().enumerate() {
        for _ in 0..34 {
            for (x, &v) in row.bits.iter().enumerate() {
                matrix.set(x, y, v);
            }
            y += 1;
        }
        if let Some(next) = rows.get(i + 1) {
            let middle: Vec<_> = (0..width)
                .map(|x| x >= 4 && x < width - 4 && x % 2 == 1)
                .collect();
            for line in [&row.separator, &middle, &next.separator] {
                for _ in 0..separator {
                    for (x, &v) in line.iter().enumerate() {
                        matrix.set(x, y, v);
                    }
                    y += 1;
                }
            }
        }
    }
    Ok(matrix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyd::traits::Decode;
    #[test]
    fn independent_padding_and_stacking() {
        for length in 1..=68 {
            let data = format!("91{}", &"1234567890".repeat(7)[..length]);
            for per_row in (2..=22).step_by(2) {
                let m = encode(data.as_bytes(), per_row, 1, false).unwrap();
                let encoding = if m.h == 34 {
                    anyd::output::Encoding::Linear(anyd::output::LinearPattern {
                        modules: m.cells[..m.w].to_vec(),
                        quiet_zone: 1,
                    })
                } else {
                    // anyd's abstract matrix decoder expects one module row for
                    // each tall data row and each of the three separator rows.
                    let rows = (m.h + 3) / 37;
                    let mut sampled = anyd::output::BitMatrix::new(m.w, rows * 4 - 3, 1);
                    for y in 0..sampled.height() {
                        let source = (y / 4) * 37 + if y % 4 == 0 { 0 } else { 33 + y % 4 };
                        for x in 0..m.w {
                            sampled.set(x, y, m.get(x, source));
                        }
                    }
                    anyd::output::Encoding::Matrix(sampled)
                };
                let decoded = anyd::codes::databar::DataBarDecoder::new()
                    .decode(&encoding)
                    .unwrap_or_else(|e| panic!("length {length}, segments {per_row}: {e}"));
                assert_eq!(
                    decoded.text().unwrap(),
                    data,
                    "length {length}, segments {per_row}"
                );
            }
        }
    }
}
