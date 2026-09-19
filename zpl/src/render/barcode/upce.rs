//! UPC-E; ISO/IEC 15420; Zebra page 85.
//! Specification links and implementation limits: docs/barcodes.md.
use super::*;
/// Inverse of the four UPC-E zero-suppression expansions (ISO/IEC 15420).
pub(super) fn compress(data: &[u8]) -> Result<Vec<u8>, String> {
    let d = digits(data)?;
    if d.len() != 11 || d[0] > 1 {
        return Err("UPC-E requires an 11-digit UPC-A value with number system 0 or 1".into());
    }
    let mut out = vec![d[0], d[1], d[2]];
    if d[3] <= 2 && d[4..8] == [0, 0, 0, 0] {
        out.extend([d[8], d[9], d[10], d[3]]);
    } else if d[3] >= 3 && d[4..9] == [0, 0, 0, 0, 0] {
        out.extend([d[3], d[9], d[10], 3]);
    } else if d[4] != 0 && d[5..10] == [0, 0, 0, 0, 0] {
        out.extend([d[3], d[4], d[10], 4]);
    } else if d[5] != 0 && d[6..10] == [0, 0, 0, 0] && d[10] >= 5 {
        out.extend([d[3], d[4], d[5], d[10]]);
    } else {
        return Err("UPC-A value cannot be zero-suppressed to UPC-E".into());
    }
    Ok(out.into_iter().map(|v| b'0' + v).collect())
}
pub(super) fn canonical(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut d = digits(data)?;
    if d.len() == 6 {
        d.insert(0, 0);
    }
    if !matches!(d.len(), 7 | 8) || d[0] > 1 {
        return Err("UPC-E requires 6 digits or 7/8 digits with number system 0 or 1".into());
    }
    let (a, x, y, z, t, u, v) = (d[0], d[1], d[2], d[3], d[4], d[5], d[6]);
    let full = match v {
        0..=2 => vec![a, x, y, v, 0, 0, 0, 0, z, t, u],
        3 => vec![a, x, y, z, 0, 0, 0, 0, 0, t, u],
        4 => vec![a, x, y, z, t, 0, 0, 0, 0, 0, u],
        _ => vec![a, x, y, z, t, u, 0, 0, 0, 0, v],
    };
    let check = mod10(&full);
    if d.len() == 8 && d[7] != check {
        return Err("UPC-E checksum mismatch".into());
    }
    d.truncate(7);
    d.push(check);
    Ok(d)
}
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    b.flag(4, true)?;
    let d = canonical(data)?;
    const PARITY: [u8; 10] = [56, 52, 50, 49, 44, 38, 35, 42, 41, 37];
    let parity = PARITY[d[7] as usize] ^ if d[0] == 1 { 63 } else { 0 };
    let mut out = vec![true, false, true];
    for (i, &digit) in d[1..7].iter().enumerate() {
        retail::digit(&mut out, digit, parity & (1 << (5 - i)) != 0);
    }
    append_pattern(&mut out, 0b010101, 6);
    retail::render(b, &out, false)
}
