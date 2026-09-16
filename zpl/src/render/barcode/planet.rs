//! PLANET; Zebra programming guide, printed pages 78 and 150.
//! Specification links and implementation limits: docs/barcodes.md.
use super::*;

pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    if !matches!(data.len(), 11 | 13) {
        return Err("PLANET requires 11 or 13 digits".into());
    }
    postal::draw(b, data, true)
}
