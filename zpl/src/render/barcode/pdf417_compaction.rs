//! Original PDF417/MicroPDF417 high-level compaction.
//! USS PDF417 §§2.2.4.4–6, Appendix D.
//! <https://www.expresscorp.com/wp-content/uploads/2023/02/USS-PDF-417.pdf>
//! Numeric runs of eight or more reproduce the observed ZD621 threshold.
//! Text submode choices are deterministic, not a minimum-codeword optimizer.
//! MicroPDF417 mode initialization: ISO/IEC 24728 §5.4.
//! <https://previewnorm.com/iec/ISO%20IEC%2024728-2006%20PDF.pdf>
const MIXED: &[u8] = b"0123456789&\r\t,:#-.$/+%*=^";
const PUNCT: &[u8] = b";<>@[\\]_`~!\r\t,:\n-.$/\"|*()?{}'";

#[derive(Clone, Copy)]
enum Text {
    Alpha,
    Lower,
    Mixed,
}
fn text_byte(c: u8) -> bool {
    c.is_ascii_graphic() || matches!(c, b' ' | b'\t' | b'\r' | b'\n')
}
fn digits(data: &[u8]) -> usize {
    data.iter().take_while(|c| c.is_ascii_digit()).count()
}
fn text_run(data: &[u8], threshold: usize) -> usize {
    (0..data.len())
        .find(|&i| !text_byte(data[i]) || digits(&data[i..]) >= threshold)
        .unwrap_or(data.len())
}
fn text(data: &[u8], state: &mut Text, out: &mut Vec<usize>, micro: bool) {
    let mut values = Vec::new();
    for (i, &c) in data.iter().enumerate() {
        loop {
            if c == b' ' {
                values.push(26);
                break;
            }
            match *state {
                Text::Alpha if c.is_ascii_uppercase() => {
                    values.push((c - b'A') as usize);
                    break;
                }
                Text::Lower if c.is_ascii_lowercase() => {
                    values.push((c - b'a') as usize);
                    break;
                }
                Text::Mixed if MIXED.contains(&c) => {
                    values.push(MIXED.iter().position(|&v| v == c).unwrap());
                    break;
                }
                Text::Alpha if c.is_ascii_lowercase() => {
                    values.push(27);
                    *state = Text::Lower;
                }
                Text::Lower if c.is_ascii_uppercase() => {
                    if micro && data.get(i + 1).is_some_and(u8::is_ascii_uppercase) {
                        values.extend([28, 28]);
                        *state = Text::Alpha;
                        continue;
                    }
                    values.extend([27, (c - b'A') as usize]);
                    break;
                }
                Text::Mixed if c.is_ascii_uppercase() => {
                    values.push(28);
                    *state = Text::Alpha;
                }
                Text::Mixed if c.is_ascii_lowercase() => {
                    values.push(27);
                    *state = Text::Lower;
                }
                _ => {
                    if let Some(v) = PUNCT.iter().position(|&v| v == c) {
                        values.extend([29, v]);
                        break;
                    }
                    values.push(28);
                    *state = Text::Mixed;
                }
            }
        }
    }
    if values.len() % 2 != 0 {
        values.push(29);
    }
    out.extend(values.as_chunks::<2>().0.iter().map(|v| 30 * v[0] + v[1]));
}
fn numeric(data: &[u8], out: &mut Vec<usize>) {
    for group in data.chunks(44) {
        let mut decimal: Vec<usize> = std::iter::once(1)
            .chain(group.iter().map(|c| (c - b'0') as usize))
            .collect();
        let mut reversed = Vec::new();
        while !decimal.is_empty() {
            let mut remainder = 0;
            let mut quotient = Vec::new();
            for d in decimal {
                let v = remainder * 10 + d;
                if v / 900 != 0 || !quotient.is_empty() {
                    quotient.push(v / 900);
                }
                remainder = v % 900;
            }
            reversed.push(remainder);
            decimal = quotient;
        }
        out.extend(reversed.into_iter().rev());
    }
}
pub(super) fn encode(data: &[u8]) -> Vec<usize> {
    encode_initial(data, true)
}

// ISO/IEC 24728 §5.4: MicroPDF417 starts in Byte, not Text mode.
pub(super) fn encode_micro(data: &[u8]) -> Vec<usize> {
    encode_initial(data, false)
}

fn encode_initial(data: &[u8], mut in_text: bool) -> Vec<usize> {
    let micro = !in_text;
    let threshold = if micro { 13 } else { 8 };
    let mut out = Vec::new();
    if micro && !data.is_empty() && data.iter().all(u8::is_ascii_digit) {
        out.push(902);
        numeric(data, &mut out);
        return out;
    }
    let mut pos = 0;
    let mut state = Text::Alpha;
    while pos < data.len() {
        let remaining = &data[pos..];
        let n = digits(remaining);
        if n >= threshold {
            out.push(902);
            numeric(&remaining[..n], &mut out);
            in_text = false;
            pos += n;
            continue;
        }
        let n = text_run(remaining, threshold);
        if n >= 5 || n == remaining.len() {
            if !in_text {
                out.push(900);
                state = Text::Alpha;
            }
            text(&remaining[..n], &mut state, &mut out, micro);
            in_text = true;
            pos += n;
            continue;
        }
        let mut n = 1;
        while n < remaining.len()
            && digits(&remaining[n..]) < threshold
            && text_run(&remaining[n..], threshold) < 5
        {
            n += 1;
        }
        if n == 1 && in_text {
            out.extend([913, remaining[0] as usize]);
        } else {
            out.extend(super::compact(&remaining[..n]));
            in_text = false;
        }
        pos += n;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn micro_initial_modes_and_byte_shift() {
        assert_eq!(encode_micro(b"A"), [900, 29]);
        assert_eq!(encode_micro(b"1"), [902, 11]);
        assert_eq!(encode_micro(b"\x80"), [901, 128]);
        let data = b"Hello\x80WORLD12345678end";
        let encoded = encode_micro(data);
        assert_eq!(
            encoded,
            [900, 237, 131, 344, 913, 128, 868, 674, 521, 118, 32, 94, 156, 218, 814, 393]
        );
        // anyd's Micro decoder does not handle 913; independently check the
        // shared ISO high-level stream with rxing's PDF417 bitstream decoder.
        let mut words = vec![(encoded.len() + 4) as u32, 927, 3];
        words.extend(encoded.iter().map(|&v| v as u32));
        words.push(900);
        let decoded =
            rxing::pdf417::decoder::decoded_bit_stream_parser::decode(&words, "").unwrap();
        assert_eq!(decoded.getText(), "Hello\u{80}WORLD12345678end");
    }
    #[test]
    fn printer_codewords() {
        for (data, expected) in [
            (
                b"Hello PDF417".as_slice(),
                vec![237, 131, 344, 807, 477, 117, 178, 121, 239],
            ),
            (b"ABCDEF", vec![1, 63, 125]),
            (b"ABC\r\nDEF", vec![1, 89, 359, 453, 125]),
            (b"1234567", vec![841, 63, 125, 187]),
            (b"12345678", vec![902, 138, 628, 478]),
            (b"1234567890123", vec![902, 17, 110, 836, 811, 223]),
            (b"\x01\x02\x03\x04\x05\x06", vec![924, 1, 620, 89, 74, 846]),
            (
                b"\x01\x02\x03\x04\x05\x06\x07",
                vec![901, 1, 620, 89, 74, 846, 7],
            ),
        ] {
            assert_eq!(encode(data), expected, "{data:?}");
        }
    }

    #[test]
    fn independent_decoder_transitions_and_numeric_groups() {
        fn check(data: &[u8]) {
            let encoded = encode(data);
            // Explicit Latin-1 in the decoder adapter avoids charset guessing;
            // it is not inserted by the production encoder.
            let mut words = vec![(encoded.len() + 4) as u32, 927, 3];
            words.extend(encoded.iter().map(|&v| v as u32));
            words.push(900);
            let decoded =
                rxing::pdf417::decoder::decoded_bit_stream_parser::decode(&words, "").unwrap();
            let expected: String = data.iter().map(|&c| char::from(c)).collect();
            assert_eq!(decoded.getText(), expected, "{data:?}: {encoded:?}");
        }
        for prefix in ["A", "AB", "abc", "abcd", "1234567", "12345678"] {
            for byte in [0, 1, 128, 255] {
                let mut data = prefix.as_bytes().to_vec();
                data.push(byte);
                data.extend_from_slice(b"HELLOworld1234567890123END");
                check(&data);
            }
        }
        for length in [8, 43, 44, 45, 87, 88, 89, 132] {
            check(&vec![b'0'; length]);
            check(
                &(0..length)
                    .map(|i| b'0' + (i % 10) as u8)
                    .collect::<Vec<_>>(),
            );
        }
        let mut seed = 0x417u32;
        let alphabet =
            b"ABCXYZabcxyz0123456789 ;<>@[\\]_`~!\r\t,:\n-.$/\"|*()?{}'&+=^\x00\x01\x80\xff";
        for length in 1..=256 {
            let data: Vec<_> = (0..length)
                .map(|_| {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    alphabet[(seed >> 16) as usize % alphabet.len()]
                })
                .collect();
            check(&data);
        }
    }
}
