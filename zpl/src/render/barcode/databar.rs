//! Original GS1 DataBar width enumeration, ISO/IEC 24724 sections 5.2.1–5.2.4.
//! <https://www.iso.org/standard/51426.html>
use super::*;
// Enumerate bounded compositions lexicographically. This deliberately uses
// direct enumeration, not the reference standard's combinatorial C algorithm.
fn widths(sum: usize, max: usize, narrow: bool) -> Vec<[usize; 4]> {
    let mut out = Vec::new();
    for a in 1..=max {
        for b in 1..=max {
            for c in 1..=max {
                if a + b + c >= sum {
                    continue;
                }
                let d = sum - a - b - c;
                if d <= max && (!narrow || [a, b, c, d].contains(&1)) {
                    out.push([a, b, c, d]);
                }
            }
        }
    }
    out
}
fn character(value: usize, outside: bool) -> [usize; 8] {
    const OUT: [(usize, usize, usize, usize, usize); 5] = [
        (0, 12, 8, 1, 1),
        (161, 10, 6, 3, 10),
        (961, 8, 4, 5, 34),
        (2015, 6, 3, 6, 70),
        (2715, 4, 1, 8, 126),
    ];
    const IN: [(usize, usize, usize, usize, usize); 4] = [
        (0, 5, 2, 7, 4),
        (336, 7, 4, 5, 20),
        (1036, 9, 6, 3, 48),
        (1516, 11, 8, 1, 81),
    ];
    let table = if outside {
        OUT.as_slice()
    } else {
        IN.as_slice()
    };
    let &(base, odd_sum, odd_max, even_max, div) =
        table.iter().rev().find(|t| value >= t.0).unwrap();
    let v = value - base;
    let odd = widths(odd_sum, odd_max, !outside);
    let even = widths(if outside { 16 } else { 15 } - odd_sum, even_max, outside);
    let a = odd[if outside { v / div } else { v % div }];
    let b = even[if outside { v % div } else { v / div }];
    std::array::from_fn(|i| if i % 2 == 0 { a[i / 2] } else { b[i / 2] })
}
pub(super) fn gtin(data: &[u8]) -> Result<u64, String> {
    let data = data.strip_prefix(b"(01)").unwrap_or(data);
    let mut d = digits(data)?;
    if d.len() == 14 {
        if mod10(&d[..13]) != d[13] {
            return Err("invalid DataBar GTIN check digit".into());
        }
        d.pop();
    }
    if d.len() != 13 {
        return Err("DataBar requires 13 digits or a checked GTIN-14".into());
    }
    Ok(d.iter().fold(0u64, |n, &v| n * 10 + v as u64))
}
pub(super) fn encode(data: &[u8]) -> Result<Vec<bool>, String> {
    let number = gtin(data)?;
    let left = (number / 4537077) as usize;
    let right = (number % 4537077) as usize;
    let chars = [
        character(left / 1597, true),
        character(left % 1597, false),
        character(right / 1597, true),
        character(right % 1597, false),
    ];
    let mut sum = 0;
    let mut weight = 1;
    for w in chars.iter().flatten() {
        sum = (sum + w * weight) % 79;
        weight = weight * 3 % 79;
    }
    if sum >= 8 {
        sum += 1;
    }
    if sum >= 72 {
        sum += 1;
    }
    const FIND: [[usize; 5]; 9] = [
        [3, 8, 2, 1, 1],
        [3, 5, 5, 1, 1],
        [3, 3, 7, 1, 1],
        [3, 1, 9, 1, 1],
        [2, 7, 4, 1, 1],
        [2, 5, 6, 1, 1],
        [2, 3, 8, 1, 1],
        [1, 5, 7, 1, 1],
        [1, 3, 9, 1, 1],
    ];
    let mut elements = vec![1, 1];
    elements.extend(chars[0]);
    elements.extend(FIND[sum / 9]);
    elements.extend(chars[1].iter().rev());
    elements.extend(chars[3]);
    elements.extend(FIND[sum % 9].iter().rev());
    elements.extend(chars[2].iter().rev());
    elements.extend([1, 1]);
    let mut result = Vec::new();
    runs(&mut result, &elements);
    for bit in &mut result {
        *bit = !*bit;
    }
    Ok(result)
}
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    let variant = b.integer(1, 1, 1, 12)?;
    let module = b.num(2, b.scale(), 1., 10.)?;
    let separator = b.integer(3, 1, 1, 2)?;
    let mut linear = b.clone();
    linear.module = module;
    linear.height = b.num(4, 25., 1., 32000.)?;
    match variant {
        7..=10 => {
            linear.params = vec![
                "N".into(),
                linear.height.to_string(),
                "N".into(),
                "N".into(),
            ];
            return match variant {
                7 => upca::render(&linear, data),
                8 => upce::render(&linear, data),
                9 => ean13::render(&linear, data),
                _ => ean8::render(&linear, data),
            };
        }
        3 | 4 => return databar_stacked::render(&linear, data, variant == 4, separator),
        5 => return databar_limited::render(&linear, data),
        6 => return databar_expanded::render(&linear, data, separator),
        11 | 12 => return composite::render(&linear, data, variant == 12, separator),
        1 | 2 => {}
        _ => unreachable!(),
    }
    linear.height = module * if variant == 1 { 33. } else { 13. };
    linear.linear(&encode(data)?, None)
}
