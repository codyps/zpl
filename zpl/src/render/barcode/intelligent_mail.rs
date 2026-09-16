//! USPS-B-3200 Rev H, sections 2.2.1–2.2.6 and Appendix E, Table 22.
//! <https://postalpro.usps.com/node/2190>
use super::*;

// Normative bit-to-bar assignment, not an encoder implementation.
const BAR_BITS: [(usize, u32, usize, u32); 65] = [
    (7, 2, 4, 3),
    (1, 10, 0, 0),
    (9, 12, 2, 8),
    (5, 5, 6, 11),
    (8, 9, 3, 1),
    (0, 1, 5, 12),
    (2, 5, 1, 8),
    (4, 4, 9, 11),
    (6, 3, 8, 10),
    (3, 9, 7, 6),
    (5, 11, 1, 4),
    (8, 5, 2, 12),
    (9, 10, 0, 2),
    (7, 1, 6, 7),
    (3, 6, 4, 9),
    (0, 3, 8, 6),
    (6, 4, 2, 7),
    (1, 1, 9, 9),
    (7, 10, 5, 2),
    (4, 0, 3, 8),
    (6, 2, 0, 4),
    (8, 11, 1, 0),
    (9, 8, 3, 12),
    (2, 6, 7, 7),
    (5, 1, 4, 10),
    (1, 12, 6, 9),
    (7, 3, 8, 0),
    (5, 8, 9, 7),
    (4, 6, 2, 10),
    (3, 4, 0, 5),
    (8, 4, 5, 7),
    (7, 11, 1, 9),
    (6, 0, 9, 6),
    (0, 6, 4, 8),
    (2, 1, 3, 2),
    (5, 9, 8, 12),
    (4, 11, 6, 1),
    (9, 5, 7, 4),
    (3, 3, 1, 2),
    (0, 7, 2, 0),
    (1, 3, 4, 1),
    (6, 10, 3, 5),
    (8, 7, 9, 4),
    (2, 11, 5, 6),
    (0, 8, 7, 12),
    (4, 2, 8, 1),
    (5, 10, 3, 0),
    (9, 3, 0, 9),
    (6, 5, 2, 4),
    (7, 8, 1, 7),
    (5, 0, 4, 5),
    (2, 3, 0, 10),
    (6, 12, 9, 2),
    (3, 11, 1, 6),
    (8, 8, 7, 9),
    (5, 4, 0, 11),
    (1, 5, 2, 2),
    (9, 1, 4, 12),
    (8, 3, 6, 6),
    (7, 0, 3, 7),
    (4, 7, 7, 5),
    (0, 12, 1, 11),
    (2, 9, 9, 0),
    (6, 8, 5, 3),
    (3, 10, 8, 2),
];

fn alphabet(weight: u32) -> Vec<u16> {
    let mut pairs = Vec::new();
    let mut symmetric = Vec::new();
    for value in (0u16..8192).filter(|v| v.count_ones() == weight) {
        let reverse = value.reverse_bits() >> 3;
        match value.cmp(&reverse) {
            std::cmp::Ordering::Less => pairs.extend([value, reverse]),
            std::cmp::Ordering::Equal => symmetric.push(value),
            _ => {}
        }
    }
    pairs.extend(symmetric.into_iter().rev());
    pairs
}
fn states(data: &[u8]) -> Result<Vec<(bool, bool)>, String> {
    let d = digits(data)?;
    if !matches!(d.len(), 20 | 25 | 29 | 31) || d[1] > 4 {
        return Err("Intelligent Mail requires 20 tracking digits (second digit 0–4) and 0, 5, 9, or 11 routing digits".into());
    }
    let route = d[20..].iter().fold(0u128, |n, &v| 10 * n + v as u128);
    let mut value = route
        + match d.len() {
            20 => 0,
            25 => 1,
            29 => 100001,
            _ => 1000100001,
        };
    for (i, &digit) in d[..20].iter().enumerate() {
        value = value * if i == 1 { 5 } else { 10 } + digit as u128;
    }
    let mut crc = 0x7ffu16;
    for bit in (0..102).rev() {
        let feedback = (crc >> 10) ^ ((value >> bit) as u16 & 1);
        crc = (crc << 1) & 0x7ff;
        if feedback != 0 {
            crc ^= 0x735;
        }
    }
    let mut words = [0usize; 10];
    words[9] = (value % 636) as usize * 2;
    value /= 636;
    for i in (1..9).rev() {
        words[i] = (value % 1365) as usize;
        value /= 1365;
    }
    words[0] = value as usize + if crc & 0x400 != 0 { 659 } else { 0 };
    let mut table = alphabet(5);
    table.extend(alphabet(2));
    let characters: Vec<_> = words
        .iter()
        .enumerate()
        .map(|(i, &w)| table[w] ^ if crc & (1 << i) != 0 { 8191 } else { 0 })
        .collect();
    Ok(BAR_BITS
        .iter()
        .map(|&(d, db, a, ab)| {
            (
                characters[a] & (1 << ab) != 0,
                characters[d] & (1 << db) != 0,
            )
        })
        .collect())
}
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    let mut path = Path::default();
    for (i, (ascend, descend)) in states(data)?.into_iter().enumerate() {
        let top = if ascend { 0. } else { b.height / 3. };
        let bottom = if descend {
            b.height
        } else {
            b.height * 2. / 3.
        };
        path.rect(
            i as f64 * b.module * (1. + b.ratio),
            top,
            b.module,
            bottom - top,
        );
    }
    Ok(path)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn usps_published_vector() {
        let actual: String = states(b"0027012345620080000198765432101")
            .unwrap()
            .into_iter()
            .map(|v| match v {
                (false, false) => 'T',
                (true, false) => 'A',
                (false, true) => 'D',
                (true, true) => 'F',
            })
            .collect();
        assert_eq!(
            actual,
            "TTFAFDADTFFFADTAFAFTTDATDFAAFTDAFDFDFDATFDFTDDDDFADFFDADDTDDTTDAT"
        );
    }
}
