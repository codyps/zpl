//! Original legacy Data Matrix encoder. ISO/IEC 16022:2000 §§5.2–5.7,
//! Annexes B–F (pp. 32–55): radix compaction, CRC, convolution and placement.
//! <https://previewnorm.com/iec/ISO%20IEC%2016022-2000%20PDF.pdf>
//! The convolution circuits are also Figures K.1–K.4 in the 2005 FCD (pp. 102–103):
//! <https://www.yumpu.com/en/document/view/11783336/information-technology-automatic-identification-and-openbio>
use super::*;

// Annex F's fixed whitening sequence, MSB first, including the final zero bit.
const RANDOM: &[u8] = &[
    0x05, 0xff, 0xc7, 0x31, 0x88, 0xa8, 0x83, 0x9c, 0x64, 0x87, 0x9f, 0x64, 0xb3, 0xe0, 0x4d, 0x9c,
    0x80, 0x29, 0x3a, 0x90, 0xb3, 0x8b, 0x9e, 0x90, 0x45, 0xbf, 0xf5, 0x68, 0x4b, 0x08, 0xcf, 0x44,
    0xb8, 0xd4, 0x4c, 0x5b, 0xa0, 0xab, 0x72, 0x52, 0x1c, 0xe4, 0xd2, 0x74, 0xa4, 0xda, 0x8a, 0x08,
    0xfa, 0xa7, 0xc7, 0xdd, 0x00, 0x30, 0xa9, 0xe6, 0x64, 0xab, 0xd5, 0x8b, 0xed, 0x9c, 0x79, 0xf8,
    0x08, 0xd1, 0x8b, 0xc6, 0x22, 0x64, 0x0b, 0x33, 0x43, 0xd0, 0x80, 0xd4, 0x44, 0x95, 0x2e, 0x6f,
    0x5e, 0x13, 0x8d, 0x47, 0x62, 0x06, 0xeb, 0x80, 0x82, 0xc9, 0x41, 0xd5, 0x73, 0x8a, 0x30, 0x23,
    0x24, 0xe3, 0x7f, 0xb2, 0xa8, 0x0b, 0xed, 0x38, 0x42, 0x4c, 0xd7, 0xb0, 0xce, 0x98, 0xbd, 0xe1,
    0xd5, 0xe4, 0xc3, 0x1d, 0x15, 0x4a, 0xcf, 0xd1, 0x1f, 0x39, 0x26, 0x18, 0x93, 0xfc, 0x19, 0xb2,
    0x2d, 0xab, 0xf2, 0x6e, 0xa1, 0x9f, 0xaf, 0xd0, 0x8a, 0x2b, 0xa0, 0x56, 0xb0, 0x41, 0x6d, 0x43,
    0xa4, 0x63, 0xf3, 0xaa, 0x7d, 0xaf, 0x35, 0x57, 0xc2, 0x94, 0x4a, 0x65, 0x0b, 0x41, 0xde, 0xb8,
    0xe2, 0x30, 0x12, 0x27, 0x9b, 0x66, 0x2b, 0x34, 0x5b, 0xb8, 0x99, 0xe8, 0x28, 0x71, 0xd0, 0x95,
    0x6b, 0x07, 0x4d, 0x3c, 0x7a, 0xb3, 0xe5, 0x29, 0xb3, 0xba, 0x8c, 0xcc, 0x2d, 0xe0, 0xc9, 0xc0,
    0x22, 0xec, 0x4c, 0xde, 0xf8, 0x58, 0x07, 0xfc, 0x19, 0xf2, 0x64, 0xe2, 0xc3, 0xe2, 0xd8, 0xb9,
    0xfd, 0x67, 0xa0, 0xbc, 0xf5, 0x2e, 0xc9, 0x49, 0x75, 0x62, 0x82, 0x27, 0x10, 0xf4, 0x19, 0x6f,
    0x49, 0xf7, 0xb3, 0x84, 0x14, 0xea, 0xeb, 0xe1, 0x2a, 0x31, 0xab, 0x47, 0x7d, 0x08, 0x29, 0xac,
    0xbb, 0x72, 0xfa, 0xfa, 0x62, 0xb8, 0xc8, 0xd3, 0x86, 0x89, 0x95, 0xfd, 0xdf, 0xcc, 0x9c, 0xad,
    0xf1, 0xd4, 0x6c, 0x64, 0x23, 0x24, 0x2a, 0x56, 0x1f, 0x36, 0xeb, 0xb7, 0xd6, 0xff, 0xda, 0x57,
    0xf4, 0x50, 0x79, 0x08, 0x00,
];

fn little(bits: &mut Vec<bool>, value: usize, count: usize) {
    bits.extend((0..count).map(|i| value & (1 << i) != 0));
}

fn crc(format: u8, data: &[u8]) -> u16 {
    // Annex D: format ID as a two-byte little-endian prefix, CCITT polynomial.
    let mut crc = 0u16;
    for byte in [format, 0].iter().chain(data) {
        for i in 0..8 {
            let feedback = (crc ^ (u16::from(*byte) >> i)) & 1;
            crc >>= 1;
            if feedback != 0 {
                crc ^= 0x8408;
            }
        }
    }
    crc
}

fn raw(data: &[u8], format: usize) -> Result<Vec<bool>, String> {
    if data.len() > 511 {
        return Err("legacy Data Matrix data exceeds the nine-bit length field".into());
    }
    let mut bits = Vec::new();
    // Figure 3: format ID MSB first, CRC and character count LSB first.
    bits.extend((0..5).rev().map(|i| (format - 1) & (1 << i) != 0));
    little(&mut bits, crc(format as u8, data) as usize, 16);
    little(&mut bits, data.len(), 9);
    let alphabet: &[u8] = match format {
        1 => b" 0123456789",
        2 => b" ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        3 => b" ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.,-/",
        4 => b" ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
        _ => b"",
    };
    if format >= 5 {
        let width = if format == 5 { 7 } else { 8 };
        for &byte in data {
            if format == 5 && byte >= 128 {
                return Err("legacy Data Matrix character outside format alphabet".into());
            }
            little(&mut bits, byte as usize, width);
        }
    } else {
        let chunk = match format {
            1 => 6,
            2 => 5,
            _ => 4,
        };
        for group in data.chunks(chunk) {
            let mut value = 0usize;
            let mut power = 1usize;
            for byte in group {
                let digit = alphabet
                    .iter()
                    .position(|v| v == byte)
                    .ok_or("legacy Data Matrix character outside format alphabet")?;
                value += digit * power;
                power *= alphabet.len();
            }
            let width = (usize::BITS - (power - 1).leading_zeros()) as usize;
            little(&mut bits, value, width);
        }
    }
    Ok(bits)
}

fn protect(raw: &[bool], ecc: usize) -> Vec<bool> {
    // Annex E circuit taps: bit zero is the current input; each branch occupies
    // `depth` bits. ECC 080's first branch has one fewer delay than the second.
    let (header, k, depth, taps): (usize, usize, usize, &[u32]) = match ecc {
        50 => (0x0e00e, 3, 4, &[0xe81, 0x0bc, 0x32e, 0xb73]),
        80 => (0x71c0e, 2, 12, &[0x9884eb, 0x349732, 0xa970e1]),
        100 => (0x7fc0e, 1, 16, &[0x87e5, 0xe85b]),
        140 => (0x7e38e, 1, 14, &[0x3491, 0x2f99, 0x3ab7, 0x3eb7]),
        _ => {
            let mut bits = Vec::new();
            little(&mut bits, 0x7e, 7);
            bits.extend_from_slice(raw);
            return bits;
        }
    };
    let mut bits = Vec::new();
    little(&mut bits, header, 19);
    let mut registers = vec![0u32; k];
    for cycle in 0..raw.len().div_ceil(k) + depth - 1 {
        let mut state = 0;
        for (branch, register) in registers.iter_mut().enumerate() {
            *register = ((*register << 1)
                | u32::from(raw.get(cycle * k + branch).copied().unwrap_or(false)))
                & ((1 << depth) - 1);
            state |= *register << (branch * depth);
        }
        bits.extend(taps.iter().map(|tap| (state & tap).count_ones() % 2 != 0));
    }
    bits
}

fn positions(n: usize) -> Vec<(usize, usize)> {
    // Compact construction of the Annex B permutations: bit-reversal order,
    // a two-column skew per row, then exchanges for the four corner bits.
    let mut order: Vec<_> = (0..n).collect();
    order.sort_by_key(|v| v.reverse_bits());
    let mut inverse = vec![0; n];
    for (i, &v) in order.iter().enumerate() {
        inverse[v] = i;
    }
    let mut positions = Vec::with_capacity(n * n);
    for i in 0..n * n {
        let y = order[i % n];
        positions.push(((inverse[i / n] + 2 * y) % n, n - 1 - y));
    }
    for (i, corner) in [(0, n - 1), (n - 1, 0), (0, 0), (n - 1, n - 1)]
        .iter()
        .enumerate()
    {
        let other = positions.iter().position(|p| p == corner).unwrap();
        positions.swap(i, other);
    }
    positions
}

pub(super) fn render(b: &Barcode, data: &[u8], ecc: usize) -> Result<Path, String> {
    let format = b.integer(5, 6, 1, 6)?;
    let data = pdf417::field_escapes(data);
    let bits = protect(&raw(&data, format)?, ecc);
    let mut forced = 0;
    for i in [3, 4] {
        let value = b.integer(i, 0, 0, 32000)?;
        if value == 0 || value > 49 {
            continue;
        }
        if value % 2 == 0 {
            return Err("legacy Data Matrix dimensions must be odd".into());
        }
        if value < 9 {
            return Err("legacy Data Matrix data does not fit requested dimensions".into());
        }
        forced = forced.max(value);
    }
    let n = (7usize..=47)
        .step_by(2)
        .find(|n| n * n >= bits.len() && (forced == 0 || n + 2 == forced))
        .ok_or("legacy Data Matrix data does not fit requested dimensions")?;
    let mut matrix = Matrix::new(n + 2, n + 2);
    for i in 0..n + 2 {
        matrix.set(0, i, true);
        matrix.set(i, n + 1, true);
        matrix.set(i, 0, i % 2 == 0);
        matrix.set(n + 1, i, i % 2 == 0);
    }
    for (i, (x, y)) in positions(n).into_iter().enumerate() {
        let whiten = RANDOM[i / 8] & (1 << (7 - i % 8)) != 0;
        matrix.set(x + 1, y + 1, bits.get(i).copied().unwrap_or(false) ^ whiten);
    }
    let module = b.num(1, 0., 0., 32000.)?;
    let module = if module == 0. {
        (b.height / (n + 2) as f64).round().max(1.)
    } else {
        module
    };
    b.matrix(&matrix, module, module)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn annex_b_seven_by_seven() {
        let expected = [
            2, 45, 10, 38, 24, 21, 1, 12, 40, 26, 5, 33, 19, 47, 22, 31, 29, 15, 43, 8, 36, 34, 20,
            48, 13, 41, 27, 6, 44, 9, 37, 23, 17, 30, 16, 39, 25, 4, 32, 18, 46, 11, 0, 28, 14, 42,
            7, 35, 3,
        ];
        let mut actual = [0; 49];
        for (i, (x, y)) in positions(7).into_iter().enumerate() {
            actual[y * 7 + x] = i;
        }
        assert_eq!(actual, expected);
    }
    #[test]
    fn crc_and_format_alphabets() {
        assert_eq!(crc(6, b"ABC123"), 0xa5e3);
        assert!(raw(b"abc", 2).is_err());
        assert!(raw(&[128], 5).is_err());
        assert!(raw(&[255], 6).is_ok());
        assert!(raw(&vec![b'0'; 512], 1).is_err());
    }
}
