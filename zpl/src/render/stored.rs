//! Request-local format storage, Zebra Programming Guide ^DF and ^XF:
//! https://docs.zebra.com/us/en/printers/software/zpl-pg/c-zpl-zpl-commands/r-zpl-df.html
//! https://docs.zebra.com/us/en/printers/software/zpl-pg/c-zpl-zpl-commands/r-zpl-xf.html
//! Expand command tokens before numbered-field planning. No filesystem/device IO.
use super::RenderError;
use crate::parse::{Element, ParseContext, Syntax};
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Token<'a> {
    raw: &'a [u8],
    name: &'a [u8],
    syntax: Syntax,
    offset: usize,
}
pub(super) struct Expanded {
    pub bytes: Vec<u8>,
    origins: Vec<(usize, usize)>,
}
impl Expanded {
    pub fn original_offset(&self, offset: usize) -> usize {
        self.origins
            .iter()
            .rev()
            .find(|(start, _)| *start <= offset)
            .map_or(offset, |(_, source)| *source)
    }
    fn push(&mut self, token: Token<'_>) -> Result<(), RenderError> {
        if self.bytes.len() + token.raw.len() > 1_048_576 {
            return Err(error(
                token.offset,
                "expanded formats exceed 1 MiB renderer limit",
            ));
        }
        self.origins.push((self.bytes.len(), token.offset));
        self.bytes.extend_from_slice(token.raw);
        Ok(())
    }
}
fn error(offset: usize, message: impl Into<String>) -> RenderError {
    RenderError {
        offset,
        message: message.into(),
    }
}
fn name(token: Token<'_>, download: bool) -> Result<(Option<u8>, String), RenderError> {
    let text = std::str::from_utf8(&token.raw[3..])
        .map_err(|_| error(token.offset, "invalid stored format name"))?
        .trim_end_matches(['\r', '\n']);
    let (device, rest) = if let Some((device, rest)) = text.split_once(':') {
        if device.len() != 1 || !b"REBA".contains(&device.as_bytes()[0]) {
            return Err(error(token.offset, "unsupported stored format device"));
        }
        (Some(device.as_bytes()[0]), rest)
    } else {
        (download.then_some(b'R'), text)
    };
    let base = rest.strip_suffix(".ZPL").unwrap_or(rest);
    let base = if base.is_empty() { "UNKNOWN" } else { base };
    if base.len() > 16 || !base.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return Err(error(
            token.offset,
            "unsupported stored format name or extension",
        ));
    }
    Ok((device, base.into()))
}

pub(super) fn expand(input: &[u8]) -> Result<Option<Expanded>, RenderError> {
    if !input.windows(2).any(|b| matches!(b, b"DF" | b"XF")) {
        return Ok(None);
    }
    let mut parser = ParseContext::from_bytes(input);
    let mut tokens = Vec::new();
    loop {
        let (offset, syntax) = (parser.position(), parser.syntax());
        let Some(item) = parser.next() else { break };
        let item = item.map_err(|e| error(offset, e.to_string()))?;
        let raw = item.as_bytes();
        let name = match item {
            Element::FormatCommand(_) => &raw[1..3],
            Element::ControlCharacter(_) => match raw[0] {
                2 => &b"XA"[..],
                3 => &b"XZ"[..],
                _ => &b""[..],
            },
            _ => b"",
        };
        tokens.push(Token {
            raw,
            name,
            syntax,
            offset,
        });
    }
    if !tokens.iter().any(|t| matches!(t.name, b"DF" | b"XF")) {
        return Ok(None);
    }
    let mut output = Expanded {
        bytes: Vec::new(),
        origins: Vec::new(),
    };
    let mut formats: HashMap<(u8, String), Vec<Token<'_>>> = HashMap::new();
    let mut i = 0;
    let mut calls = 0;
    let mut in_label = false;
    while i < tokens.len() {
        let token = tokens[i];
        if token.name == b"XA" {
            if in_label {
                return Err(error(token.offset, "nested label start"));
            }
            let end = tokens[i + 1..]
                .iter()
                .position(|t| matches!(t.name, b"XA" | b"XZ"))
                .map(|n| i + 1 + n);
            if let Some(end) = end.filter(|&end| tokens[end].name == b"XZ") {
                if let Some(df) = tokens[i + 1..end]
                    .iter()
                    .position(|t| t.name == b"DF")
                    .map(|n| i + 1 + n)
                {
                    for boundary in [token, tokens[end]] {
                        if boundary.raw.len() >= 3
                            && !boundary.raw[3..].iter().all(|b| matches!(b, b'\r' | b'\n'))
                        {
                            return Err(error(
                                boundary.offset,
                                "unexpected stored format boundary parameters",
                            ));
                        }
                    }
                    let (device, key) = name(tokens[df], true)?;
                    // PW/LL before DF configure the printer, as in the external
                    // fixture. Drawing before DF and nested definitions need
                    // separate semantics and are explicitly rejected.
                    for &prefix in &tokens[i + 1..df] {
                        if !matches!(prefix.name, b"PW" | b"LL" | b"FX") {
                            return Err(error(prefix.offset, "unsupported command before DF"));
                        }
                        output.push(prefix)?;
                    }
                    if tokens[df + 1..end].iter().any(|t| {
                        matches!(t.name, b"DF" | b"CC" | b"CT" | b"CD")
                            || (t
                                .raw
                                .get(1..3)
                                .is_some_and(|n| matches!(n, b"CC" | b"CT" | b"CD")))
                    }) {
                        return Err(error(
                            tokens[df].offset,
                            "nested definitions or syntax changes in stored formats unsupported",
                        ));
                    }
                    if formats.len() >= 256
                        && !formats.contains_key(&(device.unwrap(), key.clone()))
                    {
                        return Err(error(
                            tokens[df].offset,
                            "stored formats exceed 256-object renderer limit",
                        ));
                    }
                    formats.insert((device.unwrap(), key), tokens[df + 1..end].to_vec());
                    i = end + 1;
                    continue;
                }
            }
        }
        if token.name == b"XF" && !in_label {
            return Err(error(token.offset, "stored format recall outside label"));
        }
        if token.name == b"XA" {
            in_label = true;
        }
        if token.name == b"XZ" {
            in_label = false;
        }
        recall(token, &formats, &mut output, 0, &mut calls)?;
        i += 1;
    }
    Ok(Some(output))
}
fn recall(
    token: Token<'_>,
    formats: &HashMap<(u8, String), Vec<Token<'_>>>,
    output: &mut Expanded,
    depth: usize,
    calls: &mut usize,
) -> Result<(), RenderError> {
    if token.name != b"XF" {
        return output.push(token);
    }
    *calls += 1;
    if depth >= 8 || *calls > 4096 {
        return Err(error(token.offset, "stored format recall limit exceeded"));
    }
    let (device, key) = name(token, false)?;
    let body = b"REBA"
        .iter()
        .filter(|&&d| device.is_none_or(|wanted| wanted == d))
        .find_map(|&d| formats.get(&(d, key.clone())))
        .ok_or_else(|| {
            error(
                token.offset,
                format!("stored format {key:?} not found in this render request"),
            )
        })?;
    for &command in body {
        if command.syntax != token.syntax {
            return Err(error(
                token.offset,
                "stored format recall with different command syntax unsupported",
            ));
        }
        recall(command, formats, output, depth + 1, calls)?;
    }
    Ok(())
}
