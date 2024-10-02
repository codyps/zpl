use super::*;

#[test]
fn test_parse_prefixes_1() {
    let ctx = ParseContext::from_bytes(b"^CC&");
    let (skip, prefixes) = ctx.parse_prefixes(b"^CC&");
    assert_eq!(skip, 4);
    assert_eq!(prefixes.caret, b'&');
    assert_eq!(prefixes.tilde, b'~');
}

#[test]
fn test_parse_prefixes_2() {
    let ctx = ParseContext::from_bytes(b"~CT#");
    let (skip, prefixes) = ctx.parse_prefixes(b"~CT#");
    assert_eq!(skip, 4);
    assert_eq!(prefixes.caret, b'^');
    assert_eq!(prefixes.tilde, b'#');
}

#[test]
fn test_parse_prefixes_3() {
    let ctx = ParseContext::from_bytes(b"~CT#1234#CT~");
    let (skip, prefixes) = ctx.parse_prefixes(b"~CT#");
    assert_eq!(skip, 4);
    assert_eq!(prefixes.caret, b'^');
    assert_eq!(prefixes.tilde, b'#');
}

#[test]
fn test_scan_element_1() {
    let mut ctx = ParseContext::from_bytes(b"#XA");
    ctx.prefixes.caret = b'#';
    let element = ctx.scan_element();
    assert_eq!(
        element,
        Some((
            3,
            Prefixes {
                caret: b'#',
                tilde: b'~'
            },
            Element::FormatCommand(b"#XA")
        ))
    );
}

#[test]
fn next_element_1() {
    let mut ctx = ParseContext::from_bytes(b"#XA");
    ctx.prefixes.caret = b'#';
    let element = ctx.next_element().unwrap();
    assert_eq!(element, Some(Element::FormatCommand(b"#XA")));
}

#[test]
fn next_element_2() {
    let ctx = ParseContext::from_bytes(b"^XA~CC##XZ");
    let elements = vec![
        Element::FormatCommand(b"^XA"),
        Element::ControlCommand(b"~CC#"),
        Element::FormatCommand(b"#XZ"),
    ];
    let e = ctx.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(e, elements);
}
