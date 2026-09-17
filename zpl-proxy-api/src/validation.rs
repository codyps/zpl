//! Admission policy for untrusted printer previews, separate from lossless framing.
//!
//! References: Zebra ZPL II Programming Guide, command-specific sections:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
//! In the bundled guide: ^FD/^FH pp. 190/193, ^GF p. 215, ^MC p. 300,
//! ^WD p. 361, ^XF/^XG pp. 372–373. Framing success is not authorization.

use std::{error::Error, fmt};
use zpl::parse::{Element, ParseContext};

#[derive(Debug)]
pub struct ValidationError {
    pub offset: usize,
    pub reason: &'static str,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ZPL byte {}: {}", self.offset, self.reason)
    }
}
impl Error for ValidationError {}

/// Only this privately constructed type can cross the proxy's cache/printer boundary.
pub struct RenderZpl(String);

impl RenderZpl {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parse(input: String) -> Result<Self, ValidationError> {
        let fail = |offset, reason| ValidationError { offset, reason };
        if input.len() > 1_048_576 {
            return Err(fail(0, "label exceeds 1 MiB"));
        }
        // Reject transport/control-code alternatives and other printer languages.
        // CR/LF/TAB are permitted only where each command's grammar admits them.
        if let Some(offset) = input.bytes().position(|b| {
            (b.is_ascii_control() && !matches!(b, b'\r' | b'\n' | b'\t')) || b == 0x7f
        }) {
            return Err(fail(offset, "control characters are not allowed"));
        }
        let mut parser = ParseContext::from_bytes(input.as_bytes());
        let mut opened = false;
        let mut closed = false;
        let mut field = false;
        let mut hex_indicator = None;
        let mut graphic_bytes = 0;
        loop {
            let offset = parser.position();
            let Some(element) = parser.next() else { break };
            let element = element.map_err(|_| fail(offset, "cannot parse command framing"))?;
            let raw = match element {
                Element::BeforeFirstCommand(bytes) if whitespace(bytes) => continue,
                Element::FormatCommand(raw) => raw,
                _ => return Err(fail(offset, "only standard ^ format commands are allowed")),
            };
            let Some(code) = raw.get(1..3) else {
                return Err(fail(offset, "incomplete command"));
            };
            // Never apply syntax changes or trust unknown/binary command framing.
            if matches!(code, b"CC" | b"CT" | b"CD") {
                return Err(fail(offset, "syntax changes are not allowed"));
            }
            let data = raw[3..].trim_ascii_end();
            match code {
                b"XA" if !opened && !closed && data.is_empty() => opened = true,
                b"XZ" if opened && !closed && !field && data.is_empty() => closed = true,
                b"XA" | b"XZ" => {
                    return Err(fail(
                        offset,
                        "expected one complete label with terminated fields",
                    ));
                }
                _ if !opened || closed => {
                    return Err(fail(offset, "command outside label"));
                }
                b"FS" if data.is_empty() => {
                    field = false;
                    hex_indicator = None;
                }
                b"FH" => {
                    if data.len() > 1 || data.first().is_some_and(|b| !b.is_ascii_graphic()) {
                        return Err(fail(offset, "invalid field hex indicator"));
                    }
                    hex_indicator = Some(data.first().copied().unwrap_or(b'_'));
                }
                b"FD" | b"FV" => {
                    // Do not discard field bytes or accept embedded SGD/control lines.
                    let data = &raw[3..];
                    if field || data.iter().any(u8::is_ascii_control) {
                        return Err(fail(offset, "invalid or unterminated field data"));
                    }
                    validate_hex(data, hex_indicator).map_err(|reason| fail(offset, reason))?;
                    field = true;
                }
                b"FX" => {
                    if data.iter().any(u8::is_ascii_control) {
                        return Err(fail(offset, "comments must be single-line text"));
                    }
                }
                b"GF" => {
                    if field {
                        return Err(fail(offset, "field must end with ^FS"));
                    }
                    graphic_bytes +=
                        validate_graphic(data).map_err(|reason| fail(offset, reason))?;
                    if graphic_bytes > 250_000 {
                        return Err(fail(offset, "total graphics exceed 250000 decoded bytes"));
                    }
                    field = true;
                }
                b"CI" if matches!(data, b"0" | b"27" | b"28") => {}
                _ => {
                    let Some(schema) = schema(code) else {
                        return Err(fail(offset, "command is not allowed for rendering"));
                    };
                    validate_parameters(data, schema).map_err(|reason| fail(offset, reason))?;
                    if matches!(code, b"GB" | b"GC" | b"GD" | b"GE") {
                        if field {
                            return Err(fail(offset, "field must end with ^FS"));
                        }
                        field = true;
                    }
                }
            }
        }
        if !closed {
            return Err(fail(input.len(), "expected one complete ^XA…^XZ label"));
        }
        Ok(Self(input))
    }
}

fn whitespace(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .all(|b| matches!(b, b' ' | b'\r' | b'\n' | b'\t'))
}

#[derive(Clone, Copy)]
enum Parameter {
    Integer(i32, i32),
    Choice(&'static [u8]),
    Ratio,
}
use Parameter::{Choice as C, Integer as I, Ratio};
const DOTS: Parameter = I(0, 32000);
const ORIENTATION: Parameter = C(b"NRIB");
const YES_NO: Parameter = C(b"YN");
const COLOR: Parameter = C(b"BW");
const FONT: Parameter = C(b"0ABCDEFGH");

// This positive list is deliberately independent of the local renderer. In
// particular, rendering stored objects (^XG/^XF/^IM/^IL/^A@/^WD) is forbidden.
// New mnemonics/parameter modes need an explicit policy review, not a wildcard.
fn schema(code: &[u8]) -> Option<&'static [Parameter]> {
    Some(match code {
        b"A0" | b"AA" | b"AB" | b"AC" | b"AD" | b"AE" | b"AF" | b"AG" | b"AH" => {
            &[ORIENTATION, DOTS, DOTS]
        }
        b"CF" => &[FONT, DOTS, DOTS],
        b"FO" | b"FT" => &[DOTS, DOTS, I(0, 2)],
        b"LH" => &[DOTS, DOTS],
        b"PW" | b"LL" => &[I(1, 32000)],
        b"LS" | b"LT" => &[I(-32000, 32000)],
        b"FW" => &[ORIENTATION, I(0, 2)],
        b"PO" => &[C(b"NI")],
        b"PM" | b"LR" => &[YES_NO],
        b"FR" => &[],
        b"FB" => &[DOTS, I(1, 9999), I(-9999, 9999), C(b"LCRJ"), DOTS],
        b"FP" => &[C(b"HV"), I(0, 9999)],
        b"GB" => &[DOTS, DOTS, DOTS, COLOR, I(0, 8)],
        b"GC" => &[DOTS, DOTS, COLOR],
        b"GD" => &[DOTS, DOTS, DOTS, COLOR, C(b"LR")],
        b"GE" => &[DOTS, DOTS, DOTS, COLOR],
        b"GS" => &[ORIENTATION, DOTS, DOTS],
        b"BY" => &[I(1, 10), Ratio, I(1, 32000)],
        b"BC" => &[ORIENTATION, DOTS, YES_NO, YES_NO, YES_NO, C(b"NUAD")],
        b"B1" | b"B3" => &[ORIENTATION, YES_NO, DOTS, YES_NO, YES_NO],
        b"B2" => &[ORIENTATION, DOTS, YES_NO, YES_NO, YES_NO],
        b"B8" | b"BE" => &[ORIENTATION, DOTS, YES_NO, YES_NO],
        b"BU" | b"B9" => &[ORIENTATION, DOTS, YES_NO, YES_NO, YES_NO],
        b"BA" => &[ORIENTATION, DOTS, YES_NO, YES_NO, YES_NO],
        b"B7" => &[
            ORIENTATION,
            I(1, 32000),
            I(0, 8),
            I(1, 30),
            I(3, 90),
            YES_NO,
        ],
        b"BQ" => &[ORIENTATION, I(1, 2), I(1, 10), C(b"HQLM"), I(0, 7)],
        b"BX" => &[
            ORIENTATION,
            I(1, 32000),
            I(0, 200),
            I(0, 144),
            I(0, 144),
            I(1, 6),
            C(b"_"),
            I(1, 2),
        ],
        _ => return None,
    })
}

fn validate_parameters(data: &[u8], schema: &[Parameter]) -> Result<(), &'static str> {
    if data.is_empty() {
        return Ok(()); // Omitted parameters use the documented command defaults.
    }
    let parameters: Vec<_> = data.split(|b| *b == b',').collect();
    if parameters.len() > schema.len() {
        return Err("too many command parameters");
    }
    for (value, kind) in parameters.iter().zip(schema) {
        if value.is_empty() {
            continue;
        }
        let valid = match kind {
            I(min, max) => {
                let digits = value.strip_prefix(b"-").unwrap_or(value);
                !digits.is_empty()
                    && digits.iter().all(u8::is_ascii_digit)
                    && std::str::from_utf8(value)
                        .ok()
                        .and_then(|v| v.parse::<i32>().ok())
                        .is_some_and(|v| (*min..=*max).contains(&v))
            }
            C(choices) => value.len() == 1 && choices.contains(&value[0]),
            Ratio => matches!(
                *value,
                b"2" | b"3"
                    | b"2.0"
                    | b"2.1"
                    | b"2.2"
                    | b"2.3"
                    | b"2.4"
                    | b"2.5"
                    | b"2.6"
                    | b"2.7"
                    | b"2.8"
                    | b"2.9"
                    | b"3.0"
            ),
        };
        if !valid {
            return Err("invalid or unsupported command parameter");
        }
    }
    Ok(())
}

fn validate_hex(mut data: &[u8], indicator: Option<u8>) -> Result<(), &'static str> {
    let Some(indicator) = indicator else {
        return Ok(());
    };
    while let Some(index) = data.iter().position(|b| *b == indicator) {
        let digits = data
            .get(index + 1..index + 3)
            .ok_or("incomplete field hex escape")?;
        if !digits.iter().all(u8::is_ascii_hexdigit) {
            return Err("invalid field hex escape");
        }
        // ^FH translates field bytes, not executable command bytes (guide p.193).
        data = &data[index + 3..];
    }
    Ok(())
}

fn validate_graphic(data: &[u8]) -> Result<usize, &'static str> {
    let parts: Vec<_> = data.splitn(5, |b| *b == b',').collect();
    if parts.len() != 5 || parts[0] != b"A" {
        return Err("only ASCII ^GFA inline graphics are allowed");
    }
    let count = |part: &[u8]| {
        if part.is_empty() || !part.iter().all(u8::is_ascii_digit) {
            return None;
        }
        std::str::from_utf8(part).ok()?.parse::<usize>().ok()
    };
    let bytes = count(parts[1]).ok_or("invalid graphic size")?;
    let decoded = count(parts[2]).ok_or("invalid graphic size")?;
    let row = count(parts[3]).ok_or("invalid graphic row size")?;
    if bytes != decoded || bytes == 0 || bytes > 25_000 || row == 0 || !bytes.is_multiple_of(row) {
        return Err("invalid or excessive graphic dimensions");
    }
    // Reuse the bounded graphic decoder's size, RLE, base64, CRC, and zlib checks.
    // Only this isolated, already framed GFA command is rendered, never the user's
    // whole stream (the renderer itself also permits stored-object commands).
    // Raw prefixes must not hide inside an encoded envelope or malformed payload.
    if data.iter().any(|b| matches!(b, b'^' | b'~')) {
        return Err("command prefix inside graphic data");
    }
    let mut label = b"^XA^GF".to_vec();
    label.extend_from_slice(data);
    label.extend_from_slice(b"^FS^XZ");
    zpl::render(&label, zpl::Options::default()).map_err(|_| "invalid ASCII graphic data")?;
    Ok(bytes)
}

#[cfg(test)]
mod tests;
