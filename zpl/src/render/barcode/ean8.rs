//! EAN-8; ISO/IEC 15420; Zebra page 83.
//! Specification links and implementation limits: docs/barcodes.md.
use super::*;
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    b.linear(&retail::encode(data, 8)?, None)
}
