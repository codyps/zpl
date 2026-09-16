//! Original CC-C: ISO/IEC 24723:2010 §10, with standard PDF417 ECC/framing.
//! <https://www.iso.org/standard/51425.html>
use super::*;

#[derive(Clone, Copy)]
pub(super) struct Size {
    pub cols: usize,
    pub rows: usize,
    pub ec: usize,
    pub bits: usize,
}

pub(super) fn size(length: usize, linear_width: usize) -> Option<Size> {
    // §12.3(f) places the CC-C start 7X left of the linear start. Its right
    // edge plus 2X quiet zone may extend through the linear 10X quiet zone.
    let cols = ((linear_width + 15).saturating_sub(69) / 17).clamp(1, 30);
    for rows in 3..=90 {
        if cols + 4 >= 4 * rows || rows * cols > 928 {
            continue;
        }
        for ec in 2..=5 {
            let Some(data) = (rows * cols).checked_sub(1 << (ec + 1)) else {
                continue;
            };
            if !(3..=863).contains(&data) {
                continue;
            }
            // ISO/IEC 15438 recommended ECC levels for the total data region.
            let minimum = if data <= 40 {
                2
            } else if data <= 160 {
                3
            } else if data <= 320 {
                4
            } else {
                5
            };
            if ec < minimum {
                continue;
            }
            let bits = 8 * composite_b::byte_capacity(data - 3);
            if bits >= length {
                return Some(Size {
                    cols,
                    rows,
                    ec,
                    bits,
                });
            }
        }
    }
    None
}
pub(super) fn encode(bits: &[bool], size: Size) -> Matrix {
    let capacity = size.rows * size.cols - (1 << (size.ec + 1));
    let mut words = vec![capacity, 920];
    words.extend(pdf417::compact(&composite_b::bytes(bits)));
    debug_assert_eq!(words.len(), capacity);
    words.extend(pdf417::parity(&words, 1 << (size.ec + 1)));
    pdf417::matrix(&words, size.cols, size.rows, size.ec, false)
}
