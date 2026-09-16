//! LOGMARS; Zebra programming guide, printed page 120.
//! Specification links and implementation limits: docs/barcodes.md.
use super::*;
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    const CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%";
    let mut sum = 0;
    for c in data {
        sum += CHARS
            .iter()
            .position(|v| v == c)
            .ok_or("invalid LOGMARS character")?;
    }
    let mut v = data.to_vec();
    v.push(CHARS[sum % 43]);
    code39::render(ascii(&v)?, b.module, b.ratio, b.height)
}
