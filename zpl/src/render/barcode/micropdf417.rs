//! Original ISO/IEC 24728 MicroPDF417 row addressing and size selection.
//! <https://previewnorm.com/iec/ISO%20IEC%2024728-2006%20PDF.pdf>
//! ZPL ^BF: bundled Programming Guide pp. 111–113; firmware observations are
//! recorded in tests/fixtures/micropdf417-zd621-v1 in zebra-http-api.
use super::*;
pub(super) const ROWS: [usize; 34] = [
    11, 14, 17, 20, 24, 28, 8, 11, 14, 17, 20, 23, 26, 6, 8, 10, 12, 15, 20, 26, 32, 38, 44, 4, 6,
    8, 10, 12, 15, 20, 26, 32, 38, 44,
];
pub(super) const EC: [usize; 34] = [
    7, 7, 7, 8, 8, 8, 8, 9, 9, 10, 11, 13, 15, 12, 14, 16, 18, 21, 26, 32, 38, 44, 50, 8, 12, 14,
    16, 18, 21, 26, 32, 38, 44, 50,
];
const START: [usize; 34] = [
    1, 8, 36, 19, 9, 25, 1, 1, 8, 36, 19, 9, 27, 1, 7, 15, 25, 37, 1, 1, 21, 15, 1, 47, 1, 7, 15,
    25, 37, 1, 1, 21, 15, 1,
];
// Table 2: alternating bar/space widths, six elements totalling ten modules.
pub(super) const SIDE: [&str; 52] = [
    "221311", "311311", "312211", "222211", "213211", "214111", "223111", "313111", "322111",
    "412111", "421111", "331111", "241111", "232111", "231211", "321211", "411211", "411121",
    "411112", "321112", "312112", "311212", "311221", "311131", "311122", "311113", "221113",
    "221122", "221131", "221221", "222121", "312121", "321121", "231121", "231112", "222112",
    "213112", "212212", "212221", "212131", "212122", "212113", "211213", "211123", "211132",
    "211141", "211231", "211222", "211312", "211321", "211411", "212311",
];
pub(super) const CENTER: [&str; 52] = [
    "112231", "121231", "122131", "131131", "131221", "132121", "141121", "141211", "142111",
    "133111", "132211", "131311", "122311", "123211", "124111", "115111", "114211", "114121",
    "123121", "123112", "122212", "122221", "121321", "121411", "112411", "113311", "113221",
    "113212", "113122", "122122", "131122", "131113", "122113", "113113", "112213", "112222",
    "112312", "112321", "111421", "111331", "111322", "111232", "111223", "111133", "111124",
    "111214", "112114", "121114", "121123", "121132", "112132", "112141",
];
pub(super) fn columns(mode: usize) -> usize {
    match mode {
        0..=5 => 1,
        6..=12 => 2,
        13..=22 => 3,
        _ => 4,
    }
}
pub(super) fn encode(data: &[u8], mode: usize) -> Result<Matrix, String> {
    // Zebra ^BF puts the 4-by-4 variant last; ISO table indices used by
    // linked/composite symbols retain their column/row ordering.
    let variant = match mode {
        0..=22 => mode,
        23..=32 => mode + 1,
        33 => 23,
        _ => return Err("MicroPDF417 mode must be 0 through 33".into()),
    };
    let mut words = pdf417::compact_micro(data);
    let capacity = columns(variant) * ROWS[variant] - EC[variant];
    // ZD621 uses this repeating sequence of text-submode latches as padding.
    // Each pair changes state without emitting data, starting from Alpha (900).
    const PAD: [usize; 8] = [900, 838, 779, 867, 865, 898, 868, 839];
    let needed = capacity.saturating_sub(words.len());
    let period = if columns(variant) == 3 { 24 } else { 25 };
    words.extend((0..needed).map(|i| PAD[(i % period) % PAD.len()]));
    encode_words(words, variant)
}

// TLC39 adds a linkage codeword before the ordinary compaction latch.
pub(super) fn linked(data: &[u8]) -> Result<Matrix, String> {
    let mut words = vec![918];
    words.extend(pdf417::compact(data));
    let mode = (23..34)
        .find(|&i| 4 * ROWS[i] - EC[i] >= words.len())
        .ok_or("TLC39 payload exceeds four-column MicroPDF417 capacity")?;
    encode_words(words, mode)
}

pub(super) fn encode_words(mut values: Vec<usize>, mode: usize) -> Result<Matrix, String> {
    let cols = columns(mode);
    let rows = ROWS[mode];
    let capacity = cols * rows - EC[mode];
    if values.len() > capacity {
        return Err("MicroPDF417 data exceeds selected mode capacity".into());
    }
    values.resize(capacity, 900);
    values.extend(pdf417::parity(&values, EC[mode]));
    let rotate = if cols < 3 {
        if matches!(mode, 0 | 4 | 5 | 7 | 11 | 12) {
            8
        } else {
            0
        }
    } else {
        match rows {
            4 | 44 => 24,
            20 | 38 => 16,
            26 | 32 => 8,
            _ => 0,
        }
    };
    let mut m = Matrix::new(17 * cols + 21 + if cols >= 3 { 10 } else { 0 }, rows);
    for y in 0..rows {
        let left = (START[mode] - 1 + y) % 52;
        let cluster = (START[mode] - 1 + y) % 3;
        let mut out = Vec::new();
        let rap = |out: &mut Vec<bool>, text: &str| {
            let w: Vec<_> = text.bytes().map(|b| (b - b'0') as usize).collect();
            runs(out, &w);
        };
        rap(&mut out, SIDE[left]);
        for x in 0..cols {
            append_pattern(
                &mut out,
                pdf417_patterns::PATTERNS[cluster][values[y * cols + x]],
                17,
            );
            if cols >= 3 && x == cols - 3 {
                rap(&mut out, CENTER[(left + rotate) % 52]);
            }
        }
        rap(
            &mut out,
            SIDE[(left + if cols < 3 { rotate } else { 2 * rotate }) % 52],
        );
        out.push(true);
        for (x, v) in out.into_iter().enumerate() {
            m.set(x, y, v);
        }
    }
    Ok(m)
}
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    let matrix = encode(&pdf417::field_escapes(data), b.integer(2, 0, 0, 33)?)?;
    // Like ^B7, this firmware divides the BY overall height by the row count.
    // An explicit zero also selects automatic height.
    let height = b.num(1, 0., 0., 9999.)?;
    let height = if height == 0. {
        (b.height / matrix.h as f64).floor().max(1.)
    } else {
        height
    };
    b.matrix(&matrix, b.module, height)
}
