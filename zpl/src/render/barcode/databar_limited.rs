//! Original DataBar Limited, ISO/IEC 24724:2011 §6 and normative Annex C.
//! <https://www.iso.org/standard/51426.html>
use super::*;

// Direct lexicographic search of bounded compositions, not reference source.
pub(super) fn compositions<const N: usize>(
    sum: usize,
    max: usize,
    narrow: bool,
) -> Vec<[usize; N]> {
    fn visit<const N: usize>(
        at: usize,
        remaining: usize,
        max: usize,
        narrow: bool,
        value: &mut [usize; N],
        out: &mut Vec<[usize; N]>,
    ) {
        if at == N {
            if remaining == 0 && (!narrow || value.contains(&1)) {
                out.push(*value);
            }
            return;
        }
        for width in 1..=max.min(remaining) {
            let rest = remaining - width;
            if rest < N - at - 1 || rest > (N - at - 1) * max {
                continue;
            }
            value[at] = width;
            visit(at + 1, rest, max, narrow, value, out);
        }
    }
    let mut result = Vec::new();
    visit(0, sum, max, narrow, &mut [0; N], &mut result);
    result
}

fn character(value: usize) -> [usize; 14] {
    const GROUPS: [(usize, usize, usize, usize, usize); 7] = [
        (0, 17, 6, 3, 28),
        (183064, 13, 5, 4, 728),
        (820064, 9, 3, 6, 6454),
        (1000776, 15, 5, 4, 203),
        (1491021, 11, 4, 5, 2408),
        (1979845, 19, 8, 1, 1),
        (1996939, 7, 1, 8, 16632),
    ];
    let &(base, sum, odd_max, even_max, count) =
        GROUPS.iter().rev().find(|g| value >= g.0).unwrap();
    let odd = compositions::<7>(sum, odd_max, false)[(value - base) / count];
    let even = compositions::<7>(26 - sum, even_max, true)[(value - base) % count];
    std::array::from_fn(|i| if i % 2 == 0 { odd[i / 2] } else { even[i / 2] })
}

pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    let value = databar::gtin(data)?;
    if value > 1_999_999_999_999 {
        return Err("DataBar Limited GTIN must begin with 0 or 1".into());
    }
    let left = character((value / 2013571) as usize);
    let right = character((value % 2013571) as usize);
    let mut check = 0;
    let mut weight = 1;
    for width in left.iter().chain(&right) {
        check = (check + width * weight) % 89;
        weight = weight * 3 % 89;
    }
    // Normative Annex C sequence numbers, describing 6-space/6-bar patterns.
    const SEQUENCES: [usize; 89] = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 45, 52, 57, 63,
        64, 65, 66, 73, 74, 75, 76, 77, 78, 79, 82, 126, 127, 128, 129, 130, 132, 141, 142, 143,
        144, 145, 146, 210, 211, 212, 213, 214, 215, 216, 217, 220, 316, 317, 318, 319, 320, 322,
        323, 326, 337,
    ];
    let patterns = compositions::<6>(8, 3, false);
    let sequence = SEQUENCES[check];
    let mut elements = vec![1, 1];
    elements.extend(left);
    for (&space, &bar) in patterns[sequence / 21].iter().zip(&patterns[sequence % 21]) {
        elements.extend([space, bar]);
    }
    elements.extend([1, 1]);
    elements.extend(right);
    elements.extend([1, 1, 5]);
    let mut bits = Vec::new();
    runs(&mut bits, &elements);
    for bit in &mut bits {
        *bit = !*bit;
    }
    let mut linear = b.clone();
    linear.height = 10. * b.module;
    linear.linear(&bits, None)
}
