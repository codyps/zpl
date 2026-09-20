//! Code 11; Zebra programming guide, printed page 66.
//! Specification links and implementation limits: docs/barcodes.md.
//! <https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf>
use super::*;
pub(super) fn checked_values(b: &Barcode, data: &[u8]) -> Result<Vec<u8>, String> {
    let mut values = Vec::new();
    for &v in data {
        values.push(match v {
            b'0'..=b'9' => v - b'0',
            b'-' => 10,
            _ => return Err("invalid Code 11 character".into()),
        });
    }
    let check = |v: &[u8], cycle: usize| -> u8 {
        (v.iter()
            .rev()
            .enumerate()
            .map(|(i, &v)| (i % cycle + 1) * v as usize)
            .sum::<usize>()
            % 11) as u8
    };
    values.push(check(&values, 10));
    if !b.flag(1, false)? {
        values.push(check(&values, 9));
    }
    Ok(values)
}
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    let values = checked_values(b, data)?;
    const WIDTHS: [[usize; 5]; 12] = [
        [1, 1, 1, 1, 2],
        [2, 1, 1, 1, 2],
        [1, 2, 1, 1, 2],
        [2, 2, 1, 1, 1],
        [1, 1, 2, 1, 2],
        [2, 1, 2, 1, 1],
        [1, 2, 2, 1, 1],
        [1, 1, 1, 2, 2],
        [2, 1, 1, 2, 1],
        [2, 1, 1, 1, 1],
        [1, 1, 2, 1, 1],
        [1, 1, 2, 2, 1],
    ];
    let mut path = Path::default();
    let mut x = 0.;
    for v in std::iter::once(11).chain(values).chain(std::iter::once(11)) {
        if x != 0. {
            x += b.module;
        }
        // 0, 9 and dash have one extra-wide element, not an ordinary wide
        // element. Keep nominal and captured quantization policies separate.
        for (i, &width) in WIDTHS[v as usize].iter().enumerate() {
            let width = if width == 1 {
                b.module
            } else if b.compatibility.code11_printer_element_widths {
                // Raw module 1–10 and fractional-ratio ZD621 controls:
                // scale the 3/5-unit wide/extra-wide patterns, then truncate
                // each element independently. Do not derive extra-wide from
                // an already truncated wide element.
                // ^BY ratios are in tenths. Preserve exact boundaries such
                // as module 3, ratio 2.8 => 14, avoiding 5/3 roundoff.
                (b.module * (b.ratio * 10.) / if matches!(v, 0 | 9 | 10) { 6. } else { 10. })
                    .floor()
            } else {
                b.module
                    * if matches!(v, 0 | 9 | 10) {
                        2. * b.ratio - 1.
                    } else {
                        b.ratio
                    }
            };
            if i % 2 == 0 {
                path.rect(x, 0., width, b.height);
            }
            x += width;
        }
    }
    Ok(path)
}
