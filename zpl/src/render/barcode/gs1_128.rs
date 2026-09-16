//! Original GS1-128 composite linkage, ISO/IEC 24723:2010 §7.4, Table 8.
//! <https://www.iso.org/standard/51425.html>
use super::*;

pub(super) fn encode(data: &[u8], cc_c: bool) -> Result<(Vec<bool>, usize), String> {
    if data.is_empty()
        || data.len() > 200
        || !data.iter().all(|&c| c == 29 || (32..=126).contains(&c))
    {
        return Err(
            "GS1-128 requires ASCII data with GS/FNC1 separators (maximum 200 bytes)".into(),
        );
    }
    let mut c_set = data.iter().take_while(|b| b.is_ascii_digit()).count() >= 4;
    let mut words = vec![if c_set { 105 } else { 104 }, 102];
    let mut i = 0;
    while i < data.len() {
        if data[i] == 29 {
            words.push(102);
            i += 1;
            continue;
        }
        let digits = data[i..].iter().take_while(|b| b.is_ascii_digit()).count();
        if c_set && digits >= 2 {
            words.push(((data[i] - b'0') * 10 + data[i + 1] - b'0') as usize);
            i += 2;
        } else if c_set {
            words.push(100);
            c_set = false;
        } else if digits >= 4 {
            words.push(99);
            c_set = true;
        } else {
            words.push((data[i] - 32) as usize);
            i += 1;
        }
    }
    words.push(match (cc_c, c_set) {
        (false, false) => 99,
        (false, true) => 101,
        (true, false) => 101,
        (true, true) => 100,
    });
    let check = (words[0]
        + words[1..]
            .iter()
            .enumerate()
            .map(|(i, &v)| (i + 1) * v)
            .sum::<usize>())
        % 103;
    words.extend([check, 106]);
    if words.len() < 9 {
        return Err("GS1-128 composite needs at least nine symbol characters".into());
    }
    // §12.3(g): target last space module counted back from the Stop character.
    let target = words.len() - 1 - (words.len() - 9) / 2;
    let target_last_space = target * 11 + 10;
    let mut bits = Vec::new();
    for word in words {
        runs(
            &mut bits,
            &code128::WIDTHS[word]
                .bytes()
                .map(|b| (b - b'0') as usize)
                .collect::<Vec<_>>(),
        );
    }
    Ok((bits, target_last_space))
}
