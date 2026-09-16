//! POSTNET; Zebra programming guide, printed page 150.
//! Specification links and implementation limits: docs/barcodes.md.
use super::*;

pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    if !matches!(data.len(), 5 | 9 | 11) {
        return Err("POSTNET requires 5, 9, or 11 digits".into());
    }
    postal::draw(b, data, false)
}
