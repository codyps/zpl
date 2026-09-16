//! ANSI Codabar; Zebra programming guide, printed page 118.
//! Specification links and implementation limits: docs/barcodes.md.
use super::*;
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    b.require(1, "N", &["N"])?;
    let start = b.param(5, "A");
    let end = b.param(6, "A");
    if start.len() != 1
        || end.len() != 1
        || !b"ABCD".contains(&start.as_bytes()[0])
        || !b"ABCD".contains(&end.as_bytes()[0])
    {
        return Err("invalid Codabar start/stop character".into());
    }
    const ALPHABET: &[u8] = b"0123456789-$:/.+ABCD";
    const MASK: [u8; 20] = [
        3, 6, 9, 96, 18, 66, 33, 36, 48, 72, 12, 24, 69, 81, 84, 21, 26, 41, 11, 14,
    ];
    let mut out = Vec::new();
    for (i, c) in start
        .bytes()
        .chain(data.iter().copied())
        .chain(end.bytes())
        .enumerate()
    {
        let index = ALPHABET
            .iter()
            .position(|&v| v == c)
            .ok_or("invalid Codabar character")?;
        if i > 0 && i <= data.len() && index >= 16 {
            return Err("Codabar delimiters must not appear in data".into());
        }
        if i > 0 {
            out.push(false);
        }
        let widths: Vec<_> = (0..7)
            .rev()
            .map(|bit| if MASK[index] & (1 << bit) == 0 { 1 } else { 3 })
            .collect();
        runs(&mut out, &widths);
    }
    b.linear(&out, Some(b.ratio))
}
