//! Component-level independent decoder adapters, not a complete GS1 scanner.
use super::*;
use crate::render::profiles::SPECIFICATION;
use rxing::oned::rss::expanded::decoders::DecodedObject;
use rxing::oned::OneDReader;

fn words(matrix: &Matrix, starts: &[usize], ec: usize) -> Vec<u32> {
    let mut words = Vec::new();
    for y in 0..matrix.h {
        for &start in starts {
            let pattern =
                (start..start + 17).fold(0, |v, x| (v << 1) | u32::from(matrix.get(x, y)));
            let word = rxing::pdf417::pdf_417_common::getCodeword(pattern);
            assert!(word >= 0);
            words.push(word as u32);
        }
    }
    let original = words.clone();
    assert_eq!(
        rxing::pdf417::decoder::ec::error_correction::decode(&mut words, ec as u32, &mut [])
            .unwrap(),
        0
    );
    words[0] = (words[0] + 1) % 929;
    assert_eq!(
        rxing::pdf417::decoder::ec::error_correction::decode(&mut words, ec as u32, &mut [])
            .unwrap(),
        1
    );
    assert_eq!(words, original);
    words.truncate(words.len() - ec);
    words
}
fn general(bits: &[bool]) -> String {
    assert!(!bits[0], "general-purpose method 0");
    let mut array = rxing::common::BitArray::new();
    for &bit in bits {
        array.appendBit(bit);
    }
    // Use the bitstream decoder without its legacy AI-length validator, which
    // limits AI 91 to 30 characters. Preserve the FNC1 field boundaries.
    let mut output = String::new();
    let mut position = 1;
    let mut remaining = String::new();
    loop {
        // ISO/IEC 24723 Tables 6/7 define FNC1 as a Numeric latch. rxing
        // 0.9.2 returns the field but leaves its Alpha/ISO-646 state unchanged;
        // restart its decoder in Numeric state at each returned field boundary.
        let mut decoder = rxing::oned::rss::expanded::decoders::GeneralAppIdDecoder::new(&array);
        let info = decoder
            .decodeGeneralPurposeField(position, &remaining)
            .unwrap();
        if !info.getNewString().is_empty() {
            if !output.is_empty() {
                output.push('\u{1d}');
            }
            output.push_str(info.getNewString());
        }
        remaining = if info.isRemaining() {
            info.getRemainingValue().to_string()
        } else {
            String::new()
        };
        if position == info.getNewPosition() {
            break;
        }
        position = info.getNewPosition();
    }
    output
}
fn byte_bits(words: &[u32]) -> Vec<bool> {
    assert_eq!(words[0], 920);
    assert!(matches!(words[1], 901 | 924));
    // Strip the composite marker and explicitly select Latin-1 in this test
    // adapter so rxing returns binary bytes without character-set guessing.
    let mut ordinary = vec![(words.len() + 2) as u32, 927, 3];
    ordinary.extend_from_slice(&words[1..]);
    let decoded = rxing::pdf417::decoder::decoded_bit_stream_parser::decode(&ordinary, "").unwrap();
    let mut bits = Vec::new();
    for c in decoded.getText().chars() {
        assert!((c as u32) <= 255);
        bits::push(&mut bits, c as usize, 8);
    }
    bits
}

#[test]
fn all_cc_a_versions() {
    for (version, capacity) in composite_a::CAPACITY.into_iter().enumerate() {
        let count = 2 * ((capacity - 1) / 7) - 2;
        let data = format!("91{}", "1".repeat(count));
        let bits = gs1_compaction::encode(data.as_bytes(), vec![false], |n| {
            (n <= capacity).then_some(capacity)
        })
        .unwrap();
        let matrix = composite_a::encode(&bits);
        assert_eq!((matrix.w, matrix.h), (99, version + 3));
        let codewords = words(&matrix, &[10, 27, 54, 71], version + 4);
        // Inverse radix conversion of the decoded codewords (69 bits / seven
        // base-928 digits). All symbol patterns and RS checks use rxing.
        let mut recovered = Vec::new();
        for (group, source) in codewords.chunks(7).zip(bits.chunks(69)) {
            let value = group.iter().fold(0u128, |v, &w| v * 928 + u128::from(w));
            recovered.extend((0..source.len()).rev().map(|i| value & (1u128 << i) != 0));
        }
        assert_eq!(recovered, bits);
        assert_eq!(general(&recovered), data);
    }
}

#[test]
fn all_cc_b_versions() {
    for mode in 23..34 {
        let ec = micropdf417::EC[mode];
        let capacity = 8 * composite_b::byte_capacity(4 * micropdf417::ROWS[mode] - ec - 2);
        let data = "91ABC123abc!?";
        let data = if capacity < 90 { "911234" } else { data };
        let bits = gs1_compaction::encode(data.as_bytes(), vec![false], |n| {
            (n <= capacity).then_some(capacity)
        })
        .unwrap();
        let matrix = composite_b::encode(&bits).unwrap();
        assert_eq!(matrix.h, micropdf417::ROWS[mode]);
        let codewords = words(&matrix, &[10, 27, 54, 71], ec);
        let recovered = byte_bits(&codewords);
        assert_eq!(recovered, bits);
        assert_eq!(general(&recovered), data);
        let mut sample = anyd::output::BitMatrix::new(matrix.w, matrix.h * 2, 1);
        for y in 0..matrix.h * 2 {
            for x in 0..matrix.w {
                sample.set(x, y, matrix.get(x, y / 2));
            }
        }
        // anyd validates MicroPDF417 RAPs and error correction, then rejects
        // the composite-specific control word (not supported by that decoder).
        let error = anyd::codes::pdf417::MicroPdf417Decoder::new()
            .decode_matrix(&sample)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unsupported PDF417 mode codeword"),
            "{error}"
        );
    }
}

#[test]
fn cc_c_sizes_and_payload() {
    for length in [1, 2, 5, 20, 60, 90, 150, 400, 900, 1700, 2300] {
        let data = format!("91{}", "1".repeat(length));
        // Deliberately oversized AI fields test binary capacity independently
        // of application-level AI validation, which remains caller-owned.
        for width in [145, 300, 700] {
            let Some(_) = composite_c::size(1 + data.len() * 7 / 2, width) else {
                continue;
            };
            let bits = gs1_compaction::encode(data.as_bytes(), vec![false], |n| {
                composite_c::size(n, width).map(|s| s.bits)
            })
            .unwrap();
            let size = composite_c::size(bits.len(), width).unwrap();
            let matrix = composite_c::encode(&bits, size);
            assert!(size.cols + 4 < 4 * size.rows);
            assert!(size.rows * size.cols <= 928);
            let starts: Vec<_> = (0..size.cols).map(|x| 34 + x * 17).collect();
            let codewords = words(&matrix, &starts, 1 << (size.ec + 1));
            assert_eq!(codewords[0] as usize, codewords.len());
            let recovered = byte_bits(&codewords[1..]);
            assert_eq!(recovered, bits);
            assert_eq!(general(&recovered), data);
        }
    }
}

#[test]
fn gs1_128_linkage() {
    for cc_c in [false, true] {
        for (data, end_c) in [
            ("0103212345678906", true),
            ("010321234567890610ABC", false),
            ("010321234567890610ABC\u{1d}211234", true),
        ] {
            let (bits, _) = gs1_128::encode(data.as_bytes(), cc_c).unwrap();
            let mut array = rxing::common::BitArray::new();
            for bit in std::iter::repeat_n(false, 10)
                .chain(bits)
                .chain(std::iter::repeat_n(false, 10))
            {
                array.appendBit(bit);
            }
            let hints = rxing::DecodeHints {
                AssumeGs1: Some(true),
                ..Default::default()
            };
            let decoded = rxing::oned::Code128Reader
                .decode_row(0, &array, &hints)
                .unwrap();
            assert_eq!(decoded.getText(), format!("]C1{data}"));
            let raw = decoded.getRawBytes();
            assert_eq!(raw[1], 102);
            let expected = match (cc_c, end_c) {
                (false, true) => 101,
                (false, false) => 99,
                (true, true) => 100,
                (true, false) => 101,
            };
            assert_eq!(raw[raw.len() - 3], expected);
        }
    }
}

#[test]
fn rendered_composite_components_and_alignment() {
    use crate::{output::raster::rasterize, render};
    for (variant, payload, rows, ec, width, capacity) in [
        (11, "10ABC".to_owned(), 3, 4, 99, 78),
        (11, format!("91{}", "A".repeat(50)), 15, 21, 99, 352),
        (12, "10ABC".to_owned(), 3, 8, 154, 32),
    ] {
        for module in [2, 3] {
            for separator in [1, 2] {
                let zpl=format!("^XA^PW1000^LL1000^FO40,40^BRN,{variant},{module},{separator},60^FD0103212345678906|{payload}^FS^XZ");
                let doc = render(zpl.as_bytes(), SPECIFICATION).unwrap();
                let raster = rasterize(&doc.labels[0]).unwrap();
                let pixel = |x: usize, y: usize| raster.pixels[y * raster.width as usize + x] < 128;
                let (linear_x, cc_x, row_height) = if variant == 12 {
                    (40 + 7 * module, 40, 3 * module)
                } else {
                    (40, 40 + 21 * module, 2 * module)
                };
                let mut sampled = Matrix::new(width, rows);
                for y in 0..rows {
                    for x in 0..width {
                        sampled.set(
                            x,
                            y,
                            pixel(
                                cc_x + x * module + module / 2,
                                40 + y * row_height + row_height / 2,
                            ),
                        );
                    }
                }
                let starts = if variant == 12 {
                    (0..5).map(|x| 34 + 17 * x).collect()
                } else {
                    vec![10, 27, 54, 71]
                };
                let codewords = words(&sampled, &starts, ec);
                let recovered = if variant == 12 {
                    byte_bits(&codewords[1..])
                } else if rows == 3 {
                    let mut bits = Vec::new();
                    let mut remaining = capacity;
                    for group in codewords.chunks(7) {
                        let n = remaining.min(69);
                        remaining -= n;
                        let value = group.iter().fold(0u128, |v, &w| v * 928 + u128::from(w));
                        bits.extend((0..n).rev().map(|i| value & (1u128 << i) != 0));
                    }
                    bits
                } else {
                    byte_bits(&codewords)
                };
                assert_eq!(recovered.len(), capacity);
                assert_eq!(general(&recovered), payload);
                let sep_y = 40 + rows * row_height;
                let linear_y = sep_y + separator * module + 30;
                let mut scan = rxing::common::BitArray::new();
                for x in 0..1000 {
                    scan.appendBit(pixel(x, linear_y));
                }
                let hints = rxing::DecodeHints {
                    AssumeGs1: Some(true),
                    ..Default::default()
                };
                let decoded = rxing::oned::Code128Reader
                    .decode_row(0, &scan, &hints)
                    .unwrap();
                assert_eq!(decoded.getText(), "]C10103212345678906");
                let raw = decoded.getRawBytes();
                assert_eq!(raw[raw.len() - 3], if variant == 12 { 100 } else { 101 });
                for y in sep_y..sep_y + separator * module {
                    for x in 0..1000 {
                        let expected =
                            (linear_x..linear_x + 145 * module).contains(&x) && !pixel(x, linear_y);
                        assert_eq!(pixel(x, y), expected, "separator at {x},{y}");
                    }
                }
            }
        }
    }
}

#[test]
fn composite_input_errors() {
    use crate::render;
    for variant in [11, 12] {
        for data in [
            "0103212345678906",
            "|10ABC",
            "0103212345678906|",
            "1|10ABC",
            "0103212345678906|10@",
        ] {
            let zpl = format!("^XA^BRN,{variant}^FD{data}^FS^XZ");
            assert!(render(zpl.as_bytes(), SPECIFICATION).is_err(), "{data}");
        }
        let zpl = format!(
            "^XA^BRN,{variant}^FD0103212345678906|91{}^FS^XZ",
            "A".repeat(2400)
        );
        assert!(render(zpl.as_bytes(), SPECIFICATION).is_err());
    }
}

#[test]
fn composite_compaction_modes_and_terminal_digits() {
    for data in [
        "10ABC\u{1d}2112345",
        "10abc!\u{1d}21123456",
        "10ABC1234567890abcABCDEF12345",
        "91!\"%&'()*+,-./:;<=>?_ ",
    ] {
        for cc_c in [false, true] {
            let bits = gs1_compaction::encode(data.as_bytes(), vec![false], |n| {
                if cc_c {
                    composite_c::size(n, 145).map(|s| s.bits)
                } else {
                    composite_a::capacity(n).or_else(|| composite_b::capacity(n))
                }
            })
            .unwrap();
            assert_eq!(general(&bits), data);
        }
    }
    for length in 1..=330 {
        let data = format!("91{}", &"1234567890".repeat(33)[..length]);
        let bits = gs1_compaction::encode(data.as_bytes(), vec![false], |n| {
            composite_a::capacity(n).or_else(|| composite_b::capacity(n))
        })
        .unwrap();
        assert_eq!(general(&bits), data, "length {length}");
    }
}
