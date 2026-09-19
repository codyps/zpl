//! Original ECC200 high-level encoder, ISO/IEC 16022:2006 §§5.2.4–5.2.9,
//! Annex C (tables) and Annex P (look-ahead rules).
//! https://www.iso.org/standard/44230.html
//! Checked against the supplied second edition and Cor.1:2008 / Cor.2:2011.
//! Corrigenda revise print-quality grading and reference decoding, not these
//! encodation rules. Annex P(p)'s repeated "X12 count" is a typographical error:
//! that paragraph updates the EDIFACT count, as its heading specifies.
const ASCII: usize = 0;
const C40: usize = 1;
const TEXT: usize = 2;
const X12: usize = 3;
const EDF: usize = 4;
const B256: usize = 5;
pub(super) const FNC1: u16 = 256;
fn native(c: u16, mode: usize) -> bool {
    match mode {
        C40 => c == 32 || (48..=57).contains(&c) || (65..=90).contains(&c),
        TEXT => c == 32 || (48..=57).contains(&c) || (97..=122).contains(&c),
        X12 => native(c, C40) || matches!(c, 13 | 42 | 62),
        EDF => (32..=94).contains(&c),
        _ => false,
    }
}
/// Counters are in twelfths of a codeword, so all fractional increments in
/// Annex P are exact, independent of floating-point rounding or scan length.
fn look_ahead(data: &[u16], mode: usize) -> usize {
    let mut cost = if mode == ASCII {
        [0, 12, 12, 12, 12, 15]
    } else {
        [12, 24, 24, 24, 24, 27]
    };
    cost[mode] = 0usize;
    for (i, &c) in data.iter().enumerate() {
        cost[ASCII] = if (48..=57).contains(&c) {
            cost[ASCII] + 6
        } else {
            cost[ASCII].div_ceil(12) * 12 + if (128..256).contains(&c) { 24 } else { 12 }
        };
        for mode in [C40, TEXT] {
            cost[mode] += if native(c, mode) {
                8
            } else if (128..256).contains(&c) {
                32
            } else {
                16
            };
        }
        cost[X12] += if native(c, X12) {
            8
        } else if (128..256).contains(&c) {
            52
        } else {
            40
        };
        cost[EDF] += if native(c, EDF) {
            9
        } else if (128..256).contains(&c) {
            51
        } else {
            39
        };
        cost[B256] += if c == FNC1 { 48 } else { 12 };
        if i + 1 >= 4 {
            let less = |m: usize, others: &[usize]| others.iter().all(|&j| cost[m] + 12 < cost[j]);
            if (1..6).all(|m| cost[ASCII] + 12 <= cost[m]) {
                return ASCII;
            }
            if cost[B256] + 12 <= cost[ASCII] || less(B256, &[ASCII, C40, TEXT, X12, EDF]) {
                return B256;
            }
            for m in [EDF, TEXT, X12] {
                if (0..6).filter(|&j| j != m).all(|j| cost[m] + 12 < cost[j]) {
                    return m;
                }
            }
            if less(C40, &[ASCII, B256, EDF, TEXT]) {
                if cost[C40] < cost[X12] {
                    return C40;
                }
                if cost[C40] == cost[X12] {
                    for &next in &data[i + 1..] {
                        if matches!(next, 13 | 42 | 62) {
                            return X12;
                        }
                        if !native(next, X12) {
                            break;
                        }
                    }
                    return C40;
                }
            }
        }
    }
    let rounded = cost.map(|v| v.div_ceil(12));
    if rounded.iter().all(|&v| rounded[ASCII] <= v) {
        return ASCII;
    }
    for m in [B256, EDF, TEXT, X12] {
        if (0..6).filter(|&j| j != m).all(|j| rounded[m] < rounded[j]) {
            return m;
        }
    }
    C40
}
fn values(c: u16, mode: usize, out: &mut Vec<usize>) {
    if c == FNC1 {
        out.extend([1, 27]);
    } else if c >= 128 {
        out.extend([1, 30]);
        values(c - 128, mode, out);
    } else if c <= 31 {
        out.extend([0, c as usize]);
    } else if c == 32 {
        out.push(3);
    } else if (48..=57).contains(&c) {
        out.push(c as usize - 48 + 4);
    } else if mode == C40 && (65..=90).contains(&c) {
        out.push(c as usize - 65 + 14);
    } else if mode == TEXT && (97..=122).contains(&c) {
        out.push(c as usize - 97 + 14);
    } else if c <= 47 {
        out.extend([1, c as usize - 33]);
    } else if (58..=64).contains(&c) {
        out.extend([1, c as usize - 58 + 15]);
    } else if (91..=95).contains(&c) {
        out.extend([1, c as usize - 91 + 22]);
    } else if mode == TEXT && (65..=90).contains(&c) {
        out.extend([2, c as usize - 65 + 1]);
    } else {
        out.extend([2, c as usize - 96]);
    }
}
fn triple(out: &mut Vec<usize>, values: &[usize]) {
    for v in values.as_chunks::<3>().0 {
        let word = 1600 * v[0] + 40 * v[1] + v[2] + 1;
        out.extend([word / 256, word % 256]);
    }
}
fn ascii(data: &[u16], i: &mut usize, out: &mut Vec<usize>) {
    let c = data[*i];
    if c == FNC1 {
        out.push(232);
    } else if (48..=57).contains(&c) && data.get(*i + 1).is_some_and(|v| (48..=57).contains(v)) {
        out.push(130 + ((c - 48) * 10 + data[*i + 1] - 48) as usize);
        *i += 1;
    } else if c >= 128 {
        out.extend([235, c as usize - 127]);
    } else {
        out.push(c as usize + 1);
    }
    *i += 1;
}
fn randomize(out: &mut Vec<usize>, value: usize) {
    out.push((value + (149 * (out.len() + 1)) % 255 + 1) % 256);
}

pub(super) fn encode(
    data: &[u16],
    capacity: impl Fn(usize) -> Option<usize>,
    printer_edifact: bool,
) -> Result<Vec<usize>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if data[i] == FNC1
            || ((48..=57).contains(&data[i])
                && data.get(i + 1).is_some_and(|c| (48..=57).contains(c)))
        {
            ascii(data, &mut i, &mut out);
            continue;
        }
        let mut mode = look_ahead(&data[i..], ASCII);
        if matches!(mode, X12 | EDF) && !native(data[i], mode) {
            mode = ASCII;
        }
        match mode {
            ASCII => ascii(data, &mut i, &mut out),
            C40 | TEXT => {
                out.push(if mode == C40 { 230 } else { 239 });
                let start = i;
                let mut v = Vec::new();
                let mut lengths = Vec::new();
                while i < data.len() {
                    let before = v.len();
                    values(data[i], mode, &mut v);
                    lengths.push(v.len() - before);
                    i += 1;
                    if v.len().is_multiple_of(3) && look_ahead(&data[i..], mode) != mode {
                        break;
                    }
                }
                let full = out.len() + v.len() / 3 * 2;
                if i == data.len() && v.len() % 3 == 2 && capacity(full + 2) == Some(full + 2) {
                    v.push(0);
                    triple(&mut out, &v);
                    continue;
                }
                if i == data.len()
                    && v.len() % 3 == 1
                    && lengths.last() == Some(&1)
                    && capacity(full + 1) == Some(full + 1)
                {
                    v.pop();
                    triple(&mut out, &v);
                    i -= 1;
                    ascii(data, &mut i, &mut out);
                    continue;
                }
                while !v.len().is_multiple_of(3) {
                    let len = lengths.pop().unwrap();
                    v.truncate(v.len() - len);
                    i -= 1;
                }
                if i == start {
                    out.pop();
                    ascii(data, &mut i, &mut out);
                    continue;
                }
                triple(&mut out, &v);
                // A single remaining symbol codeword is implicitly ASCII
                // (§5.2.5.2); use the normal ASCII pad in that final slot.
                if i < data.len() || !capacity(out.len()).is_some_and(|cap| cap <= out.len() + 1) {
                    out.push(254);
                }
            }
            X12 => {
                out.push(238);
                let mut v = Vec::new();
                while i < data.len() && native(data[i], X12) {
                    let c = data[i];
                    v.push(match c {
                        13 => 0,
                        42 => 1,
                        62 => 2,
                        32 => 3,
                        48..=57 => c as usize - 48 + 4,
                        _ => c as usize - 65 + 14,
                    });
                    i += 1;
                    if v.len().is_multiple_of(3) && look_ahead(&data[i..], X12) != X12 {
                        break;
                    }
                }
                let tail = v.len() % 3;
                i -= tail;
                v.truncate(v.len() - tail);
                triple(&mut out, &v);
                let remaining = data.len() - i;
                if !(remaining == 0 && capacity(out.len()) == Some(out.len())
                    || remaining == 1 && capacity(out.len() + 1) == Some(out.len() + 1))
                {
                    out.push(254);
                }
                let end = i + tail;
                while i < end {
                    ascii(data, &mut i, &mut out);
                }
            }
            EDF => {
                out.push(240);
                let start = i;
                while i < data.len() && native(data[i], EDF) {
                    i += 1;
                    if ((printer_edifact && (i - start) % 4 == 3) || (i - start).is_multiple_of(4))
                        && look_ahead(&data[i..], EDF) != EDF
                    {
                        let tail = &data[i..];
                        let mut ascii_words = Vec::new();
                        let mut pos = 0;
                        if tail.len() <= 3 {
                            while pos < tail.len() {
                                ascii(tail, &mut pos, &mut ascii_words);
                            }
                        }
                        if !printer_edifact
                            || tail.len() > 3
                            || !tail.iter().all(|&c| native(c, EDF))
                            || ascii_words.len() + (((i - start) % 4 + 1) * 6).div_ceil(8)
                                < (((i - start) % 4 + tail.len() + 1) * 6).div_ceil(8)
                        {
                            break;
                        }
                    }
                }
                let full = (i - start) / 4 * 4;
                for group in data[start..start + full].as_chunks::<4>().0 {
                    let word = group
                        .iter()
                        .fold(0usize, |n, &c| (n << 6) | (c as usize & 63));
                    out.extend([(word >> 16) & 255, (word >> 8) & 255, word & 255]);
                }
                let tail = i - start - full;
                let end = i == data.len();
                if end
                    && tail <= 2
                    && capacity(out.len() + tail).is_some_and(|cap| cap - out.len() <= 2)
                {
                    i -= tail;
                    while i < data.len() {
                        ascii(data, &mut i, &mut out);
                    }
                } else {
                    let mut word = 0usize;
                    for &c in &data[start + full..i] {
                        word = (word << 6) | (c as usize & 63);
                    }
                    word = (word << 6) | 31;
                    let bits = (tail + 1) * 6;
                    let bytes = bits.div_ceil(8);
                    word <<= bytes * 8 - bits;
                    for k in (0..bytes).rev() {
                        out.push((word >> (8 * k)) & 255);
                    }
                }
            }
            B256 => {
                let start = i;
                while i < data.len() && data[i] != FNC1 {
                    i += 1;
                    if look_ahead(&data[i..], B256) != B256 {
                        break;
                    }
                }
                let len = i - start;
                if len == 0 || len > 1555 {
                    return Err("Data Matrix binary segment capacity exceeded".into());
                }
                out.push(231);
                if i == data.len() && capacity(out.len() + 1 + len) == Some(out.len() + 1 + len) {
                    randomize(&mut out, 0);
                } else if len <= 249 {
                    randomize(&mut out, len);
                } else {
                    randomize(&mut out, len / 250 + 249);
                    randomize(&mut out, len % 250);
                }
                for &c in &data[start..i] {
                    randomize(&mut out, c as usize);
                }
            }
            _ => unreachable!(),
        }
        if capacity(out.len()).is_none() {
            return Err("Data Matrix data does not fit requested dimensions".into());
        }
    }
    Ok(out)
}
