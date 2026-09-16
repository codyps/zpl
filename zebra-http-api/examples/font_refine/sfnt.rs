//! Bounded, deterministic calibration font writer, not a general font editor.
//! Table layouts: https://learn.microsoft.com/en-us/typography/opentype/spec/otff
use std::collections::BTreeMap;
pub type Point = (i16, i16, bool);
pub type Contour = Vec<Point>;
pub fn contours(code: u8) -> Vec<Contour> {
    let rect = |x, y, w, h| {
        vec![
            (x, y, true),
            (x, y + h, true),
            (x + w, y + h, true),
            (x + w, y, true),
        ]
    };
    match code {
        b'A' => vec![rect(128, 0, 128, 768)],
        b'B' => vec![rect(136, 0, 128, 768)], // quarter pixel phase at 32 ppem
        b'C' => vec![rect(128, 0, 384, 512), rect(320, 256, 384, 512)],
        b'D' => vec![vec![
            (128, 0, true),
            (128, 768, true),
            (896, 768, false),
            (640, 0, true),
        ]],
        b'E' => vec![vec![
            (128, 0, true),
            (640, 768, true),
            (656, 768, true),
            (144, 0, true),
        ]],
        b'F' => {
            let mut hole = rect(256, 128, 256, 512);
            hole.reverse();
            vec![rect(128, 0, 512, 768), hole]
        }
        b'G' => vec![rect(-8, -128, 128, 896)],
        b'H' => vec![rect(128, 0, 16, 768)],
        _ => vec![],
    }
}
fn u16at(v: &mut [u8], p: usize, n: u16) {
    v[p..p + 2].copy_from_slice(&n.to_be_bytes());
}
fn u32at(v: &mut [u8], p: usize, n: u32) {
    v[p..p + 4].copy_from_slice(&n.to_be_bytes());
}
fn push16(v: &mut Vec<u8>, n: i16) {
    v.extend(n.to_be_bytes());
}
fn checksum(b: &[u8]) -> u32 {
    b.chunks(4).fold(0u32, |s, c| {
        let mut a = [0; 4];
        a[..c.len()].copy_from_slice(c);
        s.wrapping_add(u32::from_be_bytes(a))
    })
}
pub fn font(shift: bool) -> Vec<u8> {
    font_with_scan(shift, None)
}
pub fn font_with_scan(shift: bool, scan: Option<u8>) -> Vec<u8> {
    build(shift, scan, false)
}
pub fn font_with_glyph_scan(mode: u8) -> Vec<u8> {
    build(false, Some(mode), true)
}
fn build(shift: bool, scan: Option<u8>, glyph_scan: bool) -> Vec<u8> {
    let mut tables = BTreeMap::new();
    if let Some(mode) = scan {
        assert!([0, 1, 2, 4, 5].contains(&mode));
        tables.insert(*b"prep", vec![0xb8, 0x01, 0xff, 0x85, 0xb0, mode, 0x8d]);
    }
    let mut glyf = Vec::new();
    let mut loca = Vec::new();
    let mut hmtx = Vec::new();
    for code in std::iter::once(0).chain(b'A'..=b'H') {
        loca.extend((glyf.len() as u32).to_be_bytes());
        let cs = contours(code);
        let points: Vec<_> = cs.iter().flatten().copied().collect();
        let minx = points.iter().map(|p| p.0).min().unwrap_or(0);
        hmtx.extend(1024u16.to_be_bytes());
        push16(&mut hmtx, minx);
        if points.is_empty() {
            continue;
        }
        push16(&mut glyf, cs.len() as i16);
        for n in [
            minx,
            points.iter().map(|p| p.1).min().unwrap(),
            points.iter().map(|p| p.0).max().unwrap(),
            points.iter().map(|p| p.1).max().unwrap(),
        ] {
            push16(&mut glyf, n);
        }
        let mut end = 0;
        for c in &cs {
            end += c.len();
            push16(&mut glyf, (end - 1) as i16);
        }
        // SVTCA[x], SLOOP, SHPIX: move every outline point exactly one device pixel.
        let instructions = if glyph_scan {
            vec![0xb8, 0x01, 0xff, 0x85, 0xb0, scan.unwrap(), 0x8d]
        } else if shift {
            let mut p = vec![
                0x01,
                0xb0,
                points.len() as u8,
                0x17,
                0x40,
                (points.len() + 1) as u8,
            ];
            p.extend(0..points.len() as u8);
            p.extend([64, 0x38]);
            p
        } else {
            vec![]
        };
        push16(&mut glyf, instructions.len() as i16);
        glyf.extend(instructions);
        glyf.extend(points.iter().map(|p| u8::from(p.2)));
        for axis in 0..2 {
            let mut last = 0;
            for p in &points {
                let value = if axis == 0 { p.0 } else { p.1 };
                push16(&mut glyf, value - last);
                last = value;
            }
        }
        while glyf.len() % 4 != 0 {
            glyf.push(0);
        }
    }
    loca.extend((glyf.len() as u32).to_be_bytes());
    tables.insert(*b"glyf", glyf);
    tables.insert(*b"loca", loca);
    tables.insert(*b"hmtx", hmtx);
    let mut head = vec![0; 54];
    u32at(&mut head, 0, 0x10000);
    u32at(&mut head, 4, 0x10000);
    u32at(&mut head, 12, 0x5f0f3cf5);
    u16at(&mut head, 16, 3);
    u16at(&mut head, 18, 1024);
    u16at(&mut head, 36, (-8i16) as u16);
    u16at(&mut head, 38, (-128i16) as u16);
    u16at(&mut head, 40, 896);
    u16at(&mut head, 42, 768);
    u16at(&mut head, 46, 8);
    u16at(&mut head, 48, 2);
    u16at(&mut head, 50, 1);
    tables.insert(*b"head", head);
    let mut hhea = vec![0; 36];
    u32at(&mut hhea, 0, 0x10000);
    u16at(&mut hhea, 4, 768);
    u16at(&mut hhea, 6, (-256i16) as u16);
    u16at(&mut hhea, 10, 1024);
    u16at(&mut hhea, 12, (-8i16) as u16);
    u16at(&mut hhea, 14, 128);
    u16at(&mut hhea, 16, 896);
    u16at(&mut hhea, 18, 1);
    u16at(&mut hhea, 34, 9);
    tables.insert(*b"hhea", hhea);
    let mut maxp = vec![0; 32];
    u32at(&mut maxp, 0, 0x10000);
    u16at(&mut maxp, 4, 9);
    u16at(&mut maxp, 6, 8);
    u16at(&mut maxp, 8, 2);
    u16at(&mut maxp, 14, 1);
    u16at(&mut maxp, 24, 16);
    u16at(&mut maxp, 26, 16);
    tables.insert(*b"maxp", maxp);
    // Format 4: contiguous A..H plus sentinel; code + delta yields glyph ID.
    let mut cmap = vec![0; 44];
    u16at(&mut cmap, 2, 1);
    u16at(&mut cmap, 4, 3);
    u16at(&mut cmap, 6, 1);
    u32at(&mut cmap, 8, 12);
    for (p, n) in [
        (12, 4),
        (14, 32),
        (18, 4),
        (20, 4),
        (22, 1),
        (26, 72),
        (28, 65535),
        (32, 65),
        (34, 65535),
        (36, 65472),
        (38, 1),
    ] {
        u16at(&mut cmap, p, n);
    }
    tables.insert(*b"cmap", cmap);
    let mut os2 = vec![0; 96];
    for (p, n) in [
        (0, 4),
        (2, 1024),
        (4, 400),
        (6, 5),
        (62, 64),
        (64, 65),
        (66, 72),
        (68, 768),
        (70, (-256i16) as u16),
        (74, 768),
        (76, 256),
        (88, 768),
        (94, 1),
    ] {
        u16at(&mut os2, p, n);
    }
    u32at(&mut os2, 42, 1);
    os2[58..62].copy_from_slice(b"ZPL ");
    tables.insert(*b"OS/2", os2);
    let mut name = vec![0; 6 + 5 * 12];
    u16at(&mut name, 2, 5);
    u16at(&mut name, 4, 66);
    for (i, (id, s)) in [
        (1, "ZplCalibration"),
        (2, "Regular"),
        (4, "ZplCalibration Regular"),
        (5, "Version 1.0"),
        (6, "ZplCalibration-Regular"),
    ]
    .iter()
    .enumerate()
    {
        let bytes: Vec<_> = s.encode_utf16().flat_map(u16::to_be_bytes).collect();
        let off = name.len() - 66;
        for (p, n) in [
            (0, 3),
            (2, 1),
            (4, 0x409),
            (6, *id),
            (8, bytes.len() as u16),
            (10, off as u16),
        ] {
            u16at(&mut name, 6 + i * 12 + p, n);
        }
        name.extend(bytes);
    }
    tables.insert(*b"name", name);
    let mut post = vec![0; 32];
    u32at(&mut post, 0, 0x30000);
    u16at(&mut post, 8, (-100i16) as u16);
    u16at(&mut post, 10, 50);
    tables.insert(*b"post", post);
    let count = tables.len();
    let power = 1usize << count.ilog2();
    let mut out = vec![0; 12 + 16 * count];
    u32at(&mut out, 0, 0x10000);
    u16at(&mut out, 4, count as u16);
    u16at(&mut out, 6, (power * 16) as u16);
    u16at(&mut out, 8, count.ilog2() as u16);
    u16at(&mut out, 10, ((count - power) * 16) as u16);
    let mut head_offset = 0;
    for (i, (tag, bytes)) in tables.into_iter().enumerate() {
        let offset = out.len();
        let p = 12 + i * 16;
        out[p..p + 4].copy_from_slice(&tag);
        u32at(&mut out, p + 4, checksum(&bytes));
        u32at(&mut out, p + 8, offset as u32);
        u32at(&mut out, p + 12, bytes.len() as u32);
        if &tag == b"head" {
            head_offset = offset;
        }
        out.extend(bytes);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }
    let adjustment = 0xb1b0afbau32.wrapping_sub(checksum(&out));
    u32at(&mut out, head_offset + 8, adjustment);
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn valid_directory_checksums_and_glyph_offsets() {
        for shift in [false, true] {
            let data = font(shift);
            assert_eq!(checksum(&data), 0xb1b0afba);
            let n = u16::from_be_bytes(data[4..6].try_into().unwrap()) as usize;
            let mut last = &b"    "[..];
            for entry in data[12..12 + n * 16].chunks_exact(16) {
                assert!(last < &entry[..4]);
                last = &entry[..4];
                let off = u32::from_be_bytes(entry[8..12].try_into().unwrap()) as usize;
                let len = u32::from_be_bytes(entry[12..16].try_into().unwrap()) as usize;
                assert_eq!(off % 4, 0);
                let mut table = data[off..off + len].to_vec();
                if last == b"head" {
                    table[8..12].fill(0);
                }
                assert_eq!(
                    checksum(&table),
                    u32::from_be_bytes(entry[4..8].try_into().unwrap())
                );
                if last == b"loca" {
                    assert_eq!(len, 40);
                    let offsets: Vec<_> = table
                        .chunks_exact(4)
                        .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
                        .collect();
                    assert!(offsets.windows(2).all(|p| p[0] <= p[1]));
                }
            }
        }
    }
}
