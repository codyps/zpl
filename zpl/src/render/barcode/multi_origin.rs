//! ^FM (Zebra guide pp. 197–199) and Macro PDF417 (USS PDF417 Appendix G).
//! https://www.expresscorp.com/wp-content/uploads/2023/02/USS-PDF-417.pdf
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use super::*;

fn file_id(b: &Barcode, data: &[u8]) -> Result<[usize; 3], String> {
    if let Some(id) = b.compatibility.macro_pdf417_file_id {
        if id.iter().any(|&v| v >= 900) {
            return Err("Macro PDF417 file ID codewords must be below 900".into());
        }
        return Ok(id.map(usize::from));
    }
    // A stable payload-derived identifier; three base-900 codewords, as allowed
    // by Appendix G.4. Identical payloads intentionally have the same file ID.
    let mut hash = 0x811c9dc5u32;
    for byte in data {
        hash = (hash ^ u32::from(*byte)).wrapping_mul(0x01000193);
    }
    let hash = hash as usize % 900usize.pow(3);
    Ok([hash / 810000, hash / 900 % 900, hash % 900])
}

fn trailer(index: usize, count: usize, id: [usize; 3]) -> Vec<usize> {
    // Five-digit numeric-compaction index and count, with the required leading
    // sentinel 1; optional field 1 is the total count. 922 marks only the last.
    let mut words = vec![928, (100000 + index) / 900, (100000 + index) % 900];
    words.extend(id);
    words.extend([923, 1, (100000 + count) / 900, (100000 + count) % 900]);
    if index + 1 == count {
        words.push(922);
    }
    words
}

pub(super) fn render(b: &Barcode, data: &[u8], origins: usize) -> Result<Vec<Path>, String> {
    let payload = pdf417::field_escapes(data);
    let micro = b.name == "BF";
    let encode = |bytes: &[u8]| {
        if micro {
            pdf417::compact_micro(bytes)
        } else {
            pdf417::compact_macro(bytes)
        }
    };
    let (cols, rows, ec, variant) = if micro {
        let mode = b.integer(2, 0, 0, 33)?;
        let variant = match mode {
            0..=22 => mode,
            23..=32 => mode + 1,
            _ => 23,
        };
        (
            micropdf417::columns(variant),
            micropdf417::ROWS[variant],
            micropdf417::EC[variant],
            variant,
        )
    } else {
        let level = b.integer(2, 0, 0, 8)?;
        let ec = 1 << (level + 1);
        let mut cols = b.integer(3, 0, 0, 30)?;
        let mut rows = b.integer(4, 0, 0, 90)?;
        let needed = (encode(&payload).len() + 12 + ec).min(928);
        if cols == 0 {
            cols = if rows != 0 {
                needed.div_ceil(rows)
            } else {
                (((69. * 69. + 408. * needed as f64).sqrt() - 69.) / 34.).round() as usize
            }
            .max(needed.div_ceil(90))
            .max(1);
        }
        if rows == 0 {
            rows = needed.div_ceil(cols).max(3);
        }
        if cols > 30 || !(3..=90).contains(&rows) || rows * cols > 928 {
            return Err("invalid PDF417 structured append dimensions".into());
        }
        (cols, rows, ec, level)
    };
    let capacity = (cols * rows)
        .checked_sub(ec)
        .ok_or("PDF417 error correction exceeds capacity")?;
    // Reserve the last-symbol terminator in every part so the split is stable.
    let budget = capacity.saturating_sub(11 + usize::from(!micro));
    if budget == 0 {
        return Ok(Vec::new());
    }
    let mut parts = Vec::new();
    let mut offset = 0;
    while offset < payload.len() {
        let mut best = None;
        // Compaction can become shorter when numeric/byte runs grow, so do not
        // stop at the first oversized prefix. Bound work by the symbol's maximum
        // numeric capacity (three digits per word), plus compaction overhead.
        for length in (1..=(payload.len() - offset).min(budget * 3 + 6)).rev() {
            let words = encode(&payload[offset..offset + length]);
            if words.len() <= budget {
                best = Some((length, words));
                break;
            }
        }
        let Some((length, words)) = best else {
            return Ok(Vec::new());
        };
        parts.push(words);
        if parts.len() > origins {
            return Ok(Vec::new());
        }
        offset += length;
    }
    if parts.is_empty() {
        return Ok(Vec::new());
    }
    let count = parts.len();
    let id = file_id(b, &payload)?;
    let mut paths = Vec::new();
    for (index, part) in parts.into_iter().enumerate() {
        let tail = trailer(index, count, id);
        let mut words = Vec::new();
        if !micro {
            words.push(capacity);
        }
        words.extend(part);
        if micro {
            const PAD: [usize; 8] = [900, 838, 779, 867, 865, 898, 868, 839];
            let period = if cols == 3 { 24 } else { 25 };
            let pad = capacity - tail.len() - words.len();
            words.extend((0..pad).map(|i| PAD[(i % period) % 8]));
        } else {
            words.resize(capacity - tail.len(), 900);
        }
        words.extend(tail);
        let matrix = if micro {
            micropdf417::encode_words(words, variant)?
        } else {
            words.extend(pdf417::parity(&words, ec));
            pdf417::matrix(&words, cols, rows, variant, b.flag(5, false)?)
        };
        let auto_height = (b.height / rows as f64).floor().max(1.);
        let height = if micro {
            let height = b.num(1, 0., 0., 9999.)?;
            if height == 0. {
                auto_height
            } else {
                height
            }
        } else {
            b.num(1, auto_height, 1., 32000.)?
        };
        paths.push(b.matrix(&matrix, b.module, height)?);
    }
    Ok(paths)
}
