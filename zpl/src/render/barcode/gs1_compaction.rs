//! Original common GS1 general-purpose compaction: ISO/IEC 24723:2010 §5.4
//! <https://www.iso.org/standard/51425.html> and ISO/IEC 24724:2011 §7.2.5.5.
//! Numeric/alphanumeric/ISO-646 modes; caller supplies header and capacities.
use super::*;
fn numeric(c: u8) -> bool {
    c.is_ascii_digit() || c == 29
}
fn alpha(c: u8) -> bool {
    numeric(c) || c.is_ascii_uppercase() || b"*,-./".contains(&c)
}

pub(super) fn encode(
    data: &[u8],
    out: Vec<bool>,
    capacity_for: impl Fn(usize) -> Option<usize>,
) -> Result<Vec<bool>, String> {
    if data.is_empty() {
        return Err("GS1 requires a GS1 AI element string".into());
    }
    encode_remainder(data, out, capacity_for)
}

/// Encode the optional general-purpose field after a compressed AI header.
pub(super) fn encode_remainder(
    data: &[u8],
    mut out: Vec<bool>,
    capacity_for: impl Fn(usize) -> Option<usize>,
) -> Result<Vec<bool>, String> {
    let mut mode = 0; // Numeric, Alphanumeric, ISO/IEC 646
    let mut i = 0;
    while i < data.len() {
        let c = data[i];
        let tail = &data[i..];
        let numbers = tail.iter().take_while(|&&v| numeric(v)).count();
        match mode {
            0 => {
                if tail.len() >= 2 && numeric(c) && numeric(tail[1]) && !(c == 29 && tail[1] == 29)
                {
                    let digit = |v: u8| if v == 29 { 10 } else { (v - b'0') as usize };
                    bits::push(&mut out, 11 * digit(c) + digit(tail[1]) + 8, 7);
                    i += 2;
                    continue;
                }
                if tail.len() == 1 && c.is_ascii_digit() {
                    // Four-bit terminal digit where it fits; otherwise append
                    // a numeric digit/FNC1 pair whose final FNC1 is padding.
                    let capacity =
                        capacity_for(out.len()).ok_or("GS1 component capacity exceeded")?;
                    if (4..=6).contains(&(capacity - out.len())) {
                        bits::push(&mut out, (c - b'0' + 1) as usize, 4);
                    } else {
                        bits::push(&mut out, 11 * (c - b'0') as usize + 18, 7);
                    }
                    i += 1;
                    continue;
                }
                bits::push(&mut out, 0, 4);
                mode = 1;
                continue;
            }
            1 => {
                if c != 29 && (numbers >= 6 || numbers >= 4 && numbers == tail.len()) {
                    bits::push(&mut out, 0, 3);
                    mode = 0;
                    continue;
                }
                if !alpha(c) {
                    bits::push(&mut out, 4, 5);
                    mode = 2;
                    continue;
                }
                if c == 29 {
                    bits::push(&mut out, 15, 5);
                    mode = 0;
                } else if c.is_ascii_digit() {
                    bits::push(&mut out, (c - 43) as usize, 5);
                } else if c.is_ascii_uppercase() {
                    bits::push(&mut out, (c - 33) as usize, 6);
                } else {
                    bits::push(
                        &mut out,
                        58 + b"*,-./".iter().position(|&v| v == c).unwrap(),
                        6,
                    );
                }
            }
            _ => {
                let no_iso = tail.iter().take(10).all(|&v| alpha(v));
                if c != 29 && numbers >= 4 && no_iso {
                    bits::push(&mut out, 0, 3);
                    mode = 0;
                    continue;
                }
                if c != 29 && tail.iter().take_while(|&&v| alpha(v)).count() >= 5 && no_iso {
                    bits::push(&mut out, 4, 5);
                    mode = 1;
                    continue;
                }
                if c == 29 {
                    bits::push(&mut out, 15, 5);
                    mode = 0;
                } else if c.is_ascii_digit() {
                    bits::push(&mut out, (c - 43) as usize, 5);
                } else if c.is_ascii_uppercase() {
                    bits::push(&mut out, (c - 1) as usize, 7);
                } else if c.is_ascii_lowercase() {
                    bits::push(&mut out, (c - 7) as usize, 7);
                } else if let Some(p) = b"!\"%&'()*+,-./:;<=>?_ ".iter().position(|&v| v == c) {
                    bits::push(&mut out, 232 + p, 8);
                } else {
                    return Err("character outside GS1's ISO/IEC 646 repertoire".into());
                }
            }
        }
        i += 1;
        if capacity_for(out.len()).is_none() {
            return Err("GS1 component capacity exceeded".into());
        }
    }
    let capacity = capacity_for(out.len()).ok_or("GS1 component capacity exceeded")?;
    if out.len() < capacity && mode == 0 {
        bits::push(&mut out, 0, 4);
    }
    while out.len() < capacity {
        bits::push(&mut out, 4, 5);
    }
    out.truncate(capacity);
    Ok(out)
}
