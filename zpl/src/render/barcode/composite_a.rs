//! Original four-column CC-A: ISO/IEC 24723:2010 §§8.3–8.5, Tables 9, 11, 12.
//! <https://www.iso.org/standard/51425.html>
use super::*;
pub(super) const CAPACITY: [usize; 5] = [78, 108, 138, 167, 197];
pub(super) fn capacity(length: usize) -> Option<usize> {
    CAPACITY.into_iter().find(|&n| n >= length)
}
pub(super) fn encode(bits: &[bool]) -> Matrix {
    let version = CAPACITY.iter().position(|&n| n == bits.len()).unwrap();
    let rows = version + 3;
    let ec = version + 4;
    let mut words = Vec::new();
    for group in bits.chunks(69) {
        let mut value = group
            .iter()
            .fold(0u128, |v, &bit| (v << 1) | u128::from(bit));
        let count = group.len() / 10 + 1;
        let first = words.len();
        words.resize(first + count, 0);
        for word in words[first..].iter_mut().rev() {
            *word = (value % 928) as usize;
            value /= 928;
        }
        debug_assert_eq!(value, 0);
    }
    words.extend(pdf417::parity(&words, ec));
    let start = [39, 42, 45, 33, 28][version]; // zero-based left RAP from Table 11
    let mut matrix = Matrix::new(99, rows);
    for y in 0..rows {
        let left = (start + y) % 52;
        let cluster = (start + y) % 3;
        let mut row = Vec::new();
        let rap = |row: &mut Vec<bool>, text: &str| {
            runs(
                row,
                &text
                    .bytes()
                    .map(|b| (b - b'0') as usize)
                    .collect::<Vec<_>>(),
            )
        };
        rap(&mut row, micropdf417::SIDE[left]);
        for x in 0..4 {
            append_pattern(
                &mut row,
                pdf417_patterns::PATTERNS[cluster][words[4 * y + x]],
                17,
            );
            if x == 1 {
                rap(&mut row, micropdf417::CENTER[(left + 32) % 52]);
            }
        }
        rap(&mut row, micropdf417::SIDE[(left + 64) % 52]);
        row.push(true);
        for (x, bit) in row.into_iter().enumerate() {
            matrix.set(x, y, bit);
        }
    }
    matrix
}
