//! UPC-A; ISO/IEC 15420; Zebra page 142.
//! Specification links and implementation limits: docs/barcodes.md.
use super::*;
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    b.flag(4, true)?;
    let v = retail::checked(data, 12)?;
    let mut d = vec![b'0'];
    d.extend(v.iter().map(|v| v + b'0'));
    retail::render(b, &retail::encode(&d, 13)?, true)
}
