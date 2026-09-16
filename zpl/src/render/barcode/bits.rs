//! Most-significant-bit-first message construction.
pub(super) fn push(out: &mut Vec<bool>, value: usize, n: usize) {
    out.extend((0..n).rev().map(|i| value & (1 << i) != 0));
}
pub(super) fn value(bits: &[bool]) -> usize {
    bits.iter().fold(0, |v, &b| (v << 1) | usize::from(b))
}
