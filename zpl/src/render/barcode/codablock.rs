//! CODABLOCK F, AIM USS (1995): row indicators and modulo-86/103 checks.
//! <https://barcodeguide.seagullscientific.com/Content/Symbologies/Codablock_F.htm>
use super::*;
fn control(n: usize) -> usize {
    if n < 48 {
        (n + 64) % 96
    } else {
        n - 22
    }
}
fn encode(b: &Barcode, data: &[u8]) -> Result<Matrix, String> {
    b.require(5, "F", &["F", "E"])?;
    if !b.flag(2, true)? {
        return Err("CODABLOCK F/E requires symbol checksums".into());
    }
    if data.iter().any(|&c| c > 127) {
        return Err("CODABLOCK currently requires ASCII bytes without ZPL function escapes".into());
    }
    let requested = b.integer(4, 0, 0, 44)?;
    let columns = b.integer(
        3,
        if requested > 0 {
            (data.len() + 2).div_ceil(requested).clamp(4, 62)
        } else {
            20
        },
        4,
        62,
    )?;
    let mut rows: Vec<(Vec<usize>, bool)> = Vec::new();
    let mut cursor = 0;
    loop {
        let mut values = Vec::new();
        let mut set_b = true;
        if rows.is_empty() && b.param(5, "F") == "E" {
            values.push(102);
        }
        while cursor < data.len() {
            let c = data[cursor];
            let target = if c < 32 {
                false
            } else if c >= 96 {
                true
            } else {
                set_b
            };
            let cost = 1 + usize::from(target != set_b);
            if values.len() + cost > columns {
                break;
            }
            if target != set_b {
                values.push(if target { 100 } else { 101 });
                set_b = target;
            }
            values.push(if c < 32 {
                c as usize + 64
            } else {
                c as usize - 32
            });
            cursor += 1;
        }
        let final_row = cursor == data.len()
            && values.len() + 2 <= columns
            && rows.len() + 1 >= requested.max(2);
        let limit = columns - if final_row { 2 } else { 0 };
        while values.len() < limit {
            values.push(if set_b { 101 } else { 100 });
            set_b = !set_b;
        }
        if final_row {
            let k1 = data
                .iter()
                .enumerate()
                .map(|(i, &c)| (i + 1) * c as usize)
                .sum::<usize>()
                % 86;
            let k2 = data
                .iter()
                .enumerate()
                .map(|(i, &c)| i * c as usize)
                .sum::<usize>()
                % 86;
            values.extend([control(k1), control(k2)]);
        }
        rows.push((values, set_b));
        if final_row {
            break;
        }
        if rows.len() >= 44 {
            return Err("CODABLOCK exceeds 44 rows".into());
        }
    }
    let mut m = Matrix::new((columns + 4) * 11 + 13, rows.len());
    for (y, (row, _)) in rows.iter().enumerate() {
        let indicator = if y == 0 { rows.len() - 2 } else { y + 42 };
        let mut words = vec![103, 100, control(indicator)];
        words.extend(row);
        let check = (words[0]
            + words[1..]
                .iter()
                .enumerate()
                .map(|(i, &v)| (i + 1) * v)
                .sum::<usize>())
            % 103;
        words.extend([check, 106]);
        let mut bits = Vec::new();
        for w in words {
            let widths: Vec<_> = code128::WIDTHS[w]
                .bytes()
                .map(|v| (v - b'0') as usize)
                .collect();
            runs(&mut bits, &widths);
        }
        for (x, v) in bits.into_iter().enumerate() {
            m.set(x, y, v);
        }
    }
    Ok(m)
}
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    let m = encode(b, data)?;
    let row = b.num(1, 8., 2., 32000.)? * b.module;
    let mut path = b.matrix(&m, b.module, row)?;
    let width = m.w as f64 * b.module;
    path.rect(0., -b.module, width, b.module);
    path.rect(0., m.h as f64 * row, width, b.module);
    Ok(path)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_decode() {
        for data in [
            b"Hello CODABLOCK F".as_slice(),
            b"\x01ABC\x7fDEF".as_slice(),
            b"A".as_slice(),
        ] {
            let b = Barcode::new("BB", &["N", "8", "Y", "8"], 2., 2., 90., 203).unwrap();
            let m = encode(&b, data).unwrap();
            let mut other = anyd::output::BitMatrix::new(m.w, m.h, 0);
            for y in 0..m.h {
                for x in 0..m.w {
                    other.set(x, y, m.get(x, y));
                }
            }
            let result = anyd::codes::codablockf::CodablockFDecoder::new()
                .decode_matrix(&other)
                .unwrap();
            assert_eq!(result.text().unwrap().as_bytes(), data);
        }
    }
}
