//! TCP reads are not printer requests. Frame SGD lines, JSON objects, and complete
//! ZPL formats using the existing binary-aware parser, never searching raw ^XZ.
use zebra_sgd_parser::parse_json_prefix;
use zpl::parse::{Element, ParseContext};

pub fn frame_end(input: &[u8]) -> Result<Option<usize>, String> {
    if input.is_empty() {
        return Ok(None);
    }
    if input[0] == b'!' {
        return Ok(input.iter().position(|&b| b == b'\n').map(|p| p + 1));
    }
    if input[0] == b'{' {
        return parse_json_prefix(input)
            .map(|result| result.map(|(_, end)| end))
            .map_err(|e| e.to_string());
    }
    if input.len() >= 3 && crate::printer::is_control(&input[..3]) {
        return Ok(Some(3));
    }
    let mut parser = ParseContext::from_bytes(input);
    let mut in_label = false;
    while let Some(element) = parser.next() {
        let element = match element {
            Ok(e) => e,
            Err(_) => return Ok(None),
        }; // a fragmented binary payload may be incomplete
        let bytes = element.as_bytes();
        match element {
            Element::FormatCommand(_) | Element::ControlCommand(_) => match bytes.get(1..3) {
                Some(b"XA") => in_label = true,
                Some(b"XZ") if in_label => {
                    // Commands include intervening whitespace; preserve only
                    // the three-byte terminator, leaving the following stream.
                    return Ok(Some(parser.position() - bytes.len() + 3));
                }
                _ => {}
            },
            Element::ControlCharacter(_) if bytes[0] == 2 => in_label = true,
            Element::ControlCharacter(_) if bytes[0] == 3 && in_label => {
                return Ok(Some(parser.position()))
            }
            _ => {}
        }
    }
    // Unsupported standalone traffic is still recorded. Do not split a label
    // on newlines, including binary/field data or a partial terminator.
    if !in_label && !input.starts_with(b"^") && !input.starts_with(b"~") {
        return Ok(input.iter().position(|&b| b == b'\n').map(|p| p + 1));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragmented_coalesced_and_binary_formats() {
        let label = b"^XA^FO2,2^GB10,10,2^FS^XZ";
        for end in 0..label.len() {
            assert_eq!(frame_end(&label[..end]).unwrap(), None);
        }
        let joined = [label.as_slice(), b"! U1 getvar \"appl.name\"\r\n"].concat();
        assert_eq!(frame_end(&joined).unwrap(), Some(label.len()));
        // ^GFB's byte count owns the embedded ^XZ, not the TCP framer.
        let binary = b"^XA^GFB,3,3,1,^XZ^FS^XZ";
        assert_eq!(frame_end(binary).unwrap(), Some(binary.len()));
        assert_eq!(frame_end(b"^XA^FDa\nb^FS").unwrap(), None);
        assert_eq!(frame_end(b"~HS~HI").unwrap(), Some(3));
        assert_eq!(
            frame_end(b"\x02^FO1,1^GB2,2,1^FS\x03").unwrap(),
            Some(b"\x02^FO1,1^GB2,2,1^FS\x03".len())
        );
    }
}
