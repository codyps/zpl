use super::*;
const DIGITS: [[usize; 5]; 10] = [
    [1, 1, 3, 3, 1],
    [3, 1, 1, 1, 3],
    [1, 3, 1, 1, 3],
    [3, 3, 1, 1, 1],
    [1, 1, 3, 1, 3],
    [3, 1, 3, 1, 1],
    [1, 3, 3, 1, 1],
    [1, 1, 1, 3, 3],
    [3, 1, 1, 3, 1],
    [1, 3, 1, 3, 1],
];
pub(super) fn encode(b: &Barcode, data: &[u8], kind: u8, check: bool) -> Result<Path, String> {
    let mut v = digits(data)?;
    if check {
        v.push(mod10(&v));
    }
    let mut out = Vec::new();
    if kind == 0 {
        if v.len() % 2 != 0 {
            v.insert(0, 0);
        }
        runs(&mut out, &[1, 1, 1, 1]);
        for pair in v.as_chunks::<2>().0 {
            let mut w = Vec::new();
            for (&bar, &space) in DIGITS[pair[0] as usize]
                .iter()
                .zip(&DIGITS[pair[1] as usize])
            {
                w.extend([bar, space]);
            }
            runs(&mut out, &w);
        }
        runs(&mut out, &[3, 1, 1]);
    } else {
        runs(
            &mut out,
            if kind == 1 {
                &[3, 1, 3, 1, 1, 1][..]
            } else {
                &[1, 1, 1, 1][..]
            },
        );
        for d in v {
            let mut widths = Vec::new();
            for w in DIGITS[d as usize] {
                widths.extend([w, 1]);
            }
            runs(&mut out, &widths);
        }
        runs(
            &mut out,
            if kind == 1 {
                &[3, 1, 1, 1, 3][..]
            } else {
                &[3, 1, 1][..]
            },
        );
    }
    b.linear(&out, Some(b.ratio))
}
