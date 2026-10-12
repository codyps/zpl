//! Original SGD request parser. Parsing does not authorize or execute commands.
//! References: Zebra Programming Guide, SGD Command Structure; Link-OS Addendum
//! P1058886-001 pp. 25–26 (null gets, strings set, and the `{}` introducer).
//! https://docs.zebra.com/us/en/printers/software/zpl-pg/about-sgd-printer-commands.html
//! https://techdocs.zebra.com/link-os/2-13/webservices/content/link-os-pg-addendum-en.pdf
#![doc = include_str!("../README.md")]

use std::{error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Get { name: String },
    Set { name: String, value: String },
    Do { name: String, value: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for ParseError {}

fn error(message: &str) -> ParseError {
    ParseError(message.into())
}

/// Parse one complete classic SGD line, with or without its CRLF terminator.
/// Quoted values are literal SGD strings, not JSON escape sequences.
pub fn parse_sgd(input: &[u8]) -> Result<Command, ParseError> {
    let line = std::str::from_utf8(input).map_err(|_| error("SGD is not UTF-8"))?;
    let mut rest = line.trim_matches(|c| c == '\r' || c == '\n' || c == ' ' || c == '\t');
    fn word<'a>(rest: &mut &'a str) -> &'a str {
        let end = rest.find([' ', '\t']).unwrap_or(rest.len());
        let result = &rest[..end];
        *rest = rest[end..].trim_start_matches([' ', '\t']);
        result
    }
    if word(&mut rest) != "!" || word(&mut rest) != "U1" {
        return Err(error("expected ! U1"));
    }
    let verb = word(&mut rest);
    fn quoted(rest: &mut &str) -> Result<String, ParseError> {
        let text = rest
            .strip_prefix('"')
            .ok_or_else(|| error("expected quoted SGD operand"))?;
        let end = text
            .find('"')
            .ok_or_else(|| error("unterminated SGD operand"))?;
        let value = &text[..end];
        if value.chars().any(char::is_control) {
            return Err(error("control character in SGD operand"));
        }
        let tail = &text[end + 1..];
        if !tail.is_empty() && !tail.starts_with([' ', '\t']) {
            return Err(error("SGD operands require whitespace"));
        }
        *rest = tail.trim_start_matches([' ', '\t']);
        Ok(value.into())
    }
    let name = quoted(&mut rest)?;
    if name.is_empty() {
        return Err(error("empty SGD name"));
    }
    let command = match verb {
        "getvar" => Command::Get { name },
        "setvar" => Command::Set {
            name,
            value: quoted(&mut rest)?,
        },
        "do" => Command::Do {
            name,
            value: quoted(&mut rest)?,
        },
        _ => return Err(error("unknown SGD verb")),
    };
    if !rest.is_empty() {
        return Err(error("unexpected SGD operands"));
    }
    Ok(command)
}

// Keep insertion order and reject duplicate keys instead of silently dropping
// requests through serde_json::Value's map representation.
struct Entries(Vec<(String, Option<String>)>);
impl<'de> serde::Deserialize<'de> for Entries {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Entries;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an SGD object with string or null values")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Entries, A::Error> {
                let mut entries = Vec::new();
                let mut names = std::collections::HashSet::new();
                while let Some((name, value)) = map.next_entry::<String, Option<String>>()? {
                    if name.is_empty() || !names.insert(name.clone()) {
                        return Err(serde::de::Error::custom("empty or duplicate SGD key"));
                    }
                    entries.push((name, value));
                }
                Ok(Entries(entries))
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}

/// Frame the first `{}`-introduced SGD JSON object. `None` means more bytes
/// are needed. Consumed bytes exclude subsequent requests and whitespace.
/// Callers must bound buffering; JSON syntax errors are distinct from EOF.
pub fn parse_json_prefix(input: &[u8]) -> Result<Option<(Vec<Command>, usize)>, ParseError> {
    if input.len() < 2 && b"{}".starts_with(input) {
        return Ok(None);
    }
    let body = input
        .strip_prefix(b"{}")
        .ok_or_else(|| error("expected SGD JSON {} introducer"))?;
    let mut stream = serde_json::Deserializer::from_slice(body).into_iter::<Entries>();
    match stream.next() {
        None => Ok(None),
        Some(Err(e)) if e.is_eof() => Ok(None),
        Some(Err(e)) => Err(ParseError(e.to_string())),
        Some(Ok(Entries(entries))) => Ok(Some((
            entries
                .into_iter()
                .map(|(name, value)| match value {
                    None => Command::Get { name },
                    Some(value) => Command::Set { name, value },
                })
                .collect(),
            2 + stream.byte_offset(),
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classic_commands_and_literal_values() {
        assert_eq!(
            parse_sgd(b"! U1 getvar \"device.location\"\r\n").unwrap(),
            Command::Get {
                name: "device.location".into()
            }
        );
        assert_eq!(
            parse_sgd(br#"! U1 setvar "device.location" "C:\desk""#).unwrap(),
            Command::Set {
                name: "device.location".into(),
                value: "C:\\desk".into()
            }
        );
        assert!(matches!(
            parse_sgd(b"! U1 do \"device.reset\" \"\"").unwrap(),
            Command::Do { .. }
        ));
        for bad in [
            b"! U1 getvar x".as_slice(),
            b"! U1 getvar \"x\" \"y\"",
            b"! U1 setvar \"x\"",
            b"! U1 getvar \"x\"junk",
            b"! U1 do \"x\" \"a\nb\"",
        ] {
            assert!(parse_sgd(bad).is_err());
        }
    }
    #[test]
    fn json_fragmentation_batch_order_and_escaping() {
        // Link-OS Addendum pp. 25–26, examples 1–4, plus JSON string escaping.
        let input = br#"{}{"device.location":"desk\"A", "device.friendly_name":null}{}{"x":null}"#;
        let end = input.len() - br#"{}{"x":null}"#.len();
        for split in 0..end {
            assert!(parse_json_prefix(&input[..split]).unwrap().is_none());
        }
        let (commands, consumed) = parse_json_prefix(input).unwrap().unwrap();
        assert_eq!(consumed, end);
        assert_eq!(
            commands,
            vec![
                Command::Set {
                    name: "device.location".into(),
                    value: "desk\"A".into()
                },
                Command::Get {
                    name: "device.friendly_name".into()
                }
            ]
        );
        assert!(parse_json_prefix(&input[consumed..]).unwrap().is_some());
    }
    #[test]
    fn json_rejects_invalid_types_duplicates_and_syntax() {
        for input in [
            br#"{}{"x":1}"#.as_slice(),
            br#"{}{"x":null,"x":"a"}"#,
            br#"{}{"":null}"#,
            br#"{}[]"#,
            br#"{}{"x":false}"#,
            br#"{}{"x":!}"#,
        ] {
            assert!(parse_json_prefix(input).is_err(), "{input:?}");
        }
    }
}
