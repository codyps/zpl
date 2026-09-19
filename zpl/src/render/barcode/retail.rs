use super::*;

/// UPC/EAN guard extensions. ZD621 203-DPI captures extend 13 dots below
/// the nominal bar height, even when the interpretation line is disabled.
/// Module-width 1/2/3 and font-height 10/20/40 controls: docs/printer-accuracy.md.
/// The guard locations follow ISO/IEC 15420 (see docs/barcodes.md).
pub(super) fn render(b: &Barcode, bits: &[bool], upca: bool) -> Result<Path, String> {
    let mut path = b.linear(bits, None)?;
    // ^BR also reuses the encoders below, but has its own component layout.
    // The standalone ^B8/^B9/^BE/^BU controls do not establish that layout.
    if !matches!(b.name.as_str(), "B8" | "B9" | "BE" | "BU") {
        return Ok(path);
    }
    let middle = bits.len() / 2;
    // ISO/IEC 15420:2009, 4.3.3 specifies a five-module extension.
    // https://www.iso.org/standard/46143.html
    let extension = b
        .compatibility
        .retail_guard_extension_dots
        .map(f64::from)
        .unwrap_or(5. * b.module);
    for (i, &black) in bits.iter().enumerate() {
        let guard = i < 3
            || i >= bits.len() - 3
            || (bits.len() != 51 && i.abs_diff(middle) <= 2)
            || (bits.len() == 51 && i >= 45)
            || (upca && (i < 10 || i >= bits.len() - 10));
        if black && guard {
            path.rect(i as f64 * b.module, b.height, b.module, extension);
        }
    }
    Ok(path)
}
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
    } else if v.len() < n - 1 {
        return Err("retail barcode data too short".into());
    } else if v.len() > n {
        return Err("retail barcode data too long".into());
    } else if mod10(&v[..n - 1]) != v[n - 1] {
        return Err("retail barcode check digit mismatch".into());
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
