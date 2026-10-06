//! Inline numbered fields, Zebra Programming Guide ^FN, p. 200:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
//! Plan data substitutions before painting, retaining the original command
//! stream, syntax, field styles and error offsets. DF/XF templates have already
//! been expanded by the request-local stored-format planner.
use super::RenderError;
use crate::parse::{Element, ParseContext};
use std::{collections::HashMap, sync::Arc};

pub(super) enum Action {
    Data(Arc<Vec<u8>>),
    Value(Arc<Vec<u8>>),
    Skip,
}
pub(super) type Plan = HashMap<usize, Action>;
struct Field {
    number: u16,
    end: usize,
    data: Option<(usize, Arc<Vec<u8>>)>,
}
fn error(offset: usize, message: impl Into<String>) -> RenderError {
    RenderError {
        offset,
        message: message.into(),
    }
}
fn number(data: &[u8], offset: usize) -> Result<u16, RenderError> {
    let text = std::str::from_utf8(data)
        .map_err(|_| error(offset, "invalid FN number"))?
        .trim_end_matches(['\r', '\n']);
    let (digits, prompt) = text
        .split_once('"')
        .map_or((text, None), |(n, p)| (n, Some(p)));
    if let Some(prompt) = prompt {
        let value = prompt
            .strip_suffix('"')
            .ok_or_else(|| error(offset, "unterminated FN prompt"))?;
        if value.len() > 255
            || !value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b' ')
        {
            return Err(error(
                offset,
                "FN prompt requires at most 255 alphanumeric characters or spaces",
            ));
        }
    }
    if digits.is_empty() {
        return Ok(0);
    }
    if !digits.bytes().all(|c| c.is_ascii_digit()) {
        return Err(error(offset, "invalid FN number"));
    }
    let number = digits
        .parse::<u16>()
        .map_err(|_| error(offset, "FN number exceeds 9999"))?;
    if number > 9999 {
        return Err(error(offset, "FN number exceeds 9999"));
    }
    Ok(number)
}
fn decoded(
    data: &[u8],
    hex: Option<u8>,
    offset: usize,
    field_limit: usize,
) -> Result<Arc<Vec<u8>>, RenderError> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if Some(data[i]) == hex {
            let pair = data
                .get(i + 1..i + 3)
                .and_then(|v| std::str::from_utf8(v).ok())
                .and_then(|v| u8::from_str_radix(v, 16).ok())
                .ok_or_else(|| error(offset, "invalid FN field hex escape"))?;
            result.push(pair);
            i += 3;
        } else {
            result.push(data[i]);
            i += 1;
        }
    }
    if result.len() > field_limit {
        return Err(error(
            offset,
            format!("field data exceeds {field_limit}-byte renderer limit"),
        ));
    }
    Ok(Arc::new(result))
}
fn finish(fields: &mut Vec<Field>, printer: bool, plan: &mut Plan) {
    if printer {
        let mut waiting: HashMap<u16, Vec<usize>> = HashMap::new();
        for field in fields.drain(..) {
            if let Some((offset, value)) = field.data {
                if let Some(references) = waiting.remove(&field.number) {
                    for end in references {
                        plan.insert(end, Action::Data(value.clone()));
                    }
                    plan.insert(offset, Action::Skip);
                }
            } else {
                waiting.entry(field.number).or_default().push(field.end);
            }
        }
    } else {
        let mut values = HashMap::new();
        for field in fields.iter() {
            if let Some((_, value)) = &field.data {
                values.insert(field.number, value.clone());
            }
        }
        for field in fields.drain(..) {
            if field.data.is_none() {
                if let Some(value) = values.get(&field.number) {
                    plan.insert(field.end, Action::Data(value.clone()));
                }
            }
        }
    }
}
pub(super) fn plan(
    input: &[u8],
    compatibility: super::compatibility::Compatibility,
    field_limit: usize,
) -> Result<Plan, RenderError> {
    let mut plan = Plan::new();
    if !input.windows(2).any(|b| b == b"FN" || b == b"FE") {
        return Ok(plan);
    }
    let mut parser = ParseContext::from_bytes(input);
    let mut fields = Vec::new();
    let (mut field_number, mut data, mut hex) = (None, None, None);
    let mut label = false;
    let mut concat = None;
    let mut encoding = 0;
    let mut values = HashMap::new();
    loop {
        let syntax = parser.syntax();
        let offset = parser.position();
        let Some(item) = parser.next() else { break };
        let item = item.map_err(|e| error(offset, e.to_string()))?;
        let raw = item.as_bytes();
        let (name, operands) = match item {
            Element::FormatCommand(_) | Element::ControlCommand(_) => (&raw[1..3], &raw[3..]),
            Element::ControlCharacter(_) => (
                match raw[0] {
                    2 => &b"XA"[..],
                    3 => b"XZ",
                    15 => b"FS",
                    _ => b"",
                },
                &b""[..],
            ),
            _ => continue,
        };
        let marker = concat.take();
        if compatibility.concatenation_retains_delimiter && !matches!(name, b"FD" | b"FV") {
            concat = marker;
        }
        match name {
            b"XA" => {
                label = true;
            }
            b"XZ" => {
                finish(
                    &mut fields,
                    compatibility.numbered_fields_forward_only,
                    &mut plan,
                );
                values.clear();
                concat = None;
                label = false;
                field_number = None;
                data = None;
                hex = None;
            }
            b"CI" => {
                encoding = std::str::from_utf8(operands)
                    .ok()
                    .and_then(|s| s.trim_end().parse::<u8>().ok())
                    .unwrap_or(0);
            }
            b"FE" => {
                concat = Some(
                    super::concatenation::delimiter(
                        operands,
                        syntax,
                        compatibility.concatenation_printer_syntax,
                    )
                    .map_err(|e| error(offset, e))?,
                );
            }
            b"FN" => {
                if !label {
                    return Err(error(offset, "FN outside label"));
                }
                if field_number.is_some() {
                    return Err(error(offset, "multiple FN commands in one field"));
                }
                field_number = Some(number(operands, offset)?);
            }
            b"FH" => {
                hex = Some(
                    operands
                        .iter()
                        .copied()
                        .take_while(|b| !matches!(b, b'\r' | b'\n'))
                        .next()
                        .unwrap_or(b'_'),
                );
            }
            b"FD" | b"FV" if field_number.is_some() || marker.is_some() => {
                if data.is_some() {
                    return Err(error(offset, "multiple data commands in numbered field"));
                }
                let mut value = decoded(operands, hex, offset, field_limit)?;
                if let Some(marker) = marker {
                    value = Arc::new(
                        super::concatenation::expand(
                            &value,
                            marker,
                            &values,
                            encoding == 28,
                            compatibility.concatenation_backward_reads_forward,
                            compatibility.concatenation_printer_syntax,
                            field_limit,
                        )
                        .map_err(|e| error(offset, e))?,
                    );
                    plan.insert(offset, Action::Value(value.clone()));
                }
                if let Some(number) = field_number {
                    // FE reads the first supplied field with this number. Later
                    // explicit bindings remain independent drawings on hardware.
                    values.entry(number).or_insert_with(|| value.clone());
                    data = Some((offset, value));
                }
            }
            b"SN" if field_number.is_some() => {
                return Err(error(offset, "serialization with FN is unsupported"))
            }
            b"FS" => {
                if let Some(number) = field_number.take() {
                    fields.push(Field {
                        number,
                        end: offset,
                        data: data.take(),
                    });
                }
                hex = None;
                concat = None;
            }
            _ => {}
        }
    }
    Ok(plan)
}
