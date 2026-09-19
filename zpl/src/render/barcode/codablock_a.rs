//! Original CODABLOCK A row framing and modulo-43 checks.
//! Zebra ZPL II Programming Guide ^BB, pp. 90–93 (rows, columns, checksum,
//! alphabet and row height). Row controls and dot placement verified against
//! ZD621 V93.21.33Z captures in tests/fixtures/barcode-modes-zd621-v1.
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use super::*;
const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%";

fn rows(b: &Barcode, data: &[u8]) -> Result<Vec<String>, String> {
    if data.iter().any(|c| !ALPHABET.contains(c)) {
        return Err("CODABLOCK A requires the Code 39 alphabet".into());
    }
    let requested = b.integer(4, 0, 0, 22)?;
    let checks = b.flag(2, true)?;
    let columns = b.integer(3, 0, 0, 62)?;
    let columns = if columns == 0 {
        (data.len() + if checks && requested > 1 { 2 } else { 0 })
            .div_ceil(requested.max(1))
            .clamp(2, 62)
    } else {
        columns
    };
    if columns < 2 {
        return Err("CODABLOCK A requires at least two data columns".into());
    }
    let count = if requested > 0 {
        requested
    } else if data.len() <= columns {
        1
    } else {
        (data.len() + if checks { 2 } else { 0 }).div_ceil(columns)
    };
    if count > 22 {
        return Err("CODABLOCK A exceeds 22 rows".into());
    }
    let capacity = count * columns - if checks && count > 1 { 2 } else { 0 };
    if data.len() > capacity {
        return Err("CODABLOCK A data exceeds requested rows and columns".into());
    }
    if count == 1 {
        return Ok(vec![format!("L{}", ascii(data)?)]);
    }
    let mut padded = data.to_vec();
    padded.resize(capacity, b' ');
    if checks {
        // Global checks include data-column padding but exclude row indicators.
        // Unlike Code 39's optional row checksum, these two checks cover the block.
        let mut k1 = 0;
        let mut k2 = 0;
        for (i, c) in padded.iter().enumerate() {
            let v = ALPHABET.iter().position(|x| x == c).unwrap();
            k1 += (i + 1) * v;
            k2 += i * v;
        }
        if b.compatibility.codablock_a_wrapping_checks {
            k1 = k1 as u16 as usize;
            k2 = k2 as u16 as usize;
        }
        padded.extend([ALPHABET[k1 % 43], ALPHABET[k2 % 43]]);
    }
    let mut rows = Vec::new();
    for (i, chunk) in padded.chunks(columns).enumerate() {
        let id = ALPHABET[if i == 0 { 20 + count } else { i - 1 }] as char;
        rows.push(format!("{id}{}{id}", ascii(chunk)?));
    }
    Ok(rows)
}
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    let rows = rows(b, data)?;
    let mut row_height = b.num(1, 8., 2., 32000.)?;
    if !b.compatibility.codablock_a_row_height_in_dots {
        row_height *= b.module;
    }
    let mut path = Path::default();
    let mut width: f64 = 0.;
    for (i, row) in rows.iter().enumerate() {
        let mut p = code39::render(row, b.module, b.ratio, row_height + b.module)?;
        width = width.max(super::super::bounds(&p).2);
        p.transform(|p| Point::new(p.x, p.y + i as f64 * row_height));
        path.segments.extend(p.segments);
    }
    if rows.len() == 1 {
        width += b.module;
    }
    let character_width = (7. + 3. * b.ratio) * b.module;
    for i in 1..rows.len() {
        path.rect(
            character_width,
            i as f64 * row_height,
            width - 2. * character_width,
            b.module,
        );
    }
    path.rect(0., 0., width, b.module);
    path.rect(0., rows.len() as f64 * row_height, width, b.module);
    Ok(super::super::font::union_lines(path))
}
