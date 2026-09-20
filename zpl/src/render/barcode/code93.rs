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
pub(super) fn interpretation(data: &[u8], normalize: bool, checks: bool) -> Result<String, String> {
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
    if checks {
        let mut checked = values;
        append_checks(&mut checked);
        for &v in &checked[checked.len() - 2..] {
            // ^BA p. 88: optional C/K interpretation. Extended values use
            // the documented ZPL control-code substitutes from pp. 88–89.
            text.push(if v < 43 { ALPHABET[v] } else { b"&'()"[v - 43] } as char);
        }
    }
    Ok(text)
}
fn append_checks(values: &mut Vec<usize>) {
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
}
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    b.flag(4, false)?;
    const PATTERNS: [u32; 48] = [
        0x114, 0x148, 0x144, 0x142, 0x128, 0x124, 0x122, 0x150, 0x112, 0x10a, 0x1a8, 0x1a4, 0x1a2,
        0x194, 0x192, 0x18a, 0x168, 0x164, 0x162, 0x134, 0x11a, 0x158, 0x14c, 0x146, 0x12c, 0x116,
        0x1b4, 0x1b2, 0x1ac, 0x1a6, 0x196, 0x19a, 0x16c, 0x166, 0x136, 0x13a, 0x12e, 0x1d4, 0x1d2,
        0x1ca, 0x16e, 0x176, 0x1ae, 0x126, 0x1da, 0x1d6, 0x132, 0x15e,
    ];
    let mut values = field_values(data, b.compatibility.code93_normalize_input)?;
    append_checks(&mut values);
    let mut out = Vec::new();
    for v in std::iter::once(47).chain(values).chain(std::iter::once(47)) {
        append_pattern(&mut out, PATTERNS[v], 9);
    }
    out.push(true);
    b.linear(&out, None)
}

pub(super) struct Caption {
    pub text: String,
    pub glyphs: Vec<(usize, &'static [u8])>,
}
impl Caption {
    fn push(&mut self, byte: u8) {
        // Native barcode-interpretation cells measured independently across
        // all 188 extended C/K pairs. Control cells 22–25 also produce the
        // Code 11 triangles and Code 93 delimiters. See fixture provenance.
        let glyph: Option<&'static [u8]> = match byte {
            19 => Some(&[15, 20, 20, 22, 20, 20, 15]),
            20 => Some(&[0, 0, 10, 21, 23, 20, 11]),
            21 => Some(&[6, 9, 28, 8, 28, 9, 6]),
            22 => Some(&[4, 10, 17, 31]),
            23 => Some(&[4, 4, 10, 10, 17, 17, 31]),
            24 => Some(&[31, 17, 17, 17, 17, 17, 31]),
            25 => Some(&[31, 31, 31, 31, 31, 31, 31]),
            92 => Some(&[4, 14, 21, 20, 21, 14, 4]),
            _ => None,
        };
        if let Some(glyph) = glyph {
            self.glyphs.push((self.text.len(), glyph));
            self.text.push(' ');
        } else {
            self.text.push(match byte {
                32..=95 | 97..=126 => byte as char,
                96 => '\'',
                _ => ' ',
            });
        }
    }
}
/// ^BA e (p. 88) requests C/K interpretation. Extended C values enter the
/// printer's shift formatter with K as lookahead, but still print K again.
/// Control/error paths instead emit a solid cell and three tail cells without
/// a stop delimiter. The specification profile keeps literal ZPL substitutes.
/// This finite symbol-domain behavior is covered for every extended C/K pair,
/// with separate payload/size/orientation controls (code93-checks-zd621-v1).
pub(super) fn extended_caption(b: &Barcode, data: &[u8]) -> Result<Option<Caption>, String> {
    if !b.compatibility.code93_extended_checksum_preview || !b.flag(4, false)? {
        return Ok(None);
    }
    let mut values = field_values(data, b.compatibility.code93_normalize_input)?;
    append_checks(&mut values);
    let c = values[values.len() - 2];
    let k = values[values.len() - 1];
    if c < 43 {
        return Ok(None);
    }
    let k_char = if k < 43 { ALPHABET[k] } else { b"&'()"[k - 43] };
    let mut caption = Caption {
        text: String::new(),
        glyphs: Vec::new(),
    };
    let symbols = b.compatibility.code93_interpretation_symbols;
    if symbols {
        caption.push(24);
    }
    caption.text.push_str(&interpretation(
        data,
        b.compatibility.code93_normalize_input,
        false,
    )?);
    let (first, tail, repeat) = match c {
        43 => (25, k_char, true),
        44 => match k {
            15..=19 => (k_char - 11, k_char, false),
            20..=24 => (k_char + 16, k_char, false),
            25..=28 => (k_char + 43, k_char, false),
            31 => (b'@', k_char, false),
            32 => (b'`', k_char, false),
            10..=14 => (25, k_char + 26, true),
            30 => (25, b'@', true),
            _ => (25, b' ', true),
        },
        45 => (k_char - 32, k_char, false),
        46 => (k_char.to_ascii_lowercase(), k_char, false),
        _ => unreachable!(),
    };
    caption.push(first);
    for _ in 0..if repeat { 3 } else { 1 } {
        caption.push(tail);
    }
    if symbols && !repeat {
        caption.push(24);
    }
    Ok(Some(caption))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_checks_use_the_same_c_and_k_as_the_symbol() {
        // Zebra ^BA e, p. 88. Captured ABC123 checks are W and 9.
        assert_eq!(interpretation(b"ABC123", false, true).unwrap(), "ABC123W9");
        assert_eq!(interpretation(b"AN", false, true).unwrap(), "AN&P");
        assert_eq!(interpretation(b"AO", false, true).unwrap(), "AO'S");
        assert_eq!(interpretation(b"AP", false, true).unwrap(), "AP(V");
        assert_eq!(interpretation(b"AQ", false, true).unwrap(), "AQ)Y");
    }
    #[test]
    fn interpretation_uses_zpl_shifts() {
        assert_eq!(interpretation(b"Hello93!", true, false).unwrap(), "HELLO93");
        assert!(interpretation(b"Hello93!", false, false).is_err());
        for normalize in [false, true] {
            assert_eq!(
                interpretation(b")A)B)C(A", normalize, false).unwrap(),
                "abc!"
            );
            assert_eq!(
                interpretation(b"(D(E(O'V'W", normalize, false).unwrap(),
                "$%/@`"
            );
            assert_eq!(interpretation(b"'U&A", normalize, false).unwrap(), "\0\x01");
        }
    }
}
