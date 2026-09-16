//! Original four-column CC-B: ISO/IEC 24723:2010 §9 and ISO/IEC 24728.
//! <https://www.iso.org/standard/51425.html>
use super::*;
pub(super) fn byte_capacity(words: usize) -> usize {
    (words / 5) * 6 + words % 5
}
fn capacity_for(mode: usize) -> usize {
    8 * byte_capacity(4 * micropdf417::ROWS[mode] - micropdf417::EC[mode] - 2)
}
pub(super) fn capacity(length: usize) -> Option<usize> {
    (23..34).map(capacity_for).find(|&n| n >= length)
}
pub(super) fn bytes(bits: &[bool]) -> Vec<u8> {
    bits.as_chunks::<8>()
        .0
        .iter()
        .map(|b| bits::value(b) as u8)
        .collect()
}
pub(super) fn encode(bits: &[bool]) -> Result<Matrix, String> {
    let mode = (23..34)
        .find(|&m| capacity_for(m) == bits.len())
        .ok_or("invalid CC-B bit capacity")?;
    let mut words = vec![920];
    words.extend(pdf417::compact(&bytes(bits)));
    debug_assert_eq!(
        words.len(),
        4 * micropdf417::ROWS[mode] - micropdf417::EC[mode]
    );
    micropdf417::encode_words(words, mode)
}
