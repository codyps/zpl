//! Interleaved 2 of 5; ISO/IEC 16390:2007; Zebra page 68.
//! Specification links and implementation limits: docs/barcodes.md.
use super::*;
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    b.require(5, "N", &["N"])?;
    two_of_five::encode(b, data, 0, b.flag(4, false)?)
}
