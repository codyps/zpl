//! Independent decoder tests: runtime encoders never link rxing.
use rxing::BarcodeFormat;
use zpl::{output::raster::rasterize, render, Options};
fn decode(command: &str, payload: &str, expected: &str, format: BarcodeFormat) {
    let zpl = format!("^XA^PW1600^LL1000^FO50,50^BY3,2,90^{command}^FD{payload}^FS^XZ");
    let doc = render(zpl.as_bytes(), Options::default())
        .unwrap_or_else(|e| panic!("{command}, {payload:?}: {e}"));
    let raster = rasterize(&doc.labels[0]).unwrap();
    let mut left = raster.width;
    let mut top = raster.height;
    let mut right = 0;
    let mut bottom = 0;
    for y in 0..raster.height {
        for x in 0..raster.width {
            if raster.pixels[(y * raster.width + x) as usize] == 0 {
                left = left.min(x);
                top = top.min(y);
                right = right.max(x);
                bottom = bottom.max(y);
            }
        }
    }
    let width = right - left + 81;
    let height = bottom - top + 81;
    let mut pixels = vec![255; (width * height) as usize];
    for y in top..=bottom {
        for x in left..=right {
            pixels[((y - top + 40) * width + x - left + 40) as usize] =
                raster.pixels[(y * raster.width + x) as usize];
        }
    }
    let found = rxing::helpers::detect_in_luma(pixels, width, height, Some(format))
        .unwrap_or_else(|e| panic!("{command}, {payload}: {e:?}"));
    assert_eq!(found.getText(), expected, "{command}");
}
#[test]
fn linear_round_trips() {
    for (command, data, expected, format) in [
        ("B8N,90,N,N", "1234567", "12345670", BarcodeFormat::EAN_8),
        (
            "BEN,90,N,N",
            "590123412345",
            "5901234123457",
            BarcodeFormat::EAN_13,
        ),
        (
            "BUN,90,N,N",
            "03600029145",
            "036000291452",
            BarcodeFormat::UPC_A,
        ),
        ("B9N,90,N,N", "042526", "00425261", BarcodeFormat::UPC_E),
        ("B2N,90,N,N,N", "12345678", "12345678", BarcodeFormat::ITF),
        (
            "BKN,N,90,N,N,A,B",
            "123456",
            "123456",
            BarcodeFormat::CODABAR,
        ),
        ("BAN,90,N,N", "Hello93!", "HELLO93", BarcodeFormat::CODE_93),
        ("BAN,90,N,N", ")A)B)C(A", "abc!", BarcodeFormat::CODE_93),
        ("B3N,N,90,N,N", "CODE39", "CODE39", BarcodeFormat::CODE_39),
        (
            "BCN,90,N,N,N,N",
            "Code128",
            "Code128",
            BarcodeFormat::CODE_128,
        ),
    ] {
        decode(command, data, expected, format);
    }
}
#[test]
fn qr_round_trips() {
    for level in ["L", "M", "Q", "H"] {
        for mask in 0..8 {
            decode(
                &format!("BQN,2,4,{level},{mask}"),
                &format!("{level}A,Hello QR 123"),
                "Hello QR 123",
                BarcodeFormat::QR_CODE,
            );
        }
    }
}
#[test]
fn large_matrix_round_trips() {
    for n in [80, 200, 500, 1000, 1500] {
        let data = "a".repeat(n);
        decode(
            "BQN,2,4,L,7",
            &format!("LA,{data}"),
            &data,
            BarcodeFormat::QR_CODE,
        );
        decode("BXN,4,200", &data, &data, BarcodeFormat::DATA_MATRIX);
        decode("BON,4", &data, &data, BarcodeFormat::AZTEC);
    }
}
#[test]
fn data_matrix_round_trips() {
    for data in ["A", "ABC123", "Hello Data Matrix 1234567890"] {
        decode("BXN,5,200", data, data, BarcodeFormat::DATA_MATRIX);
    }
}
#[test]
fn databar_round_trips() {
    for data in [
        "2001234567890",
        "0441234567890",
        "0000000000000",
        "9999999999999",
    ] {
        let check = zpl_gtin_check(data);
        decode(
            "BRN,1,3",
            data,
            &format!("{data}{check}"),
            BarcodeFormat::RSS_14,
        );
    }
}
fn zpl_gtin_check(data: &str) -> usize {
    (10 - data
        .bytes()
        .rev()
        .enumerate()
        .map(|(i, c)| (c - b'0') as usize * if i % 2 == 0 { 3 } else { 1 })
        .sum::<usize>()
        % 10)
        % 10
}

#[test]
fn new_databar_round_trips() {
    use anyd::traits::Decode;
    for variant in [3, 4] {
        for data in [
            "2001234567890",
            "0441234567890",
            "0000000000000",
            "9999999999999",
        ] {
            decode(
                &format!("BRN,{variant},3"),
                data,
                &format!("{data}{}", zpl_gtin_check(data)),
                BarcodeFormat::RSS_14,
            );
        }
    }
    for data in ["0000000000000", "1001234567890", "1999999999999"] {
        let matrix = sampled("BRN,5,3", data, 79, 1, 3, 3);
        let modules = (0..79).map(|x| matrix.get(x, 0)).collect();
        let encoding = anyd::output::Encoding::Linear(anyd::output::LinearPattern {
            modules,
            quiet_zone: 10,
        });
        let result = anyd::codes::databar::DataBarDecoder::new()
            .decode(&encoding)
            .unwrap();
        assert_eq!(
            result.text().unwrap(),
            format!("{data}{}", zpl_gtin_check(data))
        );
    }
    for segments in [2, 4, 6, 8, 22] {
        for (data, expected) in [
            ("0100012345678905", "(01)00012345678905"),
            ("1012345678901234567890", "(10)12345678901234567890"),
            ("10ABC123abc!?", "(10)ABC123abc!?"),
            (
                "01950123456789033103000123",
                "(01)95012345678903(3103)000123",
            ),
        ] {
            decode(
                &format!("BRN,6,3,1,25,{segments}"),
                data,
                expected,
                BarcodeFormat::RSS_EXPANDED,
            );
        }
    }
}

#[test]
fn databar_padding_and_group_boundaries() {
    use anyd::traits::Decode;
    // rxing's legacy AI validator limits AI 91 to 30 characters; independently
    // decode this longer rendered module row with anyd (including its checksum).
    let data = "911234567890123456789012345678901";
    let m = sampled("BRN,6,3,1,25,20", data, 298, 1, 3, 3);
    let enc = anyd::output::Encoding::Linear(anyd::output::LinearPattern {
        modules: (0..298).map(|x| m.get(x, 0)).collect(),
        quiet_zone: 10,
    });
    let decoded = anyd::codes::databar::DataBarDecoder::new()
        .decode(&enc)
        .unwrap();
    assert_eq!(decoded.text().unwrap(), data);
    for value in [
        183063u64, 183064, 820063, 820064, 1000775, 1000776, 1491020, 1491021, 1979844, 1979845,
        1996938, 1996939, 2013570,
    ] {
        let data = format!("{value:013}");
        let matrix = sampled("BRN,5,3", &data, 79, 1, 3, 3);
        let encoding = anyd::output::Encoding::Linear(anyd::output::LinearPattern {
            modules: (0..79).map(|x| matrix.get(x, 0)).collect(),
            quiet_zone: 10,
        });
        let decoded = anyd::codes::databar::DataBarDecoder::new()
            .decode(&encoding)
            .unwrap();
        assert_eq!(
            decoded.text().unwrap(),
            format!("{data}{}", zpl_gtin_check(&data))
        );
    }
    for data in [
        "a!\"%&'()*+,-./:;<=>?_ ",
        "AAAAAA1234567aABCDE1234",
        "ABC123DEF456ghi789",
    ] {
        decode(
            "BRN,6,3,2,25,4",
            &format!("91{data}"),
            &format!("(91){data}"),
            BarcodeFormat::RSS_EXPANDED,
        );
    }
    let m = sampled("BRN,6,3,1,25,22^FH", "10ABC_1D2112345", 183, 1, 3, 3);
    let enc = anyd::output::Encoding::Linear(anyd::output::LinearPattern {
        modules: (0..183).map(|x| m.get(x, 0)).collect(),
        quiet_zone: 10,
    });
    let decoded = anyd::codes::databar::DataBarDecoder::new()
        .decode(&enc)
        .unwrap();
    assert_eq!(decoded.text().unwrap(), "10ABC\u{1d}2112345");
}

#[test]
fn new_barcode_input_errors() {
    for (command, data) in [
        ("BRN,5", "2000000000000"),
        ("BRN,5", "10012345678909"),
        ("BRN,3", "123"),
        ("BRN,6,3,1,25,3", "10ABC"),
        ("BRN,6", "10@"),
        ("BT", "123456,"),
        ("BT", "123456,ABC,,DEF"),
        ("BT", "123456,ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
    ] {
        assert!(
            render(
                format!("^XA^{command}^FD{data}^FS^XZ").as_bytes(),
                Options::default()
            )
            .is_err(),
            "{command}, {data}"
        );
    }
    let overlong = format!("^XA^BRN,6^FD91{}^FS^XZ", "1".repeat(100));
    assert!(render(overlong.as_bytes(), Options::default()).is_err());
}
#[test]
fn codabar_alphabet() {
    decode(
        "BKN,N,90,N,N,C,D",
        "0123456789-$:/.+",
        "0123456789-$:/.+",
        BarcodeFormat::CODABAR,
    );
}

#[test]
fn postal_round_trips() {
    use anyd::traits::Decode;
    for (command, data, imb) in [
        ("BZN,90,N,N,0", "12345", false),
        ("BZN,90,N,N,0", "123456789", false),
        ("BZN,90,N,N,1", "12345678901", false),
        ("B5N,90,N,N", "1234567890123", false),
        ("BZN,90,N,N,3", "0027012345620080000198765432101", true),
        ("BZN,90,N,N,3", "00270123456200800001", true),
    ] {
        let label = format!("^XA^PW1200^LL200^FO20,20^BY3,2^{command}^FD{data}^FS^XZ");
        let doc = render(label.as_bytes(), Options::default()).unwrap();
        let r = rasterize(&doc.labels[0]).unwrap();
        let bars = if imb { 65 } else { (data.len() + 1) * 5 + 2 };
        let mut matrix = anyd::output::BitMatrix::new(bars * 2 - 1, 3, 0);
        for i in 0..bars {
            let x = 21 + i * 9;
            for (row, y) in if imb { [35, 65, 95] } else { [35, 95, 115] }
                .into_iter()
                .enumerate()
            {
                matrix.set(i * 2, row, r.pixels[y * r.width as usize + x] < 128);
            }
        }
        let decoded = anyd::codes::postal::PostalDecoder::new()
            .decode(&anyd::output::Encoding::Matrix(matrix))
            .unwrap_or_else(|e| panic!("{command}: {e}"));
        assert_eq!(decoded.text().unwrap(), data, "{command}");
    }
}

#[test]
fn matrix_dimensions_and_pdf_ecc() {
    for (w, h) in [(18, 8), (32, 8), (26, 12), (36, 12), (36, 16), (48, 16)] {
        decode(
            &format!("BXN,5,200,{w},{h},6,_,2"),
            "AB12",
            "AB12",
            BarcodeFormat::DATA_MATRIX,
        );
    }
    for level in 5..=8 {
        decode(
            &format!("B7N,9,{level},10"),
            "Hello PDF417",
            "Hello PDF417",
            BarcodeFormat::PDF_417,
        );
    }
    for variant in 7..=10 {
        let (data, expected, format) = match variant {
            7 => ("03600029145", "036000291452", BarcodeFormat::UPC_A),
            8 => ("042526", "00425261", BarcodeFormat::UPC_E),
            9 => ("590123412345", "5901234123457", BarcodeFormat::EAN_13),
            _ => ("1234567", "12345670", BarcodeFormat::EAN_8),
        };
        decode(&format!("BRN,{variant},3,1,90"), data, expected, format);
    }
}

#[test]
fn unsupported_variants_are_explicit() {
    for command in ["BQN,1", "BXN,4,0", "BBN,8,Y,10,2,A"] {
        let label = format!("^XA^{command}^FD123456,ABC^FS^XZ");
        assert!(
            render(label.as_bytes(), Options::default()).is_err(),
            "{command}"
        );
    }
}

#[test]
fn field_hex_ascii_and_barcode_state() {
    let text = "\0\x01!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~\x7f";
    let encoded: String = text.bytes().map(|v| format!("_{v:02X}")).collect();
    for (field, expected) in [("_29A_28A_26A", "a!\x01"), ("abc", "ABC"), ("A!b?C", "ABC")] {
        decode("BAN,90,N,N^FH", field, expected, BarcodeFormat::CODE_93);
    }
    decode(
        "BQN,2,4,L,0^FH",
        &format!("LA,{encoded}"),
        text,
        BarcodeFormat::QR_CODE,
    );
    decode("BON,4^FH", &encoded, text, BarcodeFormat::AZTEC);
    decode(
        "BQN,2,4^BCN,90,N,N,N,N",
        "Code128",
        "Code128",
        BarcodeFormat::CODE_128,
    );
    decode(
        "BCN,90,N,N,N,N^BXN,5,200",
        "Matrix",
        "Matrix",
        BarcodeFormat::DATA_MATRIX,
    );
    decode(
        "BQN,2,4^B3N,N,90,N,N",
        "CODE39",
        "CODE39",
        BarcodeFormat::CODE_39,
    );
    for orientation in ["R", "I", "B"] {
        decode(
            &format!("BX{orientation},5,200"),
            "ROTATED",
            "ROTATED",
            BarcodeFormat::DATA_MATRIX,
        );
    }
}

#[test]
fn code93_zpl_full_ascii_substitutes() {
    for start in (0u8..128).step_by(8) {
        let data: Vec<_> = (start..start + 8).collect();
        let mut field = Vec::new();
        for &c in &data {
            if b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%".contains(&c) {
                field.push(c);
                continue;
            }
            let pair = match c {
                0 => [b'\'', b'U'],
                1..=26 => [b'&', b'A' + c - 1],
                27..=31 => [b'\'', b'A' + c - 27],
                33..=47 => [b'(', b'A' + c - 33],
                58 => [b'(', b'Z'],
                59..=63 => [b'\'', b'F' + c - 59],
                64 => [b'\'', b'V'],
                91..=95 => [b'\'', b'K' + c - 91],
                96 => [b'\'', b'W'],
                97..=122 => [b')', b'A' + c - 97],
                123..=127 => [b'\'', b'P' + c - 123],
                _ => unreachable!(),
            };
            field.extend(pair);
        }
        let escaped: String = field.iter().map(|v| format!("_{v:02X}")).collect();
        decode(
            "BAN,90,N,N^FH",
            &escaped,
            std::str::from_utf8(&data).unwrap(),
            BarcodeFormat::CODE_93,
        );
    }
}

fn linear_sample(command: &str, data: &str) -> anyd::output::Encoding {
    let label = format!("^XA^PW6000^LL200^FO20,20^BY3,2,90^{command}^FD{data}^FS^XZ");
    let doc = render(label.as_bytes(), Options::default()).unwrap();
    let r = rasterize(&doc.labels[0]).unwrap();
    let mut end = 20;
    for x in 20..r.width {
        if r.pixels[(65 * r.width + x) as usize] < 128 {
            end = x + 1;
        }
    }
    let modules = (20..end)
        .step_by(3)
        .map(|x| r.pixels[(65 * r.width + x + 1) as usize] < 128)
        .collect();
    anyd::output::Encoding::Linear(anyd::output::LinearPattern {
        modules,
        quiet_zone: 10,
    })
}
#[test]
fn independent_rare_linear_decoders() {
    use anyd::traits::Decode;
    for (mode, check) in [
        ("A", anyd::codes::msi::MsiCheck::None),
        ("B", anyd::codes::msi::MsiCheck::Mod10),
        ("C", anyd::codes::msi::MsiCheck::Mod1010),
        ("D", anyd::codes::msi::MsiCheck::Mod1110),
    ] {
        let m = linear_sample(&format!("BMN,{mode},90,N,N"), "1234567");
        let decoded = anyd::codes::msi::MsiDecoder::with_check(check)
            .decode(&m)
            .unwrap();
        assert_eq!(decoded.text().unwrap(), "1234567");
    }
    for flag in ["Y", "N"] {
        let mut m = linear_sample(&format!("B1N,{flag},90,N,N"), "123-45678");
        // anyd accepts only a binary narrow/wide pattern, not Code 11's
        // extra-wide bars. Normalize widths only in this decoder adapter;
        // exact printer comparisons independently check the physical geometry.
        let anyd::output::Encoding::Linear(ref mut pattern) = m else {
            unreachable!()
        };
        let mut normalized = Vec::new();
        let mut i = 0;
        while i < pattern.modules.len() {
            let start = i;
            let bit = pattern.modules[i];
            while i < pattern.modules.len() && pattern.modules[i] == bit {
                i += 1;
            }
            normalized.extend(std::iter::repeat_n(bit, (i - start).min(2)));
        }
        pattern.modules = normalized;
        let decoded = anyd::codes::code11::Code11Decoder::new()
            .with_check_count(if flag == "Y" { 1 } else { 2 })
            .decode(&m)
            .unwrap();
        assert_eq!(decoded.text().unwrap(), "123-45678");
    }
    for command in ["BIN,90,N,N", "BJN,90,N,N"] {
        let m = linear_sample(command, "1234567890");
        let decoded = anyd::codes::twoof5::TwoOf5Decoder::new()
            .decode(&m)
            .unwrap();
        assert_eq!(decoded.text().unwrap(), "1234567890");
    }
    let m = linear_sample("BPN,N,90,N,N", "123ABC");
    let decoded = anyd::codes::msi::MsiDecoder::plessey().decode(&m).unwrap();
    assert_eq!(decoded.text().unwrap(), "123ABC");
}

#[test]
fn retail_extensions_and_logmars() {
    use rxing::oned::OneDReader;
    for data in ["00", "01", "02", "03", "51234", "99999"] {
        let anyd::output::Encoding::Linear(pattern) = linear_sample("BSN,90,N,N", data) else {
            unreachable!()
        };
        let mut row = rxing::common::BitArray::default();
        for bit in pattern.modules {
            row.appendBit(bit);
        }
        let result = if data.len() == 2 {
            rxing::oned::UPCEANExtension2Support::default().decodeRow(0, &row, &[0, 4, 0])
        } else {
            rxing::oned::UPCEANExtension5Support.decodeRow(0, &row, &[0, 4])
        }
        .unwrap();
        assert_eq!(result.getText(), data);
    }
    let anyd::output::Encoding::Linear(pattern) = linear_sample("BLN,90,N", "LOGMARS") else {
        unreachable!()
    };
    let mut row = rxing::common::BitArray::default();
    for bit in std::iter::repeat_n(false, 10)
        .chain(pattern.modules)
        .chain(std::iter::repeat_n(false, 10))
    {
        row.appendBit(bit);
    }
    let result = rxing::oned::Code39Reader::default()
        .decode_row(0, &row, &Default::default())
        .unwrap();
    // LOGMARS always appends a modulo-43 check; the default Code 39 reader exposes it.
    assert_eq!(result.getText(), "LOGMARSJ");
}

#[test]
fn rare_stacked_raster_round_trips() {
    for (command, code49, w, h, row_height) in [
        ("B4N,8,N,A", true, 70, 3, 27),
        ("BBN,8,Y,8", false, 145, 2, 24),
    ] {
        let mut m = sampled(
            command,
            "HELLO WORLD",
            if code49 { w + 10 } else { w },
            h,
            3,
            row_height,
        );
        if code49 {
            let mut rows = anyd::output::BitMatrix::new(w, h, 0);
            for y in 0..h {
                for x in 0..w {
                    rows.set(x, y, m.get(x + 10, y));
                }
            }
            m = rows;
        }
        let decoded = if code49 {
            anyd::codes::code49::Code49Decoder::new().decode_matrix(&m)
        } else {
            anyd::codes::codablockf::CodablockFDecoder::new().decode_matrix(&m)
        }
        .unwrap();
        assert_eq!(decoded.text().unwrap(), "HELLO WORLD");
    }
}

#[test]
fn maxicode_raster_round_trips() {
    for mode in [2, 3, 4, 5, 6] {
        let data = match mode {
            2 => "001840123450000Hello MaxiCode",
            3 => "001826ABC123Hello MaxiCode",
            _ => "Hello MaxiCode",
        };
        let label = format!("^XA^PW400^LL400^FO20,20^BD{mode}^FD{data}^FS^XZ");
        let doc = render(label.as_bytes(), Options::default()).unwrap();
        let r = rasterize(&doc.labels[0]).unwrap();
        let pitch = (25.50 / 29.) * 203. / 25.4;
        let radius = pitch / 3f64.sqrt();
        let mut matrix = anyd::output::BitMatrix::new(30, 33, 0);
        for y in 0..33 {
            for x in 0..30 {
                let px = (20. + (x as f64 + 0.5 + (y % 2) as f64 / 2.) * pitch).floor() as usize;
                let py = (20. + y as f64 * pitch * 3f64.sqrt() / 2. + radius).floor() as usize;
                matrix.set(x, y, r.pixels[py * r.width as usize + px] < 128);
            }
        }
        let result = anyd::codes::maxicode::MaxiCodeDecoder::new()
            .decode_matrix(&matrix)
            .unwrap();
        assert!(result.text().unwrap().ends_with("Hello MaxiCode"));
    }
}
#[test]
fn aztec_round_trips() {
    for data in ["A", "ABC123", "Hello Aztec 1234567890"] {
        decode("BON,5", data, data, BarcodeFormat::AZTEC);
    }
}
#[test]
fn pdf417_round_trips() {
    decode("BT", "123456", "123456", BarcodeFormat::CODE_39);
    for length in 1..=12 {
        let data = "abcdefABCDEF".get(..length).unwrap();
        decode("B7N,9,2,4", data, data, BarcodeFormat::PDF_417);
    }
    for level in 0..=4 {
        for data in ["ABC123", "Hello PDF417 1234567890"] {
            decode(
                &format!("B7N,9,{level},4"),
                data,
                data,
                BarcodeFormat::PDF_417,
            );
        }
    }
}

#[test]
fn pdf417_compaction_and_field_escapes() {
    for data in [
        "A",
        "abcXYZxyz",
        "Hello PDF417",
        "1234567",
        "12345678",
        "123456789012345678901234567890123456789012345678901234567890",
        "000000000000000000000000000000000000000000000000000000000000",
        "abc1234567890123DEF",
        "ABCDEF\u{1}abcdef",
        "abcde\u{1}ABCDEF",
        "abcdef\u{1}ghijkl",
        "1234567\u{1}abcdef",
        "ABCDE\u{1}\u{2}abcdef",
        ";<>@[]_`!\r\t,:\n-.$/\"|*()?{}'",
        "ABC\r\nDEF",
    ] {
        let field: String = data.bytes().map(|v| format!("_{v:02X}")).collect();
        decode("B7N,9,2,4^FH", &field, data, BarcodeFormat::PDF_417);
    }
    decode(
        "B7N,9,2,4",
        "ABC\\&DEF",
        "ABC\r\nDEF",
        BarcodeFormat::PDF_417,
    );
    decode(
        "B7N,9,2,4",
        "ABC\\\\DEF",
        "ABC\\DEF",
        BarcodeFormat::PDF_417,
    );
    // Text compaction makes this printer-supported fixed-size symbol fit.
    decode("B7N,9,2,4,3", "ABCDEF", "ABCDEF", BarcodeFormat::PDF_417);
    // Check all byte values without letting ZPL delimiters become commands.
    let data: Vec<u8> = (0..=255).collect();
    let field: String = data.iter().map(|v| format!("_{v:02X}")).collect();
    let source = format!("^XA^PW1600^LL1600^FO50,50^BY3,2,90^B7N,9,2,10^FH^FD{field}^FS^XZ");
    let doc = render(source.as_bytes(), Options::default()).unwrap();
    let r = rasterize(&doc.labels[0]).unwrap();
    let found =
        rxing::helpers::detect_in_luma(r.pixels, r.width, r.height, Some(BarcodeFormat::PDF_417))
            .unwrap();
    // No consecutive backslashes or backslash-ampersand occur in this sequence.
    let expected: String = data.iter().map(|&v| char::from(v)).collect();
    assert_eq!(found.getText(), expected);
}

fn sampled(
    command: &str,
    data: &str,
    w: usize,
    h: usize,
    x_scale: usize,
    y_scale: usize,
) -> anyd::output::BitMatrix {
    let label = format!("^XA^PW2000^LL1200^FO20,20^BY3,2,90^{command}^FD{data}^FS^XZ");
    let doc = render(label.as_bytes(), Options::default()).unwrap();
    let r = rasterize(&doc.labels[0]).unwrap();
    let mut m = anyd::output::BitMatrix::new(w, h, 0);
    for y in 0..h {
        for x in 0..w {
            m.set(
                x,
                y,
                r.pixels[(20 + y * y_scale + y_scale / 2) * r.width as usize
                    + 20
                    + x * x_scale
                    + x_scale / 2]
                    < 128,
            );
        }
    }
    m
}
#[test]
fn micropdf417_all_sizes() {
    let rows = [
        11, 14, 17, 20, 24, 28, 8, 11, 14, 17, 20, 23, 26, 6, 8, 10, 12, 15, 20, 26, 32, 38, 44, 6,
        8, 10, 12, 15, 20, 26, 32, 38, 44, 4,
    ];
    for (mode, rows) in rows.into_iter().enumerate() {
        let cols = match mode {
            0..=5 => 1,
            6..=12 => 2,
            13..=22 => 3,
            _ => 4,
        };
        let m = sampled(
            &format!("BFN,6,{mode}"),
            "A",
            17 * cols + 21 + if cols >= 3 { 10 } else { 0 },
            rows * 2,
            3,
            3,
        );
        let decoded = anyd::codes::pdf417::MicroPdf417Decoder::new()
            .decode_matrix(&m)
            .unwrap_or_else(|e| panic!("mode {mode}: {e}"));
        assert_eq!(decoded.text().unwrap(), "A");
    }
}

#[test]
fn micropdf417_compaction_and_capacity() {
    for (mode, rows, data, expected) in [
        (0, 11, "ABCDEF", "ABCDEF"),
        (0, 11, "12345678", "12345678"),
        (33, 4, "ABCDEF", "ABCDEF"),
        (32, 44, "Hello Micro417", "Hello Micro417"),
        (32, 44, "_80", "\u{80}"),
        (32, 44, "_01_02_03_04_05_06", "\x01\x02\x03\x04\x05\x06"),
        (
            32,
            44,
            "_01_02_03_04_05_06_07",
            "\x01\x02\x03\x04\x05\x06\x07",
        ),
        (
            32,
            44,
            "Hello_80_81WORLD12345678end",
            "Hello\u{80}\u{81}WORLD12345678end",
        ),
        (
            32,
            44,
            "12345678901234567890123456789012345678901234567890",
            "12345678901234567890123456789012345678901234567890",
        ),
        (32, 44, r"ABC\&DEF\\GHI", "ABC\r\nDEF\\GHI"),
    ] {
        let width = if mode == 0 { 38 } else { 99 };
        let m = sampled(&format!("BFN,6,{mode}^FH"), data, width, rows * 2, 3, 3);
        let decoded = anyd::codes::pdf417::MicroPdf417Decoder::new()
            .decode_matrix(&m)
            .unwrap_or_else(|e| panic!("{data}: {e}"));
        assert_eq!(
            decoded.payload_bytes(),
            expected.chars().map(|c| c as u8).collect::<Vec<_>>(),
            "mode {mode}: {data}"
        );
    }
    assert!(render(b"^XA^BFN,4,0^FDABCDEFG^FS^XZ", Options::default())
        .unwrap_err()
        .to_string()
        .contains("capacity"));
}

#[test]
fn tlc39_linkage_and_components() {
    use rxing::oned::OneDReader;
    for (payload, expected, rows, ec) in [
        ("ABC", "ABC", 4, 8),
        ("12345678901234", "12345678901234", 8, 14),
        ("ABC,DEF", "ABC\u{1d}DEF", 4, 8),
    ] {
        let label = format!("^XA^PW600^LL500^FO20,20^BTN,2,2,40,2,4^FD239316,{payload}^FS^XZ");
        let doc = render(label.as_bytes(), Options::default()).unwrap();
        let raster = rasterize(&doc.labels[0]).unwrap();
        let pixel = |x: usize, y: usize| raster.pixels[y * raster.width as usize + x] < 128;
        let mut codewords = Vec::new();
        for y in 0..rows {
            for start in [10, 27, 54, 71] {
                let pattern = (start..start + 17).fold(0, |v, x| {
                    (v << 1) | u32::from(pixel(22 + 2 * x + 1, 20 + 4 * y + 2))
                });
                let word = rxing::pdf417::pdf_417_common::getCodeword(pattern);
                assert!(
                    word >= 0,
                    "invalid MicroPDF417 pattern {pattern:017b}, {payload}, row {y}, x {start}"
                );
                codewords.push(word as u32);
            }
        }
        // Independently verify every codeword, including linkage, with GF(929)
        // error correction. Then consume the TLC-specific marker explicitly;
        // existing generic MicroPDF417 decoders do not accept codeword 918.
        assert_eq!(
            rxing::pdf417::decoder::ec::error_correction::decode(&mut codewords, ec, &mut [])
                .unwrap(),
            0
        );
        assert_eq!(codewords[0], 918);
        let capacity = codewords.len() - ec as usize;
        let mut ordinary = vec![capacity as u32];
        ordinary.extend_from_slice(&codewords[1..capacity]);
        let decoded =
            rxing::pdf417::decoder::decoded_bit_stream_parser::decode(&ordinary, "").unwrap();
        assert_eq!(decoded.getText(), expected);
        let linear_y = 20 + rows * 4 + 8 + 20;
        let mut scan = rxing::common::BitArray::new();
        for x in 0..600 {
            scan.appendBit(pixel(x, linear_y));
        }
        let decoded = rxing::oned::Code39Reader::default()
            .decode_row(0, &scan, &Default::default())
            .unwrap();
        assert_eq!(decoded.getText(), "239316");
        // Eight Code 39 characters (including start/stop), seven 1X gaps,
        // 10X quiet zone, then the isolated T linkage flag.
        let flag_x = 20 + 2 * (8 * 12 + 7 + 10);
        let mut at = flag_x;
        for (i, width) in [1, 1, 1, 1, 2, 1, 2, 2, 1].into_iter().enumerate() {
            for x in at..at + width * 2 {
                assert_eq!(pixel(x, linear_y), i % 2 == 0);
            }
            at += width * 2;
        }
    }
}
