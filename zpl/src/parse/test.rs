use super::*;

fn split(input: &[u8]) -> Vec<Element<'_>> {
    let elements = ParseContext::from_bytes(input)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let restored: Vec<u8> = elements
        .iter()
        .flat_map(|e| e.as_bytes().iter().copied())
        .collect();
    assert_eq!(restored, input);
    elements
}

fn slices(input: &[u8]) -> Vec<&[u8]> {
    split(input).iter().map(Element::as_bytes).collect()
}

#[test]
fn ordinary_fields_comments_and_unknown_commands_are_lossless() {
    let input = b"\0 preamble\r\n^XA\n^A0N,30,30^FDhi\xff\0^FS^FXnote^FO1,2~HS^??opaque^XZ\r\n";
    assert_eq!(
        slices(input),
        [
            &b"\0 preamble\r\n"[..],
            b"^XA\n",
            b"^A0N,30,30",
            b"^FDhi\xff\0",
            b"^FS",
            b"^FXnote",
            b"^FO1,2",
            b"~HS",
            b"^??opaque",
            b"^XZ\r\n",
        ]
    );
    assert!(matches!(split(input)[0], Element::BeforeFirstCommand(_)));
    assert!(matches!(split(input)[7], Element::ControlCommand(_)));
}

#[test]
fn font_forms_and_omitted_parameters() {
    assert_eq!(
        slices(b"^A^A0^AF^A@N,20,20,R:FONT.TTF^B3,,100^XZ"),
        [
            &b"^A"[..],
            b"^A0",
            b"^AF",
            b"^A@N,20,20,R:FONT.TTF",
            b"^B3,,100",
            b"^XZ",
        ]
    );
    assert_eq!(slices(b"^A"), [b"^A"]);
}

#[test]
fn syntax_changes_apply_immediately_and_persist_across_labels() {
    let input = b"^XA~CC##XZ#XA#CT!#CD;#FO1;2!HS!CC^!CT~^CD,^XZ";
    let mut parser = ParseContext::from_bytes(input);
    let elements = parser.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(
        elements.iter().map(Element::as_bytes).collect::<Vec<_>>(),
        [
            &b"^XA"[..],
            b"~CC#",
            b"#XZ",
            b"#XA",
            b"#CT!",
            b"#CD;",
            b"#FO1;2",
            b"!HS",
            b"!CC^",
            b"!CT~",
            b"^CD,",
            b"^XZ",
        ]
    );
    assert_eq!(parser.syntax(), Syntax::default());
    assert_eq!(parser.position(), input.len());
}

#[test]
fn syntax_operand_may_itself_be_a_prefix_or_newline() {
    assert_eq!(slices(b"^CD^^FO1^XZ"), [&b"^CD^"[..], b"^FO1", b"^XZ"]);
}

#[test]
fn newline_is_not_skipped_when_changing_a_prefix() {
    let mut parser = ParseContext::from_bytes(b"^CC\n\nXA\nXZ");
    let parts = parser.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(
        parts.iter().map(Element::as_bytes).collect::<Vec<_>>(),
        [&b"^CC\n"[..], b"\nXA", b"\nXZ",]
    );
    assert_eq!(parser.syntax().format_prefix, b'\n');
}

#[test]
fn custom_prefix_inside_mnemonic_is_not_a_boundary() {
    assert_eq!(slices(b"^CCCCCACXZ"), [&b"^CCC"[..], b"CCA", b"CXZ"]);
    let mut first = ParseContext::from_bytes(b"^CC//CT!/CD;");
    first.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
    let next = ParseContext::with_syntax(b"/FO1;2!HS/XZ", first.syntax());
    assert_eq!(
        next.map(|e| e.unwrap().as_bytes()).collect::<Vec<_>>(),
        [&b"/FO1;2"[..], b"!HS", b"/XZ",]
    );
}

#[test]
fn binary_graphics_ignore_all_byte_values_and_use_transmitted_count() {
    for mode in ['B', 'C'] {
        let mut input = format!("^XA^GF{mode},256,4096,16,").into_bytes();
        let data_start = input.len();
        input.extend(0..=255);
        input.extend_from_slice(b"^FS^XZ");
        let parts = split(&input);
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[1].as_bytes(), &input[3..data_start + 256]);
        assert_eq!(parts[2].as_bytes(), b"^FS");
    }
}

#[test]
fn binary_objects_do_not_apply_embedded_syntax_changes() {
    for mode in ['B', 'C'] {
        let payload = b"\0^CC/\xff~CT!^CD;\x11\x13";
        let mut input = format!("~DYR:TEST.TTF,{mode},T,{},,", payload.len()).into_bytes();
        input.extend_from_slice(payload);
        let boundary = input.len();
        input.extend_from_slice(b"^XA^XZ");
        let mut parser = ParseContext::from_bytes(&input);
        assert_eq!(
            parser.next().unwrap().unwrap().as_bytes(),
            &input[..boundary]
        );
        assert_eq!(parser.syntax(), Syntax::default());
        assert_eq!(parser.next().unwrap().unwrap().as_bytes(), b"^XA");
        assert_eq!(parser.next().unwrap().unwrap().as_bytes(), b"^XZ");
    }
}

#[test]
fn binary_download_headers_use_current_delimiter() {
    let input = b"^CD;^GFB;4;4;4;^~\0\xff^FS~DYR:T;B;T;3;;^~\0^XZ";
    assert_eq!(
        slices(input),
        [
            &b"^CD;"[..],
            b"^GFB;4;4;4;^~\0\xff",
            b"^FS",
            b"~DYR:T;B;T;3;;^~\0",
            b"^XZ",
        ]
    );
}

#[test]
fn binary_framing_applies_only_to_the_documented_prefix_kind() {
    assert_eq!(
        slices(b"~GFB,3,3,3,^FS^DYR:T,B,T,3,,^XZ"),
        [&b"~GFB,3,3,3,"[..], b"^FS", b"^DYR:T,B,T,3,,", b"^XZ",]
    );
}

#[test]
fn ascii_graphics_abort_at_commands_and_preserve_compression_codes() {
    assert_eq!(
        slices(b"^GFA,8,8,2,FF00\r\nG0,:!~DN^FS"),
        [&b"^GFA,8,8,2,FF00\r\nG0,:!"[..], b"~DN", b"^FS",]
    );
    assert_eq!(
        slices(b"~DGR:X.GRF,100,10,00^CC//XZ"),
        [&b"~DGR:X.GRF,100,10,00"[..], b"^CC/", b"/XZ",]
    );
}

#[test]
fn encoded_downloads_use_crc_boundary_not_decoded_length() {
    for header in [
        "^GFA,999,999,1,",
        "~DGR:T.GRF,999,1,",
        "~DER:T.DAT,999,",
        "~DSR:T.FNT,999,",
        "~DTR:T.TTF,999,",
        "~DUR:T.TTF,999,",
        "~DYR:T,A,G,999,1,",
    ] {
        for encoding in ["B64", "Z64"] {
            let command = format!("{header}\r\n:{encoding}:AA+/\r\nAA==:a1B2");
            let input = format!("{command}^XZ");
            assert_eq!(slices(input.as_bytes()), [command.as_bytes(), b"^XZ"]);
        }
    }
}

#[test]
fn remapped_prefix_in_base64_and_crc_remains_payload() {
    let input = b"^CC//GFA,4,4,1,:B64:AA/AAA==:a1B2/XZ";
    assert_eq!(
        slices(input),
        [&b"^CC/"[..], b"/GFA,4,4,1,:B64:AA/AAA==:a1B2", b"/XZ"]
    );
    let input = b"^CCAAGFA,4,4,1,:B64:AAAAAA==:AAAAAXZ";
    assert_eq!(
        slices(input),
        [&b"^CCA"[..], b"AGFA,4,4,1,:B64:AAAAAA==:AAAA", b"AXZ"]
    );
}

#[test]
fn bitmap_font_preserves_multiple_encoded_glyphs() {
    let font = b"~DBR:T.FNT,N,8,8,8,8,2,TEST,#0001.8.8.0.0.8.:B64:AA==:0000\n#0002.8.8.0.0.8.:Z64:AA==:0000";
    let mut input = font.to_vec();
    input.extend_from_slice(b"^XZ");
    assert_eq!(slices(&input), [font.as_slice(), b"^XZ"]);
    // An encoding signature in normal field data is not treated as a download.
    assert_eq!(slices(b"^FD:B64:bad^FS"), [&b"^FD:B64:bad"[..], b"^FS"]);
}

#[test]
fn field_hex_does_not_reinterpret_decoded_prefixes() {
    assert_eq!(
        slices(b"^FH_^FD_5E_7E_00^FS^FE%Ignored^XZ"),
        [
            &b"^FH_"[..],
            b"^FD_5E_7E_00",
            b"^FS",
            b"^FE%Ignored",
            b"^XZ",
        ]
    );
}

#[test]
fn framing_errors_report_offset_and_stop_iteration() {
    let cases: &[(&[u8], ParseErrorKind)] = &[
        (b"^", ParseErrorKind::IncompleteCommand),
        (b"^X", ParseErrorKind::IncompleteCommand),
        (b"^CC", ParseErrorKind::MissingSyntaxCharacter),
        (b"~CT", ParseErrorKind::MissingSyntaxCharacter),
        (b"^CD", ParseErrorKind::MissingSyntaxCharacter),
        (b"^GFB", ParseErrorKind::InvalidBinaryHeader),
        (b"~DYR:T,B", ParseErrorKind::InvalidBinaryHeader),
        (b"^GFB,NaN,1,1,x", ParseErrorKind::InvalidBinaryLength),
        (
            b"^GFB,999999999999999999999999999999,1,1,x",
            ParseErrorKind::InvalidBinaryLength,
        ),
        (b"~DYR:T,B,T,-1,,x", ParseErrorKind::InvalidBinaryLength),
        (
            b"^GFB,100,100,1,abc^FS",
            ParseErrorKind::TruncatedBinaryData,
        ),
        (
            b"~DYR:T,B,T,100,,abc^XZ",
            ParseErrorKind::TruncatedBinaryData,
        ),
        (
            b"^GFA,1,1,1,:B64:AA==",
            ParseErrorKind::IncompleteEncodedData,
        ),
        (
            b"~DGR:T,1,1,:Z64:AA==:00",
            ParseErrorKind::IncompleteEncodedData,
        ),
        (
            b"~DTR:T,1,:B64:AA!==:0000",
            ParseErrorKind::InvalidEncodedData,
        ),
        (
            b"~DUR:T,1,:B64:AA==:GGGG",
            ParseErrorKind::InvalidEncodedData,
        ),
    ];
    for &(bad, kind) in cases {
        let mut input = b"^XA".to_vec();
        input.extend_from_slice(bad);
        let mut parser = ParseContext::from_bytes(&input);
        assert_eq!(parser.next().unwrap().unwrap().as_bytes(), b"^XA");
        assert_eq!(
            parser.next().unwrap().unwrap_err(),
            ParseError { offset: 3, kind },
            "{bad:?}"
        );
        assert_eq!(parser.position(), 3);
        assert!(parser.next().is_none());
        assert!(parser.next_element().unwrap().is_none());
    }
}

#[test]
fn arbitrary_byte_streams_never_panic_or_stall() {
    let mut seed = 7u32;
    for length in 0..512 {
        let input: Vec<_> = (0..length)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed >> 24) as u8
            })
            .collect();
        for syntax in [
            Syntax::default(),
            Syntax {
                format_prefix: 0,
                control_prefix: 255,
                delimiter: 1,
            },
        ] {
            let mut parser = ParseContext::with_syntax(&input, syntax);
            let mut restored = Vec::new();
            while let Some(result) = parser.next() {
                match result {
                    Ok(element) => {
                        assert!(!element.as_bytes().is_empty());
                        restored.extend_from_slice(element.as_bytes());
                    }
                    Err(_) => {
                        restored.extend_from_slice(&input[parser.position()..]);
                        assert!(parser.next().is_none());
                        break;
                    }
                }
            }
            assert_eq!(restored, input);
        }
    }
}

#[test]
fn single_byte_format_and_field_separators_are_commands() {
    let input = b"\x02^FO1,2^FDhello\x0f\x03";
    let parts = split(input);
    assert_eq!(
        parts.iter().map(Element::as_bytes).collect::<Vec<_>>(),
        [&b"\x02"[..], b"^FO1,2", b"^FDhello", b"\x0f", b"\x03",]
    );
    assert!(matches!(parts[0], Element::ControlCharacter(b"\x02")));
    assert!(matches!(parts[3], Element::ControlCharacter(b"\x0f")));
    assert!(matches!(parts[4], Element::ControlCharacter(b"\x03")));
    assert_eq!(slices(b"^A\x0f\x03"), [&b"^A"[..], b"\x0f", b"\x03"]);
}

#[test]
fn every_truncated_binary_payload_fails_before_exposing_commands() {
    let payload = b"^CC/\x02~CT!\x03^XZ\x0f\xff\0";
    for header in [
        format!("^GFB,{0},{0},1,", payload.len()),
        format!("~DYR:T,B,T,{},,", payload.len()),
    ] {
        for length in 0..payload.len() {
            let mut input = header.as_bytes().to_vec();
            input.extend_from_slice(&payload[..length]);
            let mut parser = ParseContext::from_bytes(&input);
            let error = parser.next().unwrap().unwrap_err();
            assert_eq!(error.kind, ParseErrorKind::TruncatedBinaryData);
            assert_eq!(parser.syntax(), Syntax::default());
            assert!(parser.next().is_none());
        }
    }
}
