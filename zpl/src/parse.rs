//! Lossless framing of complete ZPL command streams.
//!
//! Parameters remain raw bytes. Prefix/delimiter changes, prefix-valued ^BX escape
//! operands and download framing are interpreted; this is not a validator or a printer emulator.
//! See `docs/parser-coverage.md` for the reference and coverage contract.

use std::{error::Error, fmt, iter::FusedIterator};

#[cfg(test)]
mod test;

/// Syntax already configured on the printer, or left by a preceding stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Syntax {
    pub format_prefix: u8,
    pub control_prefix: u8,
    pub delimiter: u8,
}

impl Default for Syntax {
    fn default() -> Self {
        Self {
            format_prefix: b'^',
            control_prefix: b'~',
            delimiter: b',',
        }
    }
}

impl Syntax {
    fn is_prefix(self, byte: u8) -> bool {
        byte == self.format_prefix || byte == self.control_prefix
    }

    fn is_boundary(self, byte: u8) -> bool {
        self.is_prefix(byte) || is_control_character(byte)
    }
}

/// A borrowed, unmodified part of the input, including intervening whitespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element<'a> {
    BeforeFirstCommand(&'a [u8]),
    FormatCommand(&'a [u8]),
    ControlCommand(&'a [u8]),
    /// STX (0x02) = ^XA, ETX (0x03) = ^XZ, SI (0x0f) = ^FS.
    ControlCharacter(&'a [u8]),
}

impl<'a> Element<'a> {
    pub fn as_bytes(&self) -> &'a [u8] {
        match self {
            Self::BeforeFirstCommand(bytes)
            | Self::FormatCommand(bytes)
            | Self::ControlCommand(bytes)
            | Self::ControlCharacter(bytes) => bytes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorKind {
    IncompleteCommand,
    MissingSyntaxCharacter,
    InvalidBinaryHeader,
    InvalidBinaryLength,
    TruncatedBinaryData,
    InvalidEncodedData,
    IncompleteEncodedData,
}

/// A framing failure. The offset identifies the command whose boundary is unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseError {
    pub offset: usize,
    pub kind: ParseErrorKind,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ZPL framing error at byte {}: {:?}",
            self.offset, self.kind
        )
    }
}

impl Error for ParseError {}

/// An iterator over a complete byte buffer. On a framing error it stops rather
/// than interpreting the remainder of a binary download as executable commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseContext<'a> {
    syntax: Syntax,
    input: &'a [u8],
    position: usize,
    failed: bool,
}

impl<'a> ParseContext<'a> {
    pub fn from_bytes(input: &'a [u8]) -> Self {
        Self::with_syntax(input, Syntax::default())
    }

    pub fn with_syntax(input: &'a [u8], syntax: Syntax) -> Self {
        Self {
            syntax,
            input,
            position: 0,
            failed: false,
        }
    }

    pub fn syntax(&self) -> Syntax {
        self.syntax
    }

    /// Offset of the next element, or of the command that failed to frame.
    pub fn position(&self) -> usize {
        self.position
    }

    fn error(&self, kind: ParseErrorKind) -> ParseError {
        ParseError {
            offset: self.position,
            kind,
        }
    }

    fn next_boundary(&self, start: usize, syntax: Syntax) -> usize {
        self.input[start..]
            .iter()
            .position(|&byte| syntax.is_boundary(byte))
            .map_or(self.input.len(), |offset| start + offset)
    }

    /// Read a fixed number of delimited header fields without entering a later
    /// command. Delimiters take precedence when a custom delimiter is a prefix.
    fn fields<const N: usize>(&self, start: usize) -> Option<([&'a [u8]; N], usize)> {
        let mut fields = [&[][..]; N];
        let mut cursor = start;
        for field in &mut fields {
            let begin = cursor;
            loop {
                let &byte = self.input.get(cursor)?;
                if byte == self.syntax.delimiter {
                    *field = &self.input[begin..cursor];
                    cursor += 1;
                    break;
                }
                if self.syntax.is_boundary(byte) {
                    return None;
                }
                cursor += 1;
            }
        }
        Some((fields, cursor))
    }

    fn binary_end(&self, start: usize, length: &[u8]) -> Result<usize, ParseError> {
        // Do not allocate from an untrusted length, or let overflow wrap around.
        let length = trim_ascii(length);
        if length.is_empty() || !length.iter().all(u8::is_ascii_digit) {
            return Err(self.error(ParseErrorKind::InvalidBinaryLength));
        }
        let count = length.iter().try_fold(0usize, |count, digit| {
            count.checked_mul(10)?.checked_add((digit - b'0') as usize)
        });
        let count = count.ok_or_else(|| self.error(ParseErrorKind::InvalidBinaryLength))?;
        let end = start
            .checked_add(count)
            .filter(|&end| end <= self.input.len())
            .ok_or_else(|| self.error(ParseErrorKind::TruncatedBinaryData))?;
        Ok(end)
    }

    /// ZB64 carries its own terminator and four hex CRC digits. Its declared
    /// download length is the decoded size, not the number of transmitted bytes.
    fn encoded_end(&self, start: usize) -> Result<Option<usize>, ParseError> {
        if !self.input[start..].starts_with(b":B64:") && !self.input[start..].starts_with(b":Z64:")
        {
            return Ok(None);
        }
        let mut cursor = start + 5;
        loop {
            match self.input.get(cursor) {
                Some(b':') => break,
                Some(byte)
                    if byte.is_ascii_alphanumeric()
                        || matches!(byte, b'+' | b'/' | b'=' | b'\r' | b'\n') =>
                {
                    cursor += 1;
                }
                Some(_) => return Err(self.error(ParseErrorKind::InvalidEncodedData)),
                None => return Err(self.error(ParseErrorKind::IncompleteEncodedData)),
            }
        }
        let crc = self
            .input
            .get(cursor + 1..cursor + 5)
            .ok_or_else(|| self.error(ParseErrorKind::IncompleteEncodedData))?;
        if !crc.iter().all(u8::is_ascii_hexdigit) {
            return Err(self.error(ParseErrorKind::InvalidEncodedData));
        }
        Ok(Some(cursor + 5))
    }

    fn ascii_download_end(&self, data: usize, per_character: bool) -> Result<usize, ParseError> {
        let mut cursor = data;
        // The reference permits CR/LF formatting inside download data.
        while self
            .input
            .get(cursor)
            .is_some_and(|b| matches!(b, b'\r' | b'\n'))
        {
            cursor += 1;
        }
        if let Some(end) = self.encoded_end(cursor)? {
            return Ok(self.next_boundary(end, self.syntax));
        }
        if per_character {
            // ~DB encodes each glyph separately; ~DL is also listed as accepting
            // ZB64. Keep encoded subfields intact without decoding their contents.
            while cursor < self.input.len() {
                if let Some(end) = self.encoded_end(cursor)? {
                    cursor = end;
                } else if self.syntax.is_boundary(self.input[cursor]) {
                    return Ok(cursor);
                } else {
                    cursor += 1;
                }
            }
            return Ok(cursor);
        }
        Ok(self.next_boundary(data, self.syntax))
    }

    fn command_end(&self, code: &[u8], format: bool) -> Result<usize, ParseError> {
        let start = self.position + 3;
        if format && code == b"GF" {
            let mode = self.input.get(start).copied().unwrap_or(b'A');
            let fields = if mode == self.syntax.delimiter {
                self.fields::<3>(start + 1)
            } else if matches!(mode, b'A' | b'B' | b'C')
                && self.input.get(start + 1) == Some(&self.syntax.delimiter)
            {
                self.fields::<3>(start + 2)
            } else {
                None
            };
            if matches!(mode, b'B' | b'C') {
                let (fields, data) =
                    fields.ok_or_else(|| self.error(ParseErrorKind::InvalidBinaryHeader))?;
                let end = self.binary_end(data, fields[0])?;
                return Ok(self.next_boundary(end, self.syntax));
            }
            if let Some((_, data)) = fields {
                return self.ascii_download_end(data, false);
            }
        } else if !format && code == b"DY" {
            if let Some((_, mode_start)) = self.fields::<1>(start) {
                if matches!(
                    self.input[mode_start..]
                        .iter()
                        .find(|b| !b.is_ascii_whitespace()),
                    Some(b'B' | b'C')
                ) {
                    let (fields, data) = self
                        .fields::<5>(start)
                        .ok_or_else(|| self.error(ParseErrorKind::InvalidBinaryHeader))?;
                    let end = self.binary_end(data, fields[3])?;
                    return Ok(self.next_boundary(end, self.syntax));
                }
            }
            if let Some((_, data)) = self.fields::<5>(start) {
                return self.ascii_download_end(data, false);
            }
        } else if !format {
            let data = match code {
                b"DE" | b"DS" | b"DT" | b"DU" => self.fields::<2>(start).map(|(_, p)| p),
                b"DG" => self.fields::<3>(start).map(|(_, p)| p),
                b"DB" => self.fields::<8>(start).map(|(_, p)| p),
                b"DL" => Some(start),
                _ => None,
            };
            if let Some(data) = data {
                return self.ascii_download_end(data, matches!(code, b"DB" | b"DL"));
            }
        }
        // ^BX's seventh operand is a single escape character (Zebra guide
        // pp. 145–147). A literal prefix immediately before a delimiter or
        // the next command belongs to that operand, e.g. ...,6,~^FD.
        // Do not consume an omitted operand's following ^FS/~HS mnemonic.
        if format && code == b"BX" {
            if let Some((_, escape)) = self.fields::<6>(start) {
                if self
                    .input
                    .get(escape)
                    .is_some_and(|&b| self.syntax.is_prefix(b))
                    && self.input.get(escape + 1).is_none_or(|&b| {
                        self.syntax.is_prefix(b)
                            || b == self.syntax.delimiter
                            || b.is_ascii_whitespace()
                    })
                {
                    return Ok(self.next_boundary(escape + 1, self.syntax));
                }
            }
        }
        // Always skip the mnemonic, even when its letters match a custom prefix.
        Ok(self.next_boundary(start, self.syntax))
    }

    fn scan_element(&mut self) -> Result<Option<Element<'a>>, ParseError> {
        if self.failed || self.position == self.input.len() {
            return Ok(None);
        }
        let start = self.position;
        let first = self.input[start];
        if is_control_character(first) {
            self.position += 1;
            return Ok(Some(Element::ControlCharacter(
                &self.input[start..self.position],
            )));
        }
        if !self.syntax.is_prefix(first) {
            self.position = self.next_boundary(start, self.syntax);
            return Ok(Some(Element::BeforeFirstCommand(
                &self.input[start..self.position],
            )));
        }
        let format = first == self.syntax.format_prefix;
        // ^A is the one-letter font command; its font/size operands may be
        // omitted. ^A@ and ^A<font> are retained as the same raw command slice.
        if format
            && self.input.get(start + 1) == Some(&b'A')
            && self
                .input
                .get(start + 2)
                .is_none_or(|&b| self.syntax.is_boundary(b))
        {
            self.position = self.next_boundary(start + 2, self.syntax);
            return Ok(Some(Element::FormatCommand(
                &self.input[start..self.position],
            )));
        }
        let code = self
            .input
            .get(start + 1..start + 3)
            .ok_or_else(|| self.error(ParseErrorKind::IncompleteCommand))?;
        let end = if matches!(code, b"CC" | b"CT" | b"CD") {
            let &character = self
                .input
                .get(start + 3)
                .ok_or_else(|| self.error(ParseErrorKind::MissingSyntaxCharacter))?;
            let mut syntax = self.syntax;
            match code {
                b"CC" => syntax.format_prefix = character,
                b"CT" => syntax.control_prefix = character,
                b"CD" => syntax.delimiter = character,
                _ => unreachable!(),
            }
            let end = self.next_boundary(start + 4, syntax);
            self.syntax = syntax;
            end
        } else {
            self.command_end(code, format)?
        };
        self.position = end;
        let bytes = &self.input[start..end];
        Ok(Some(if format {
            Element::FormatCommand(bytes)
        } else {
            Element::ControlCommand(bytes)
        }))
    }

    pub fn next_element(&mut self) -> Result<Option<Element<'a>>, ParseError> {
        let result = self.scan_element();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}

fn is_control_character(byte: u8) -> bool {
    matches!(byte, 0x02 | 0x03 | 0x0f)
}

fn trim_ascii(mut bytes: &[u8]) -> &[u8] {
    while bytes.first().is_some_and(u8::is_ascii_whitespace) {
        bytes = &bytes[1..];
    }
    while bytes.last().is_some_and(u8::is_ascii_whitespace) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

impl<'a> Iterator for ParseContext<'a> {
    type Item = Result<Element<'a>, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_element().transpose()
    }
}

impl FusedIterator for ParseContext<'_> {}
