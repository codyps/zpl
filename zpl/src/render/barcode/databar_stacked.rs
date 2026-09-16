//! Original DataBar Stacked/Stacked Omnidirectional, ISO/IEC 24724:2011 §5.3.
//! <https://www.iso.org/standard/51426.html>
use super::*;

pub(super) fn render(
    b: &Barcode,
    data: &[u8],
    omni: bool,
    separator: usize,
) -> Result<Path, String> {
    let bits = databar::encode(data)?;
    let mut top = bits[..48].to_vec();
    top.extend([true, false]);
    let mut bottom = vec![true, false];
    bottom.extend_from_slice(&bits[48..]);
    let (upper, lower) = if omni { (33, 33) } else { (5, 7) };
    let mut rows = vec![top.clone(); upper];
    if omni {
        let mut a: Vec<_> = top.iter().map(|v| !v).collect();
        let mut c: Vec<_> = bottom.iter().map(|v| !v).collect();
        alternate(&top, &mut a, 18..31);
        // Finder element numbering is right-to-left, but separator alternation
        // over its spaces is left-to-right (§5.3.2.2).
        alternate(&bottom, &mut c, 19..32);
        // Finder 3 has a nine-module bar: move the isolated separator module
        // one position right so it lies over the start of the three-module bar.
        if bottom[17..32]
            == [
                true, false, true, true, true, true, true, true, true, true, true, false, true,
                true, true,
            ]
        {
            c[28] = false;
            c[29] = true;
        }
        let middle: Vec<_> = (0..50).map(|x| x % 2 == 1).collect();
        for mut row in [a, middle, c] {
            row[..4].fill(false);
            row[46..].fill(false);
            rows.extend(vec![row; separator]);
        }
    } else {
        let mut row = vec![false; 50];
        for x in 4..46 {
            row[x] = if top[x] == bottom[x] {
                !top[x]
            } else {
                !row[x - 1]
            };
        }
        rows.extend(vec![row; separator]);
    }
    rows.extend(vec![bottom; lower]);
    let mut matrix = Matrix::new(50, rows.len());
    for (y, row) in rows.iter().enumerate() {
        for (x, &bit) in row.iter().enumerate() {
            matrix.set(x, y, bit);
        }
    }
    b.matrix(&matrix, b.module, b.module)
}

fn alternate(row: &[bool], separator: &mut [bool], positions: impl Iterator<Item = usize>) {
    let mut previous = false;
    for x in positions {
        separator[x] = !row[x] && !previous;
        previous = separator[x];
    }
}
