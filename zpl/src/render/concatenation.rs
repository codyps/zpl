//! ^FE field concatenation, Zebra Programming Guide pp. 191–192:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use crate::parse::Syntax;
use std::{collections::HashMap, sync::Arc};

pub(super) fn delimiter(data: &[u8], syntax: Syntax, printer: bool) -> Result<u8, String> {
    let mut end = data.len();
    while end > 0 && matches!(data[end - 1], b'\r' | b'\n') {
        end -= 1;
    }
    let data = &data[..end];
    let marker = match data {
        [] => b'#',
        [c] if printer && (*c == syntax.delimiter || *c == b' ') => b'#',
        [c] if c.is_ascii() => *c,
        _ => return Err("FE requires one ASCII delimiter".into()),
    };
    if marker == syntax.format_prefix || marker == syntax.control_prefix {
        return Err("FE delimiter cannot be a command prefix".into());
    }
    Ok(marker)
}

pub(super) fn expand(
    data: &[u8],
    marker: u8,
    values: &HashMap<u16, Arc<Vec<u8>>>,
    utf8: bool,
    backward_reads_forward: bool,
    printer_syntax: bool,
    field_limit: usize,
) -> Result<Vec<u8>, String> {
    let append = |output: &mut Vec<u8>, value: &[u8]| append(output, value, field_limit);
    let mut output = Vec::new();
    let mut remaining = data;
    while let Some(start) = remaining.iter().position(|&c| c == marker) {
        append(&mut output, &remaining[..start])?;
        remaining = &remaining[start + 1..];
        let Some(end) = remaining.iter().position(|&c| c == marker) else {
            append(&mut output, &[marker])?;
            append(&mut output, remaining)?;
            return Ok(output);
        };
        if end == 0 {
            append(&mut output, &[marker])?;
            if !printer_syntax {
                append(&mut output, &[marker])?;
            }
            remaining = &remaining[1..];
            continue;
        }
        if let Ok(token) = std::str::from_utf8(&remaining[..end]) {
            let parts: Vec<_> = token.split(',').collect();
            if let Some(value) = parts
                .first()
                .and_then(|n| n.parse::<u16>().ok())
                .and_then(|n| values.get(&n))
            {
                if parts.len() == 1 {
                    append(&mut output, value)?;
                } else if parts.len() == 4 {
                    let length = if printer_syntax {
                        parts[3].parse::<i64>().ok().map(|n| n as usize)
                    } else {
                        parts[3].parse::<usize>().ok()
                    };
                    if let (Ok(position), Some(length)) = (parts[2].parse::<usize>(), length) {
                        let mut offsets: Vec<_> = if utf8 {
                            std::str::from_utf8(value)
                                .map_err(|_| "invalid UTF-8 referenced field")?
                                .char_indices()
                                .map(|(i, _)| i)
                                .collect()
                        } else {
                            (0..value.len()).collect()
                        };
                        offsets.push(value.len());
                        let count = offsets.len() - 1;
                        if position > 0 && position <= count {
                            let direction = if printer_syntax {
                                parts[1].to_ascii_lowercase()
                            } else {
                                parts[1].to_string()
                            };
                            let range = match direction.as_str() {
                                "f" => Some(
                                    (position - 1)
                                        ..(position - 1).saturating_add(length).min(count),
                                ),
                                "b" if backward_reads_forward => Some(
                                    (count - position)
                                        ..(count - position).saturating_add(length).min(count),
                                ),
                                "b" => {
                                    let end = count - position + 1;
                                    Some(end.saturating_sub(length)..end)
                                }
                                _ => None,
                            };
                            if let Some(range) = range {
                                append(
                                    &mut output,
                                    &value[offsets[range.start]..offsets[range.end]],
                                )?;
                            }
                        }
                    }
                }
            }
        }
        remaining = &remaining[end + 1..];
    }
    append(&mut output, remaining)?;
    Ok(output)
}
fn append(output: &mut Vec<u8>, value: &[u8], field_limit: usize) -> Result<(), String> {
    if output
        .len()
        .checked_add(value.len())
        .is_none_or(|n| n > field_limit)
    {
        return Err(format!(
            "concatenated field exceeds {field_limit}-byte renderer limit"
        ));
    }
    output.extend_from_slice(value);
    Ok(())
}
