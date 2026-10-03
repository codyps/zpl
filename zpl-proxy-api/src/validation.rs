//! Admission policy for untrusted printer previews, separate from lossless framing.
//!
//! References: Zebra ZPL II Programming Guide, command-specific sections:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
//! In the bundled guide: ^FD/^FH pp. 190/193, ^GF p. 215, ^MC p. 300,
//! ^WD p. 361, ^XF/^XG pp. 372–373. Framing success is not authorization.

use serde::Deserialize;
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

/// Operator-selected policy for a named preview endpoint. Requests cannot override it.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionPolicy {
    #[default]
    Restricted,
    Unrestricted,
}

/// A restricted rendering stream admitted independently of caching and recovery.
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
                _ => {
                    if !render_command(code) {
                        return Err(fail(offset, "command is not allowed for rendering"));
                    }
                    // Validate the command boundary, not firmware-specific operand
                    // ranges, enum values or counts. No embedded control/SGD lines
                    // or prefixes swallowed by a special parser rule (notably BX).
                    if data
                        .iter()
                        .any(|b| b.is_ascii_control() || matches!(b, b'^' | b'~'))
                    {
                        return Err(fail(
                            offset,
                            "control characters or prefixes in command parameters",
                        ));
                    }
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

// Positive list independent of the local renderer. Zebra Programming Guide:
// ^A/^A@ pp.60-62; barcodes pp.64-150; ^CI pp.155-159; fields pp.186-209;
// ^IL/^IM pp.247-248; ^PA p.315; ^SF/^SN pp.335/341; ^TB p.356; ^XG p.373.
// Stored-format recall (^XF) can execute commands and is not a read-only image.
fn render_command(code: &[u8]) -> bool {
    matches!(code, [b'A', b'0'..=b'9' | b'A'..=b'Z' | b'@'])
        || matches!(
            code,
            b"CF"
                | b"CI"
                | b"FO"
                | b"FT"
                | b"FM"
                | b"LH"
                | b"PW"
                | b"LL"
                | b"LS"
                | b"LT"
                | b"FW"
                | b"PO"
                | b"PM"
                | b"LR"
                | b"FR"
                | b"FB"
                | b"FP"
                | b"PA"
                | b"TB"
                | b"FC"
                | b"FE"
                | b"FN"
                | b"SN"
                | b"SF"
                | b"GB"
                | b"GC"
                | b"GD"
                | b"GE"
                | b"GS"
                | b"XG"
                | b"IL"
                | b"IM"
                | b"BY"
                | b"B0"
                | b"B1"
                | b"B2"
                | b"B3"
                | b"B4"
                | b"B5"
                | b"B7"
                | b"B8"
                | b"B9"
                | b"BA"
                | b"BB"
                | b"BC"
                | b"BD"
                | b"BE"
                | b"BF"
                | b"BI"
                | b"BJ"
                | b"BK"
                | b"BL"
                | b"BM"
                | b"BO"
                | b"BP"
                | b"BQ"
                | b"BR"
                | b"BS"
                | b"BT"
                | b"BU"
                | b"BX"
                | b"BZ"
        )
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
