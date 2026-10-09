//! Lossless framing adapter; see docs/parser-coverage.md for the Rust contract.
//! Byte slices cross Wasm without UTF-8 decoding:
//! https://wasm-bindgen.github.io/wasm-bindgen/reference/types/number-slices.html
use wasm_bindgen::prelude::*;
use zpl::parse::{Element, ParseContext, Syntax};

#[wasm_bindgen]
pub struct Parsed {
    // Triples of kind, byte offset, and byte length; no payload copies in Wasm.
    spans: Vec<u32>,
    error_kind: String,
    pub error_offset: Option<u32>,
    pub format_prefix: u8,
    pub control_prefix: u8,
    pub delimiter: u8,
}

#[wasm_bindgen]
impl Parsed {
    pub fn take_spans(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.spans)
    }

    pub fn error_kind(&self) -> String {
        self.error_kind.clone()
    }
}

/// Parse a complete buffer. Framing errors are returned as structured data so
/// the Node wrapper can throw its public ParseError without parsing error text.
#[wasm_bindgen]
pub fn parse_bytes(input: &[u8], format_prefix: u8, control_prefix: u8, delimiter: u8) -> Parsed {
    let mut parser = ParseContext::with_syntax(
        input,
        Syntax {
            format_prefix,
            control_prefix,
            delimiter,
        },
    );
    let mut spans = Vec::new();
    let mut error_kind = String::new();
    let mut error_offset = None;
    loop {
        let offset = parser.position();
        let Some(element) = parser.next() else { break };
        match element {
            Ok(element) => {
                let kind = match element {
                    Element::BeforeFirstCommand(_) => 0,
                    Element::FormatCommand(_) => 1,
                    Element::ControlCommand(_) => 2,
                    Element::ControlCharacter(_) => 3,
                };
                spans.extend([kind, offset as u32, element.as_bytes().len() as u32]);
            }
            Err(error) => {
                error_kind = format!("{:?}", error.kind);
                error_offset = Some(error.offset as u32);
                spans.clear();
                break;
            }
        }
    }
    let syntax = parser.syntax();
    Parsed {
        spans,
        error_kind,
        error_offset,
        format_prefix: syntax.format_prefix,
        control_prefix: syntax.control_prefix,
        delimiter: syntax.delimiter,
    }
}
