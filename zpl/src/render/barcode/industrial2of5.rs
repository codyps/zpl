//! Industrial 2 of 5; Zebra programming guide, printed page 114.
//! Specification links and implementation limits: docs/barcodes.md.
use super::*;
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    two_of_five::encode(b, data, 1, false)
}
