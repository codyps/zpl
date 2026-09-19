//! Code 128 (ISO/IEC 15417), retained from the original local renderer.
//! Specification links and implementation limits: docs/barcodes.md.
use crate::output::Path;
// Code 128 symbol widths (alternating bars/spaces), ISO/IEC 15417.
pub(super) const WIDTHS: [&str; 107] = [
    "212222", "222122", "222221", "121223", "121322", "131222", "122213", "122312", "132212",
    "221213", "221312", "231212", "112232", "122132", "122231", "113222", "123122", "123221",
    "223211", "221132", "221231", "213212", "223112", "312131", "311222", "321122", "321221",
    "312212", "322112", "322211", "212123", "212321", "232121", "111323", "131123", "131321",
    "112313", "132113", "132311", "211313", "231113", "231311", "112133", "112331", "132131",
    "113123", "113321", "133121", "313121", "211331", "231131", "213113", "213311", "213131",
    "311123", "311321", "331121", "312113", "312311", "332111", "314111", "221411", "431111",
    "111224", "111422", "121124", "121421", "141122", "141221", "112214", "112412", "122114",
    "122411", "142112", "142211", "241211", "221114", "413111", "241112", "134111", "111242",
    "121142", "121241", "114212", "124112", "124211", "411212", "421112", "421211", "212141",
    "214121", "412121", "111143", "111341", "131141", "114113", "114311", "411113", "411311",
    "113141", "114131", "311141", "411131", "211412", "211214", "211232", "2331112",
];

// Zebra ZPL II Programming Guide, ^BC pp. 94–103, especially Table 2
// (invocations), automatic-mode digit runs and UCC/EAN data preparation.
// https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
#[derive(Clone, Copy)]
enum Token {
    Byte(u8),
    Word(usize),
}
fn tokens(data: &[u8]) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'>' {
            let c = *data.get(i + 1).ok_or("incomplete Code 128 invocation")?;
            out.push(match c {
                b'0' => Token::Byte(b'>'),
                b'<' => Token::Word(62),
                b'=' => Token::Word(94),
                b'1'..=b'9' => Token::Word((c - b'1') as usize + 95),
                b':' => Token::Word(104),
                b';' => Token::Word(105),
                _ => return Err("invalid Code 128 invocation".into()),
            });
            i += 2;
        } else {
            if data[i] > 127 {
                return Err("Code 128 requires ASCII bytes or invocation sequences".into());
            }
            out.push(Token::Byte(data[i]));
            i += 1;
        }
    }
    Ok(out)
}
fn check_digit(data: &[u8]) -> u8 {
    let sum: usize = data
        .iter()
        .rev()
        .enumerate()
        .map(|(i, c)| (c - b'0') as usize * if i % 2 == 0 { 3 } else { 1 })
        .sum();
    b'0' + ((10 - sum % 10) % 10) as u8
}
fn gs1(data: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    // Parentheses delimit application identifiers; variable-length elements
    // need FNC1 before the next AI. Guide Table 4, pp. 103–104.
    if !data.starts_with(b"(") {
        let clean: Vec<_> = data.iter().copied().filter(|c| *c != b' ').collect();
        if clean.starts_with(b"00") && clean.len() == 20
            || (clean.starts_with(b"01") || clean.starts_with(b"02")) && clean.len() == 16
        {
            let wrapped = [b"(".as_slice(), &clean[..2], b")", &clean[2..]].concat();
            let (encoded, _) = gs1(&wrapped)?;
            let mut display = data.to_vec();
            if let Some(last) = display.iter_mut().rfind(|c| **c != b' ') {
                *last = *encoded.last().unwrap();
            }
            return Ok((encoded, display));
        }
        return Ok((clean, data.to_vec()));
    }
    let mut out = Vec::new();
    let mut edits = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        if data[pos] != b'(' {
            return Err("invalid Code 128 application identifier".into());
        }
        let close = data[pos + 1..]
            .iter()
            .position(|c| *c == b')')
            .ok_or("unclosed application identifier")?
            + pos
            + 1;
        let ai = &data[pos + 1..close];
        if !(2..=4).contains(&ai.len()) || !ai.iter().all(u8::is_ascii_digit) {
            return Err("invalid Code 128 application identifier".into());
        }
        let end = data[close + 1..]
            .iter()
            .position(|c| *c == b'(')
            .map_or(data.len(), |n| close + 1 + n);
        let mut value: Vec<u8> = data[close + 1..end]
            .iter()
            .copied()
            .filter(|c| *c != b' ')
            .collect();
        let checked = match ai {
            b"00" => Some(18),
            b"01" | b"02" => Some(14),
            _ => None,
        };
        if let Some(n) = checked {
            if !value.iter().all(u8::is_ascii_digit) {
                return Err("GS1 identification key requires digits".into());
            }
            let append = value.len() == n - 1;
            if append {
                value.push(check_digit(&value));
            }
            if value.len() != n {
                return Err("invalid GS1 identification key length".into());
            }
            value[n - 1] = check_digit(&value[..n - 1]);
            let last = (close + 1..end).rfind(|&i| data[i] != b' ').unwrap();
            edits.push((last + usize::from(append), append, value[n - 1]));
        }
        let fixed = checked.is_some()
            || matches!(ai, b"11" | b"12" | b"13" | b"15" | b"16" | b"17" | b"20")
            || (ai.len() == 4 && ai[0] == b'3' && ai[1].is_ascii_digit() && ai[1] <= b'6');
        out.extend(ai);
        out.extend(value);
        if !fixed && end < data.len() {
            out.push(29);
        }
        pos = end;
    }
    let mut display = data.to_vec();
    for (index, insert, digit) in edits.into_iter().rev() {
        if insert {
            display.insert(index, digit);
        } else {
            display[index] = digit;
        }
    }
    Ok((out, display))
}
fn encode(b: &super::Barcode, data: &[u8]) -> Result<(Vec<usize>, String), String> {
    b.require(5, "N", &["N", "A", "U", "D"])?;
    let mode = b.param(5, "N");
    let mut prepared = data.to_vec();
    let mut gs1_display = None;
    if mode == "U" {
        if !prepared.iter().all(u8::is_ascii_digit) {
            return Err("UCC case mode requires digits".into());
        }
        prepared.resize(19, b'0');
        prepared.push(check_digit(&prepared));
    } else if mode == "D" {
        let (encoded, display) = gs1(data)?;
        prepared = encoded;
        gs1_display = Some(display);
    } else if b.flag(4, false)? {
        // UCC Mod-10 covers the numeric field before the generated digit.
        let digits: Vec<_> = tokens(&prepared)?
            .iter()
            .filter_map(|t| match t {
                Token::Byte(c) => Some(*c),
                Token::Word(_) => None,
            })
            .collect();
        if digits.len() < 3 || !digits.iter().all(u8::is_ascii_digit) {
            return Err("UCC check digit requires an AI and numeric data".into());
        }
        prepared.push(check_digit(&digits));
    }
    b.flag(4, false)?;
    let input = tokens(&prepared)?;
    let auto = mode != "N";
    let mut i = 0;
    let mut set = 1usize; // A=0, B=1, C=2
    let digits = |start: usize| {
        input[start..]
            .iter()
            .take_while(|t| matches!(t,Token::Byte(c) if c.is_ascii_digit()))
            .count()
    };
    if let Some(Token::Word(w @ 103..=105)) = input.first() {
        set = w - 103;
        i = 1;
    } else if auto {
        set = if digits(0) >= 4 {
            2
        } else if matches!(input.first(), Some(Token::Byte(0..=31))) {
            0
        } else {
            1
        };
    }
    let mut words = vec![103 + set];
    let mut display = Vec::new();
    if matches!(mode, "U" | "D") {
        words.push(102);
    }
    let mut shift = None;
    while i < input.len() {
        match input[i] {
            Token::Word(w) => {
                if w >= 103 {
                    return Err("Code 128 start invocation must precede data".into());
                }
                words.push(w);
                match w {
                    99 if set != 2 => set = 2,
                    100 if set != 1 => set = 1,
                    101 if set != 0 => set = 0,
                    98 if set != 2 => {
                        shift = Some(set);
                        set = 1 - set;
                    }
                    102 => {}
                    96 | 97 => {}
                    100 | 101 => return Err("Code 128 extended-byte FNC4 mode unsupported".into()),
                    _ if set != 2 && w <= 95 => display.push(if set == 0 && w >= 64 {
                        (w - 64) as u8
                    } else {
                        (w + 32) as u8
                    }),
                    _ => {}
                }
                if w <= 95 {
                    if let Some(previous) = shift.take() {
                        set = previous;
                    }
                }
                i += 1;
            }
            Token::Byte(c) => {
                if mode == "D" && c == 29 {
                    words.push(102);
                    i += 1;
                    continue;
                }
                if auto {
                    if set != 2 && digits(i) >= 4 {
                        words.push(99);
                        set = 2;
                        continue;
                    }
                    if set == 2 && digits(i) < 2 {
                        set = if c < 32 { 0 } else { 1 };
                        words.push(if set == 0 { 101 } else { 100 });
                        continue;
                    }
                    if set == 0 && c >= 96 {
                        words.push(100);
                        set = 1;
                    }
                    if set == 1 && c < 32 {
                        words.push(101);
                        set = 0;
                    }
                }
                if set == 2 || (set == 0 && !auto) {
                    let next = input.get(i + 1);
                    if !c.is_ascii_digit() {
                        i += 1;
                        continue;
                    }
                    if let Some(Token::Byte(d)) = next {
                        if d.is_ascii_digit() {
                            let v = ((c - b'0') * 10 + d - b'0') as usize;
                            words.push(v);
                            if set == 2 {
                                display.extend([c, *d]);
                            } else if v <= 95 {
                                display.push(if v >= 64 {
                                    (v - 64) as u8
                                } else {
                                    (v + 32) as u8
                                });
                            }
                        }
                        i += 2;
                    } else {
                        i += 1;
                    }
                } else {
                    if (set == 0 && c > 95) || (set == 1 && c < 32) {
                        return Err("Code 128 byte outside selected subset".into());
                    }
                    words.push(if c < 32 {
                        (c + 64) as usize
                    } else {
                        (c - 32) as usize
                    });
                    display.push(c);
                    i += 1;
                }
                if let Some(previous) = shift.take() {
                    set = previous;
                }
            }
        }
    }
    if shift.is_some() {
        return Err("Code 128 SHIFT requires a following character".into());
    }
    let checksum = (words[0]
        + words[1..]
            .iter()
            .enumerate()
            .map(|(i, v)| (i + 1) * v)
            .sum::<usize>())
        % 103;
    words.extend([checksum, 106]);
    let text = gs1_display.unwrap_or(display);
    // Function/control characters have no printable interpretation glyph.
    Ok((
        words,
        text.into_iter()
            .filter(|c| (32..=126).contains(c))
            .map(char::from)
            .collect(),
    ))
}
pub(super) fn interpretation(b: &super::Barcode, data: &[u8]) -> Result<String, String> {
    Ok(encode(b, data)?.1)
}
pub(super) fn render(b: &super::Barcode, data: &[u8]) -> Result<Path, String> {
    let (words, _) = encode(b, data)?;
    let mut path = Path::default();
    let mut x = 0.;
    for word in words {
        for (i, w) in WIDTHS[word].bytes().enumerate() {
            let width = f64::from(w - b'0') * b.module;
            if i % 2 == 0 {
                path.rect(x, 0., width, b.height);
            }
            x += width;
        }
    }
    Ok(path)
}
