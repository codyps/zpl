//! Synthetic font download regressions, not printer fidelity claims.
//! Zebra Programming Guide ~DB, ~DT, ~DU, ~DY and B64/Z64 appendix:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use zpl::{
    bitmap_font::{Glyph, Settings},
    render::{
        fonts::Fonts, profiles::SPECIFICATION, render_with_fonts, render_with_fonts_and_limits,
        Limits,
    },
    truetype::Hinting,
};
const TTF: &[u8] = include_bytes!("fixtures/truetype-regression/font-probes-20261002/probe.ttf");
fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02X}")).collect()
}
fn bitmap(data: &str, advance: u32) -> String {
    format!("~DBR:TEST.FNT,N,10,10,7,4,1,TEST,#0041.2.8.1.7.{advance}.{data}")
}
fn label(selection: &str, text: &str) -> String {
    format!("^XA^PW100^LL100^FO10,10{selection}^FD{text}^FS^XZ")
}
fn raster(input: &[u8], fonts: &Fonts<'_>) -> Vec<raster_diff::Raster> {
    render_with_fonts(input, SPECIFICATION, fonts)
        .unwrap()
        .labels
        .iter()
        .map(|s| zpl::output::raster::rasterize(s).unwrap())
        .collect()
}
fn reference(advance: u32, rows: &[u8]) -> Fonts<'static> {
    let mut fonts = Fonts::new();
    fonts
        .insert_bitmap(
            'Z',
            Settings {
                font: '0',
                height: 10,
                width: 10,
                dpi: 203,
            },
            vec![
                Glyph {
                    codepoint: 65,
                    advance,
                    left: 1,
                    top: -7,
                    width: 8,
                    height: 2,
                    bitmap: rows.iter().map(|b| vec![*b]).collect(),
                },
                Glyph {
                    codepoint: 32,
                    advance: 4,
                    left: 0,
                    top: 0,
                    width: 0,
                    height: 0,
                    bitmap: vec![],
                },
            ],
            7.,
        )
        .unwrap();
    fonts
}
#[test]
fn bitmap_hex_b64_z64_metrics_rotations_and_missing_spaces() {
    let reference = reference(9, &[255, 129]);
    for data in ["FF81", ":B64:/4E=:EF02", ":Z64:eJz73wgAAoEBgQ==:12CE"] {
        for rotation in ['N', 'R', 'I', 'B'] {
            for size in [10, 20, 31] {
                let expected = label(&format!("^AZ{rotation},{size},{size}"), "A A");
                let actual = format!(
                    "{}{}",
                    bitmap(data, 9),
                    label(&format!("^A@{rotation},{size},{size},R:TEST.FNT"), "A?A")
                );
                assert_eq!(
                    raster(actual.as_bytes(), &Fonts::new()),
                    raster(expected.as_bytes(), &reference)
                );
            }
        }
    }
}
#[test]
fn replacements_update_aliases_and_remembered_names_in_order() {
    let first = bitmap("FF81", 9);
    let second = bitmap("8181", 5);
    let one = label("^AZN,10,10", "AA");
    let direct = label("^A@N,10,10,R:TEST.FNT", "AA");
    let remembered = label("^A@N,10,10", "AA");
    let actual = format!("{first}^CWZ,R:TEST.FNT{one}{direct}{second}{one}{remembered}");
    let a = raster(one.as_bytes(), &reference(9, &[255, 129])).remove(0);
    let b = raster(one.as_bytes(), &reference(5, &[129, 129])).remove(0);
    assert_ne!(a, b);
    assert_eq!(
        raster(actual.as_bytes(), &Fonts::new()),
        vec![a.clone(), a, b.clone(), b]
    );
    assert!(zpl::render(direct.as_bytes(), SPECIFICATION)
        .unwrap_err()
        .message
        .contains("unresolved named font"));
}
#[test]
fn downloads_override_named_caller_resources_without_mutation() {
    let mut fonts = Fonts::new();
    fonts
        .insert_named_truetype("R:TEST.FNT", TTF, Hinting::Native)
        .unwrap();
    let before = label("^A@N,24,24,R:TEST.FNT", "!");
    let expected = raster(before.as_bytes(), &fonts);
    let actual = format!(
        "^CWZ,R:TEST.FNT{}{}",
        bitmap("FF81", 9),
        label("^AZN,10,10", "AA")
    );
    assert_eq!(
        raster(actual.as_bytes(), &fonts),
        raster(
            label("^AZN,10,10", "AA").as_bytes(),
            &reference(9, &[255, 129])
        )
    );
    assert_eq!(raster(before.as_bytes(), &fonts), expected);
}
#[test]
fn truetype_downloads_match_host_registration() {
    let mut fonts = Fonts::new();
    fonts.insert_truetype('Z', TTF, Hinting::Native).unwrap();
    for (command, name, binary) in [
        ("DT", "TEST.DAT", false),
        ("DU", "TEST.FNT", false),
        ("DY", "TEST.TTF", true),
        ("DY", "TEST.OTF", false),
        ("DY", "TEST.TTE", true),
    ] {
        for rotation in ['N', 'R', 'I', 'B'] {
            let mut input = if command == "DY" {
                format!(
                    "~DY{name},{},{},{},,",
                    if binary { "B" } else { "A" },
                    if name.ends_with("TTE") { "E" } else { "T" },
                    TTF.len()
                )
                .into_bytes()
            } else {
                format!("~D{}{},{} ,", &command[1..], name, TTF.len()).into_bytes()
            };
            input.extend(if binary {
                TTF.to_vec()
            } else {
                hex(TTF).into_bytes()
            });
            input.extend(label(&format!("^A@{rotation},24,31,R:{name}"), "!!").bytes());
            assert_eq!(
                raster(&input, &Fonts::new()),
                raster(
                    label(&format!("^AZ{rotation},24,31"), "!!").as_bytes(),
                    &fonts
                )
            );
        }
    }
    // Omitted legacy names/extensions use their command-specific defaults.
    for (command, name) in [("DT", "DAT"), ("DU", "FNT")] {
        let input = format!(
            "~{command},{},{}{}",
            TTF.len(),
            hex(TTF),
            label(&format!("^A@N,24,31,R:UNKNOWN.{name}"), "!!")
        );
        assert_eq!(
            raster(input.as_bytes(), &Fonts::new()),
            raster(label("^AZN,24,31", "!!").as_bytes(), &fonts)
        );
    }
}
#[test]
fn binary_payloads_do_not_execute_embedded_commands_and_syntax_changes_work() {
    let mut data = TTF.to_vec();
    data.extend(b"^XZ~DB^CC!\0\xff"); // SFNT allows trailing bytes; parser must consume them.
    let mut input = format!("^CD;^CT?^CC!?DYTEST;B;T;{};;", data.len()).into_bytes();
    input.extend(data);
    input.extend(b"!XA!PW100!LL100!FO10;10!A@N;24;31;R:TEST.TTF!FD!!FS!XZ");
    // Changed prefix makes ! unavailable as literal field data; use FH.
    let end = input.len() - b"!FD!!FS!XZ".len();
    input.truncate(end);
    input.extend(b"!FH!FD_21!FS!XZ");
    let mut fonts = Fonts::new();
    fonts.insert_truetype('Z', TTF, Hinting::Native).unwrap();
    assert_eq!(
        raster(&input, &Fonts::new()),
        raster(label("^AZN,24,31", "!").as_bytes(), &fonts)
    );
}
#[test]
fn downloads_work_in_recalled_formats() {
    let input = format!("^XA^DFR:LABEL.ZPL{}^FO10,10^A@N,10,10,R:TEST.FNT^FDA^FS^XZ^XA^PW100^LL100^XFR:LABEL.ZPL^XZ",bitmap("FF81",9));
    assert_eq!(
        raster(input.as_bytes(), &Fonts::new()),
        raster(
            label("^AZN,10,10", "A").as_bytes(),
            &reference(9, &[255, 129])
        )
    );
}
#[test]
fn invalid_downloads_and_limits_report_command_offsets() {
    for (command, message) in [
        (bitmap("FF", 9), "byte count"),
        (bitmap("FF8100", 9), "byte count"),
        (bitmap("FF8", 9), "incomplete font hex"),
        (bitmap(":B64:/4E=:0000", 9), "CRC mismatch"),
        (bitmap(":Z64:eJz73wgAAoEBgQ==:0000", 9), "CRC mismatch"),
        (bitmap("GGGG", 9), "hex digit"),
        (
            bitmap("FF81", 9).replace(",1,TEST,", ",2,TEST,"),
            "glyph count",
        ),
        (
            bitmap("FF81", 9).replace(".2.8.", ".4097.8."),
            "supported range",
        ),
        ("~DYTEST,A,G,1,,00".into(), "object type"),
        ("~DYTEST,C,T,1,,x".into(), "encoding"),
        (
            "~DUTEST,4,01020304".into(),
            "unsupported downloaded font format",
        ),
        ("~DUTEST,4,00010000".into(), "TrueType"),
        ("^DBTEST".into(), "control prefix"),
    ] {
        let input = format!("^XA{command}^XZ");
        let err = zpl::render(input.as_bytes(), SPECIFICATION).unwrap_err();
        assert_eq!(err.offset, 3, "{command}: {err}");
        assert!(err.message.contains(message), "{command}: {err}");
    }
    let mut input = format!("~DYTEST,B,T,{},,", TTF.len()).into_bytes();
    input.extend(TTF);
    input.extend(label("^A@N,24,24,R:TEST.TTF", "!").bytes());
    let limits = Limits {
        font_bytes: TTF.len() - 1,
        ..Limits::default()
    };
    assert!(
        render_with_fonts_and_limits(&input, SPECIFICATION, &Fonts::new(), limits)
            .unwrap_err()
            .message
            .contains("font bytes")
    );
    let limits = Limits {
        font_bytes: TTF.len(),
        ..limits
    };
    assert!(render_with_fonts_and_limits(&input, SPECIFICATION, &Fonts::new(), limits).is_ok());
    let mut twice = input.clone();
    twice.extend(&input);
    assert!(
        render_with_fonts_and_limits(&twice, SPECIFICATION, &Fonts::new(), limits)
            .unwrap_err()
            .message
            .contains("font bytes")
    );
}

#[test]
fn resources_are_not_visible_before_their_download_and_errors_stay_ordered() {
    let future = format!(
        "^CWZ,R:TEST.FNT{}{}",
        bitmap("FF81", 9),
        label("^AZN,10,10", "A")
    );
    let err = zpl::render(future.as_bytes(), SPECIFICATION).unwrap_err();
    assert_eq!(err.offset, 0);
    assert!(err.message.contains("unresolved named font"));
    let invalid_later = format!("^XA^ZZ{}^XZ", bitmap("GG", 9));
    let err = zpl::render(invalid_later.as_bytes(), SPECIFICATION).unwrap_err();
    assert_eq!(err.offset, 3);
    assert!(err.message.contains("unsupported"));
    let duplicate = bitmap("FF81", 9).replace(",1,TEST,", ",2,TEST,") + "#0041.2.8.1.7.9.FF81";
    assert!(
        zpl::render(format!("{duplicate}^XA^XZ").as_bytes(), SPECIFICATION)
            .unwrap_err()
            .message
            .contains("duplicate bitmap")
    );
}

// Independent transport generator; known bitmap B64/Z64 vectors above also pin
// the CRC convention, so a decoder/encoder agreement alone is not the evidence.
fn b64(data: &[u8]) -> String {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::new();
    for chunk in data.chunks(3) {
        let v = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for (index, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            encoded.push(if index > chunk.len() {
                '='
            } else {
                alphabet[((v >> shift) & 63) as usize] as char
            });
        }
    }
    let mut crc = 0u16;
    for b in encoded.bytes() {
        for bit in (0..8).rev() {
            let carry = ((crc >> 15) ^ u16::from((b >> bit) & 1)) != 0;
            crc <<= 1;
            if carry {
                crc ^= 0x1021;
            }
        }
    }
    format!(":B64:{encoded}:{crc:04X}")
}
#[test]
fn truetype_b64_transports_and_malformed_sfnt_are_checked() {
    assert_eq!(b64(&[255, 129]), ":B64:/4E=:EF02");
    let mut fonts = Fonts::new();
    fonts.insert_truetype('Z', TTF, Hinting::Native).unwrap();
    for (command, name) in [("DT", "TEST.DAT"), ("DU", "TEST.FNT"), ("DY", "TEST.TTF")] {
        let header = if command == "DY" {
            format!("~DY{name},A,T,{},,", TTF.len())
        } else {
            format!("~{command}{name},{},", TTF.len())
        };
        let input = format!(
            "{header}{}{}",
            b64(TTF),
            label(&format!("^A@N,24,31,R:{name}"), "!!")
        );
        assert_eq!(
            raster(input.as_bytes(), &Fonts::new()),
            raster(label("^AZN,24,31", "!!").as_bytes(), &fonts)
        );
    }
    for bytes in [&TTF[..4], &TTF[..TTF.len() / 2]] {
        let mut input = format!("~DYTEST,B,T,{},,", bytes.len()).into_bytes();
        input.extend(bytes);
        input.extend(b"^XA^XZ");
        assert!(zpl::render(&input, SPECIFICATION).is_err());
    }
}

#[test]
fn bitmap_allocation_limits_and_stored_format_error_offsets() {
    let limits = Limits {
        font_bytes: 1,
        ..Limits::default()
    };
    // A large declared glyph must hit the budget before decoding its short Z64 data.
    let input =
        b"~DBTEST,N,10,10,7,4,1,TEST,#0041.4096.4096.0.0.9.:Z64:eJz73wgAAoEBgQ==:12CE^XA^XZ";
    let err =
        render_with_fonts_and_limits(input, SPECIFICATION, &Fonts::new(), limits).unwrap_err();
    assert!(err.message.contains("font bytes"), "{err}");
    let input = format!(
        "^XA^DFR:LABEL.ZPL{}^XZ^XA^XFR:LABEL.ZPL^XZ",
        bitmap("GG", 9)
    );
    let err = zpl::render(input.as_bytes(), SPECIFICATION).unwrap_err();
    assert_eq!(err.offset, input.find("~DB").unwrap());
    assert!(err.message.contains("hex digit"));
}

#[test]
fn downloaded_bitmap_metrics_follow_the_resource_not_the_alias() {
    // ~DB pp. 169–170; measured sizing/baseline semantics are pinned separately
    // by downloaded_bitmap_preview. Caller bitmap APIs retain explicit metrics.
    use zpl::render::profiles::ZD621_203_DPI;
    let source = format!(
        "{}{}",
        bitmap("FF81", 9),
        label("^A@N,31,26,R:TEST.FNT", "AA")
    );
    let direct = zpl::render(source.as_bytes(), ZD621_203_DPI).unwrap();
    let image = zpl::output::raster::rasterize(&direct.labels[0]).unwrap();
    for id in ['A', 'D', '0', 'Z'] {
        let source = format!(
            "{}^CW{id},R:TEST.FNT{}",
            bitmap("FF81", 9),
            label(&format!("^A{id}N,31,26"), "AA")
        );
        let doc = zpl::render(source.as_bytes(), ZD621_203_DPI).unwrap();
        assert_eq!(
            image,
            zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
        );
    }
    let mut options = ZD621_203_DPI;
    options.compatibility.downloaded_bitmap_font_metrics = false;
    let uncalibrated = zpl::render(source.as_bytes(), options).unwrap();
    assert_ne!(
        image,
        zpl::output::raster::rasterize(&uncalibrated.labels[0]).unwrap()
    );
    let caller = reference(9, &[255, 129]);
    let expected =
        render_with_fonts(label("^AZN,31,26", "AA").as_bytes(), ZD621_203_DPI, &caller).unwrap();
    assert_eq!(
        zpl::output::raster::rasterize(&expected.labels[0]).unwrap(),
        zpl::output::raster::rasterize(&uncalibrated.labels[0]).unwrap()
    );
}
