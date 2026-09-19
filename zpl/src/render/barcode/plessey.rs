//! Plessey; Zebra programming guide, printed page 126.
//! Specification links and implementation limits: docs/barcodes.md.
//! <https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf>
use super::*;
fn checked_bits(data: &[u8]) -> Result<Vec<bool>, String> {
    let mut bits = Vec::new();
    for &c in data {
        let d = match c {
            b'0'..=b'9' => c - b'0',
            b'A'..=b'F' => c - b'A' + 10,
            _ => return Err("Plessey requires uppercase hexadecimal digits".into()),
        };
        bits.extend((0..4).map(|i| d & (1 << i) != 0));
    }
    let mut division = bits.clone();
    division.resize(bits.len() + 8, false);
    for i in 0..bits.len() {
        if division[i] {
            for j in [0, 1, 2, 3, 5, 8] {
                division[i + j] = !division[i + j];
            }
        }
    }
    bits.extend_from_slice(&division[data.len() * 4..]);
    Ok(bits)
}

pub(super) fn interpretation(b: &Barcode, data: &[u8]) -> Result<String, String> {
    let bits = checked_bits(data)?;
    let mut text = ascii(data)?.to_string();
    if b.flag(1, false)? {
        for chunk in bits[data.len() * 4..].as_chunks::<4>().0 {
            let value = chunk
                .iter()
                .enumerate()
                .fold(0, |n, (i, &bit)| n | ((bit as usize) << i));
            text.push(b"0123456789ABCDEF"[value] as char);
        }
    }
    Ok(text)
}
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    b.flag(1, false)?; // Controls interpretation only; CRC is always encoded.
    let bits = checked_bits(data)?;
    let w = b.ratio;
    let mut widths = vec![w, 1., w, 1., 1., w, w, 1.];
    for bit in bits {
        widths.extend(if bit { [w, 1.] } else { [1., w] });
    }
    widths.extend([w + 1., w, 1., w, 1., 1., w, 1., w]);
    let mut path = Path::default();
    let mut x = 0.;
    for (i, width) in widths.into_iter().enumerate() {
        let width = width * b.module;
        if i % 2 == 0 {
            path.rect(x, 0., width, b.height);
        }
        x += width;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn check_display_does_not_change_bars() {
        let no = Barcode::new(
            "BP",
            &["N", "N", "80", "Y", "N"],
            2.,
            2.,
            80.,
            203,
            crate::render::profiles::SPECIFICATION.compatibility,
        )
        .unwrap();
        let yes = Barcode::new(
            "BP",
            &["N", "Y", "80", "Y", "N"],
            2.,
            2.,
            80.,
            203,
            crate::render::profiles::SPECIFICATION.compatibility,
        )
        .unwrap();
        assert_eq!(interpretation(&no, b"123ABC").unwrap(), "123ABC");
        assert_eq!(interpretation(&yes, b"123ABC").unwrap(), "123ABC0B");
        assert_eq!(
            render(&no, b"123ABC").unwrap(),
            render(&yes, b"123ABC").unwrap()
        );
    }
}
