use super::*;

/// Zebra ^B8/^B9/^BE/^BU interpretation, measured on ZD621 203 DPI.
/// ^BU pp. 142–143 specifies resident A below module 3, OCR-B thereafter;
/// font E's native 28x15 cell/20-dot advance is in Table 29, p. 1582.
/// Raw controls and the independently sampled OCR-B strike are preserved in
/// tests/fixtures/retail-caption-zd621-v1 (see its README for source links).
pub(super) fn caption(b: &Barcode, bytes: &[u8], rotation: u8) -> Result<Path, String> {
    let m = b.module;
    let (id, fw, fh) = if m < 3. {
        ('A', 5. * m, 9. * m)
    } else {
        let scale = (m / 3.).floor();
        ('E', 15. * scale, 28. * scale)
    };
    let advance = super::super::font::width_for(id, "0", fw, fh)?;
    let digits = match b.name.as_str() {
        "B8" => checked(bytes, 8)?,
        "B9" => upce::canonical(bytes)?,
        "BE" => checked(bytes, 13)?,
        _ => checked(bytes, 12)?,
    };
    let half = (m / 2.).floor();
    let mut path = Path::default();
    let mut group = |digits: &[u8], x: f64| -> Result<(), String> {
        let value: String = digits.iter().map(|v| (v + b'0') as char).collect();
        let mut text = super::super::font::text_for(id, &value, fw, fh)?;
        let reverse =
            b.compatibility.barcode_reverse_interpretation_shift && matches!(rotation, b'I' | b'B');
        text.transform(|p| {
            Point::new(p.x + x - if reverse { 1. } else { 0. }, p.y + b.height + 4.)
        });
        path.segments.extend(text.segments);
        Ok(())
    };
    match b.name.as_str() {
        "B8" => {
            group(&digits[..4], 31. * m - 4. * advance + half)?;
            group(&digits[4..], 36. * m + half)?;
        }
        "B9" => {
            group(&digits[..1], -9. * m)?;
            group(&digits[1..7], ((51. * m - 6. * advance) / 2.).floor())?;
            if b.flag(4, true)? {
                group(&digits[7..], 53. * m)?;
            }
        }
        "BE" => {
            group(&digits[..1], -9. * m)?;
            group(&digits[1..7], 45. * m - 6. * advance + half)?;
            group(&digits[7..], 50. * m + half)?;
        }
        _ => {
            group(&digits[..1], -9. * m)?;
            group(&digits[1..6], 45. * m - 5. * advance + half)?;
            group(&digits[6..11], 50. * m + half)?;
            if b.flag(4, true)? {
                group(&digits[11..], 97. * m)?;
            }
        }
    }
    Ok(path)
}

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
