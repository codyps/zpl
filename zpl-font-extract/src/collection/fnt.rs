//! Decode observed type-1 FNT files. Raw IDs are never Unicode by assumption.
//! Layout reference: zebra-firmware/docs/fnt-format.md, commit 768c231.
use super::*;
use eyre::ensure;
pub fn decode(name: &str, data: &[u8]) -> Result<Font> {
    ensure!(selector(name) && name.contains(':'), "invalid named font");
    ensure!(
        (116..=32 * 1024 * 1024).contains(&data.len()),
        "invalid FNT size"
    );
    ensure!(
        data[..4] == [0; 4] && data[4] == 1 && [&[0, 3][..], &[3, 0][..]].contains(&&data[6..8]),
        "unsupported bitmap FNT type/variant"
    );
    let u16at = |i| u16::from_be_bytes([data[i], data[i + 1]]);
    let u32at = |i| u32::from_be_bytes(data[i..i + 4].try_into().unwrap()) as usize;
    let count = u16at(100);
    let table = u32at(92);
    let bitmap = u32at(96);
    ensure!(
        (1..=4096).contains(&count)
            && u16at(76) == 116
            && table == 116
            && bitmap == 116 + usize::from(count) * 20
            && bitmap <= data.len(),
        "invalid FNT table bounds"
    );
    let mut f = Font {
        name: name.into(),
        source_sha256: hash(data),
        header_hex: hex(&data[..116]),
        slot_count: count,
        absent_slots: vec![],
        zero_record_slots: vec![],
        records: vec![],
        encodings: vec![],
    };
    let mut total = 0usize;
    for id in 0..count {
        let p = table + usize::from(id) * 20;
        let row = &data[p..p + 20];
        if row.iter().all(|b| *b == 255) {
            f.absent_slots.push(id);
            continue;
        }
        if row.iter().all(|b| *b == 0) {
            f.zero_record_slots.push(id);
            continue;
        }
        let h = u16at(p + 2);
        let w = u16at(p + 4);
        let len = usize::from(u16at(p + 6));
        let offset = u32at(p + 16);
        ensure!(
            u16at(p) == id
                && w <= 4096
                && h <= 4096
                && len == usize::from(w).div_ceil(8) * usize::from(h),
            "invalid FNT record {id}"
        );
        ensure!(
            offset <= data.len() - bitmap && len <= data.len() - bitmap - offset,
            "truncated FNT glyph {id}"
        );
        total += len;
        ensure!(total <= 16 * 1024 * 1024, "decoded bitmap budget exceeded");
        f.records.push(Record {
            id,
            advance: u16at(p + 12),
            left: u16at(p + 8) as i16,
            top: (u16at(p + 10) as i16)
                .checked_neg()
                .ok_or_else(|| eyre::eyre!("unrepresentable FNT y offset"))?,
            width: w,
            height: h,
            flags: u16at(p + 14),
            bitmap_hex: hex(&data[bitmap + offset..bitmap + offset + len]),
        });
    }
    Ok(f)
}
pub fn import_directory(path: &Path) -> Result<Collection> {
    let mut fonts = vec![];
    let mut skipped = vec![];
    for entry in std::fs::read_dir(path)? {
        let p = entry?.path();
        let Some(file) = p.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !file.ends_with(".FNT") {
            continue;
        }
        let name = file.replacen('-', ":", 1);
        let bytes = read(&p)?;
        ensure!(bytes.len() >= 116, "truncated FNT header in {name}");
        if bytes.get(4) != Some(&1) {
            skipped.push(serde_json::json!({"name":name,"sha256":hash(&bytes),"reason":"not a type-1 bitmap font"}));
            continue;
        }
        fonts.push(decode(&name, &bytes)?);
    }
    fonts.sort_by(|a, b| a.name.cmp(&b.name));
    Collection::new(
        fonts,
        serde_json::json!({"source":"stored-font-objects","skipped":skipped,"mapping_completeness":"unprobed"}),
    )
}
