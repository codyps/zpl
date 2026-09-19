//! Code 93; Zebra programming guide, printed page 87.
//! Specification links and implementation limits: docs/barcodes.md.
//! <https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf>
use super::*;
const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%";
fn field_values(data: &[u8], normalize: bool) -> Result<Vec<usize>, String> {
    // Zebra guide pp. 87–89: these are control-code substitutes, not literal
    // punctuation. The ZD621 uppercases raw letters and skips unsupported bytes.
    let mut values = Vec::new();
    for &c in data {
        let c = if normalize { c.to_ascii_uppercase() } else { c };
        let value = match c {
            b'&' => Some(43),
            b'\'' => Some(44),
            b'(' => Some(45),
            b')' => Some(46),
            _ => ALPHABET.iter().position(|&v| v == c),
        };
        if let Some(value) = value {
            values.push(value);
        } else if !normalize {
            return Err("Code 93 requires the ZPL alphabet or full-ASCII shift substitutes".into());
        }
    }
    Ok(values)
}
pub(super) fn interpretation(data: &[u8], normalize: bool) -> Result<String, String> {
    let values = field_values(data, normalize)?;
    let mut text = String::new();
    let mut i = 0;
    while i < values.len() {
        let v = values[i];
        if v < 43 {
            text.push(ALPHABET[v] as char);
            i += 1;
            continue;
        }
        let next = *values.get(i + 1).ok_or("Code 93 incomplete shift pair")?;
        let c = *ALPHABET.get(next).ok_or("Code 93 invalid shift pair")?;
        let ch = match (v, c) {
            (43, b'A'..=b'Z') => c - b'A' + 1,
            (44, b'A'..=b'E') => c - b'A' + 27,
            (44, b'F'..=b'J') => c - b'F' + 59,
            (44, b'K'..=b'O') => c - b'K' + 91,
            (44, b'P'..=b'T') => c - b'P' + 123,
            (44, b'U') => 0,
            (44, b'V') => b'@',
            (44, b'W') => b'`',
            (44, b'X'..=b'Z') => 127,
            (45, b'A'..=b'O') => c - b'A' + b'!',
            (45, b'Z') => b':',
            (46, b'A'..=b'Z') => c - b'A' + b'a',
            _ => return Err("Code 93 invalid shift pair".into()),
        };
        text.push(ch as char);
        i += 2;
    }
    Ok(text)
}
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    if b.flag(4, false)? {
        return Err("Code 93 check characters in interpretation text are not implemented".into());
    }
    const PATTERNS: [u32; 48] = [
        0x114, 0x148, 0x144, 0x142, 0x128, 0x124, 0x122, 0x150, 0x112, 0x10a, 0x1a8, 0x1a4, 0x1a2,
        0x194, 0x192, 0x18a, 0x168, 0x164, 0x162, 0x134, 0x11a, 0x158, 0x14c, 0x146, 0x12c, 0x116,
        0x1b4, 0x1b2, 0x1ac, 0x1a6, 0x196, 0x19a, 0x16c, 0x166, 0x136, 0x13a, 0x12e, 0x1d4, 0x1d2,
        0x1ca, 0x16e, 0x176, 0x1ae, 0x126, 0x1da, 0x1d6, 0x132, 0x15e,
    ];
    let mut values = field_values(data, b.compatibility.code93_normalize_input)?;
    for cycle in [20, 15] {
        let check = values
            .iter()
            .rev()
            .enumerate()
            .map(|(i, &v)| (i % cycle + 1) * v)
            .sum::<usize>()
            % 47;
        values.push(check);
    }
    let mut out = Vec::new();
    for v in std::iter::once(47).chain(values).chain(std::iter::once(47)) {
        append_pattern(&mut out, PATTERNS[v], 9);
    }
    out.push(true);
    b.linear(&out, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interpretation_uses_zpl_shifts() {
        assert_eq!(interpretation(b"Hello93!", true).unwrap(), "HELLO93");
        assert!(interpretation(b"Hello93!", false).is_err());
        for normalize in [false, true] {
            assert_eq!(interpretation(b")A)B)C(A", normalize).unwrap(), "abc!");
            assert_eq!(interpretation(b"(D(E(O'V'W", normalize).unwrap(), "$%/@`");
            assert_eq!(interpretation(b"'U&A", normalize).unwrap(), "\0\x01");
        }
    }
}
