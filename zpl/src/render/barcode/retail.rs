use super::*;
pub(super) const LEFT: [u32; 10] = [13, 25, 19, 61, 35, 49, 47, 59, 55, 11];
pub(super) fn digit(out: &mut Vec<bool>, d: u8, even: bool) {
    let mut p = LEFT[d as usize];
    if even {
        p = (!p & 127).reverse_bits() >> 25;
    }
    append_pattern(out, p, 7);
}
pub(super) fn checked(data: &[u8], n: usize) -> Result<Vec<u8>, String> {
    let mut v = digits(data)?;
    if v.len() == n - 1 {
        v.push(mod10(&v));
    } else if v.len() != n || mod10(&v[..n - 1]) != v[n - 1] {
        return Err(format!(
            "expected {} data digits or {} digits with a valid check digit",
            n - 1,
            n
        ));
    }
    Ok(v)
}
pub(super) fn encode(data: &[u8], n: usize) -> Result<Vec<bool>, String> {
    let v = checked(data, n)?;
    let mut out = vec![true, false, true];
    if n == 13 {
        const PARITY: [u8; 10] = [0, 11, 13, 14, 19, 25, 28, 21, 22, 26];
        for (i, &d) in v[1..7].iter().enumerate() {
            digit(&mut out, d, PARITY[v[0] as usize] & (1 << (5 - i)) != 0);
        }
    } else {
        for &d in &v[..4] {
            digit(&mut out, d, false);
        }
    }
    append_pattern(&mut out, 0b01010, 5);
    for &d in &v[if n == 13 { 7 } else { 4 }..] {
        append_pattern(&mut out, !LEFT[d as usize] & 127, 7);
    }
    append_pattern(&mut out, 0b101, 3);
    Ok(out)
}
