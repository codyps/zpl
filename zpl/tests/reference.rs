use zpl::parse::{Element, ParseContext, Syntax};

const INDEX: &str = include_str!("../../docs/zpl-command-index.tsv");

fn specimen(command: &str) -> String {
    let parameters = match command {
        "^A" => "0N,30,30",
        "^CC" | "~CC" => "^",
        "^CT" | "~CT" => "~",
        "^CD" | "~CD" => ",",
        "^GF" => "A,1,1,1,FF",
        "~DY" => "R:T,B,T,4,,^~\0\u{1}",
        "~DG" => "R:T,1,1,FF",
        "~DE" | "~DS" | "~DT" | "~DU" => "R:T,4,00000000",
        "~DB" => "R:T,N,1,8,1,8,1,TEST,#0041.1.8.0.0.8.FF",
        // Other operands do not affect framing; these are mnemonic-boundary
        // specimens, not valid parameter sets to execute on a physical printer.
        _ => "",
    };
    format!("{command}{parameters}")
}

#[test]
fn every_reference_command_has_a_lossless_boundary() {
    let mut count = 0;
    for row in INDEX.lines().filter(|row| !row.starts_with('#')) {
        let (command, page) = row.split_once('\t').unwrap();
        assert!(page.parse::<usize>().unwrap() > 0);
        count += 1;
        for syntax in [
            Syntax::default(),
            Syntax {
                format_prefix: b'/',
                control_prefix: b'!',
                delimiter: b';',
            },
        ] {
            let remap = |byte| match byte {
                b'^' => syntax.format_prefix,
                b'~' => syntax.control_prefix,
                b',' => syntax.delimiter,
                b => b,
            };
            let command_bytes: Vec<_> = specimen(command).bytes().map(remap).collect();
            let mut input = vec![syntax.format_prefix, b'X', b'A'];
            input.extend_from_slice(&command_bytes);
            input.extend_from_slice(&[syntax.format_prefix, b'X', b'Z']);
            let parts = ParseContext::with_syntax(&input, syntax)
                .collect::<Result<Vec<_>, _>>()
                .unwrap_or_else(|e| panic!("{command} (reference page {page}): {e}"));
            assert_eq!(parts.len(), 3, "{command}: {parts:?}");
            assert_eq!(parts[1].as_bytes(), command_bytes, "{command}");
            assert_eq!(
                matches!(parts[1], Element::FormatCommand(_)),
                command.starts_with('^')
            );
            let joined: Vec<_> = parts
                .iter()
                .flat_map(|p| p.as_bytes().iter().copied())
                .collect();
            assert_eq!(joined, input);
        }
    }
    assert_eq!(count, 224);
}
