//! MSI; Zebra programming guide, printed page 122.
//! Specification links and implementation limits: docs/barcodes.md.
use super::*;
pub(super) fn checks(data: &[u8], mode: &str) -> Result<Vec<u8>, String> {
    let mut v = digits(data)?;
    fn luhn(v: &[u8]) -> u8 {
        let sum: usize = v
            .iter()
            .rev()
            .enumerate()
            .map(|(i, &d)| {
                let n = d as usize * if i % 2 == 0 { 2 } else { 1 };
                n / 10 + n % 10
            })
            .sum();
        ((10 - sum % 10) % 10) as u8
    }
    match mode {
        "A" => {}
        "B" => {
            v.push(luhn(&v));
        }
        "C" => {
            v.push(luhn(&v));
            v.push(luhn(&v));
        }
        "D" => {
            let n = 11
                - v.iter()
                    .rev()
                    .enumerate()
                    .map(|(i, &d)| (i % 6 + 2) * d as usize)
                    .sum::<usize>()
                    % 11;
            if n == 10 {
                v.extend([1, 0]);
            } else {
                v.push((n % 11) as u8);
            }
            v.push(luhn(&v));
        }
        _ => return Err("invalid MSI check mode".into()),
    }
    Ok(v)
}
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    b.flag(5, false)?;
    if data.len() > if b.param(1, "B") == "A" { 13 } else { 14 } {
        return Err("MSI data exceeds ZPL digit limit".into());
    }
    let v = checks(data, b.param(1, "B"))?;
    let mut out = vec![true, true, false];
    for d in v {
        for bit in (0..4).rev() {
            append_pattern(&mut out, if d & (1 << bit) == 0 { 0b100 } else { 0b110 }, 3);
        }
    }
    append_pattern(&mut out, 0b1001, 4);
    b.linear(&out, Some(b.ratio))
}
