//! UPC/EAN two- and five-digit supplements; ISO/IEC 15420; Zebra page 137.
//! Specification links and implementation limits: docs/barcodes.md.
use super::*;
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    let d = digits(data)?;
    let parity = match d.len() {
        2 => (d[0] * 10 + d[1]) % 4,
        5 => {
            let c = ((d[0] + d[2] + d[4]) * 3 + (d[1] + d[3]) * 9) % 10;
            [24, 20, 18, 17, 12, 6, 3, 10, 9, 5][c as usize]
        }
        _ => return Err("UPC extension requires 2 or 5 digits".into()),
    };
    let mut out = vec![true, false, true, true];
    for (i, &v) in d.iter().enumerate() {
        if i > 0 {
            out.extend([false, true]);
        }
        retail::digit(&mut out, v, parity & (1 << (d.len() - 1 - i)) != 0);
    }
    b.linear(&out, None)
}
