//! Embedded, captured resident fonts. Bitmap pixels become output-neutral paths.
//! Metrics: ZPL Programming Guide Tables 29/31, pp. 1582–1583:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use crate::{
    bitmap_font::{self, Glyph, Settings},
    output::Path,
};
use std::{collections::BTreeMap, sync::OnceLock};
const DATA: &[u8] = include_bytes!("../../assets/font0-32.zbf");
fn strike() -> &'static (Settings, Vec<Glyph>) {
    static FONT: OnceLock<(Settings, Vec<Glyph>)> = OnceLock::new();
    FONT.get_or_init(|| {
        let (settings, mut glyphs) = bitmap_font::unpack(DATA).expect("validated ASCII strike");
        glyphs.extend(
            bitmap_font::unpack(include_bytes!("../../assets/font0-32-latin1.zbf"))
                .expect("validated Latin-1 supplement")
                .1,
        );
        extend_hyphens(settings, &mut glyphs);
        (settings, glyphs)
    })
}
// Independently captured soft-hyphen and eth supplements. ^FB p. 187;
// see field-block-hyphenation-zd621-v1 for the CI27 automatic-break departure.
fn extend_hyphens(settings: Settings, glyphs: &mut Vec<Glyph>) {
    let data: &[u8] = match (settings.font, settings.height, settings.width) {
        ('0', 24, 24) => include_bytes!("../../assets/font0-24-24-hyphen.zbf"),
        ('A', 9, 5) => include_bytes!("../../assets/fontA-9-5-hyphen.zbf"),
        ('0', 32, 0) => include_bytes!("../../assets/font0-32-0-hyphen.zbf"),
        ('0', 16, 0) => include_bytes!("../../assets/font0-16-0-hyphen.zbf"),
        ('0', 20, 0) => include_bytes!("../../assets/font0-20-0-hyphen.zbf"),
        ('0', 64, 0) => include_bytes!("../../assets/font0-64-0-hyphen.zbf"),
        ('0', 32, 16) => include_bytes!("../../assets/font0-32-16-hyphen.zbf"),
        ('0', 32, 24) => include_bytes!("../../assets/font0-32-24-hyphen.zbf"),
        ('0', 32, 64) => include_bytes!("../../assets/font0-32-64-hyphen.zbf"),
        ('B', 11, 7) => include_bytes!("../../assets/fontB-11-7-hyphen.zbf"),
        ('D', 18, 10) => include_bytes!("../../assets/fontD-18-10-hyphen.zbf"),
        ('E', 28, 15) => include_bytes!("../../assets/fontE-28-15-hyphen.zbf"),
        ('F', 26, 13) => include_bytes!("../../assets/fontF-26-13-hyphen.zbf"),
        ('G', 60, 40) => include_bytes!("../../assets/fontG-60-40-hyphen.zbf"),
        ('H', 21, 13) => include_bytes!("../../assets/fontH-21-13-hyphen.zbf"),
        _ => return,
    };
    glyphs.extend(
        bitmap_font::unpack(data)
            .expect("validated hyphen supplement")
            .1,
    );
    glyphs.sort_by_key(|g| g.codepoint);
}
fn strikes() -> &'static Vec<(Settings, Vec<Glyph>)> {
    static STRIKES: OnceLock<Vec<(Settings, Vec<Glyph>)>> = OnceLock::new();
    STRIKES.get_or_init(|| {
        [
            include_bytes!("../../assets/font0-16-0.zbf").as_slice(),
            include_bytes!("../../assets/font0-20-0.zbf").as_slice(),
            include_bytes!("../../assets/font0-24-24.zbf").as_slice(),
            include_bytes!("../../assets/font0-64-0.zbf").as_slice(),
            include_bytes!("../../assets/font0-32-16.zbf").as_slice(),
            include_bytes!("../../assets/font0-32-24.zbf").as_slice(),
            include_bytes!("../../assets/font0-32-64.zbf").as_slice(),
            include_bytes!("../../assets/fontA-9-5.zbf").as_slice(),
            include_bytes!("../../assets/fontB-11-7.zbf").as_slice(),
            include_bytes!("../../assets/fontD-18-10.zbf").as_slice(),
            include_bytes!("../../assets/fontF-26-13.zbf").as_slice(),
            include_bytes!("../../assets/fontG-60-40.zbf").as_slice(),
            include_bytes!("../../assets/fontH-21-13.zbf").as_slice(),
            include_bytes!("../../assets/fontGS-24-24.zbf").as_slice(),
            include_bytes!("../../assets/fontE-28-15.zbf").as_slice(),
        ]
        .into_iter()
        .map(|data| {
            let (settings, mut glyphs) =
                bitmap_font::unpack(data).expect("validated resident strike");
            extend_hyphens(settings, &mut glyphs);
            (settings, glyphs)
        })
        .collect()
    })
}
fn selected(id: char, w: f64, h: f64) -> (&'static [Glyph], f64, f64) {
    // C and D share the 18x10 matrix (ZPL Programming Guide Table 31,
    // p. 1583); resident-bc-zd621-v1 verifies the alias across all ASCII.
    let id = if id == 'C' { 'D' } else { id };
    for (s, glyphs) in strikes() {
        let sw = if s.width == 0 { s.height } else { s.width } as f64;
        if s.font == id && (id != '0' || (s.height as f64 == h && sw == w)) {
            return (glyphs, w / sw, h / s.height as f64);
        }
    }
    (&strike().1, w / 32., h / 32.)
}
fn glyph_from(glyphs: &'static [Glyph], c: char) -> Result<&'static Glyph, String> {
    let index = glyphs
        .binary_search_by_key(&(c as u32), |g| g.codepoint as u32)
        .map_err(|_| format!("unsupported embedded font glyph {c:?}"))?;
    Ok(&glyphs[index])
}
#[cfg(test)]
fn glyph(c: char) -> Result<&'static Glyph, String> {
    glyph_from(&strike().1, c)
}
#[cfg(test)]
fn baseline(h: f64) -> f64 {
    baseline_for('0', h)
}
pub(super) fn baseline_for(id: char, h: f64) -> f64 {
    // Zebra guide p. 1582: one-based baselines 7 (A), 11 (B), 14 (C/D),
    // 23 (E), 21 (F), 48 (G), 21 (H). These zero-based offsets locate native glyph ink in its cell.
    h * match id {
        'A' => 6. / 9.,
        'B' => 10. / 11.,
        'C' | 'D' => 13. / 18.,
        'E' => 22. / 28.,
        'F' => 20. / 26.,
        'G' => 47. / 60.,
        'H' => 20. / 21.,
        // Captured GS metrics use baseline 23; the specification FT anchor
        // is selected separately by graphic_symbol_last_row_baseline.
        'S' => 23. / 24.,
        _ => 0.75,
    }
}
#[cfg(test)]
fn width(s: &str, w: f64) -> Result<f64, String> {
    width_for('0', s, w, 32.)
}
pub(super) fn width_for(id: char, s: &str, w: f64, h: f64) -> Result<f64, String> {
    let (glyphs, sx, _) = selected(id, w, h);
    s.chars().try_fold(0., |sum, c| {
        Ok(sum + glyph_from(glyphs, c)?.advance as f64 * sx)
    })
}
pub(super) fn inverted_margin(id: char, value: &str, w: f64, h: f64) -> Result<f64, String> {
    if id == 'A' {
        return Ok(w / 5. + 2.);
    }
    if id == 'B' {
        return Ok(2. * w / 7. + 2.);
    }
    if id == 'E' {
        // resident-e-zd621-v1: OCR-B's inverted margin is six native dots,
        // although its ordinary advance includes only five gap dots.
        return Ok(6. * w / 15. + 2.);
    }
    if id == 'H' {
        return Ok(6. * w / 13. + 2.);
    }
    if id == 'G' {
        return Ok(8. * w / 40. + 2.);
    }
    if id == 'F' {
        return Ok(3. * w / 13. + 2.);
    }
    if matches!(id, 'C' | 'D') {
        return Ok(w / 5. + 2.);
    }
    let Some(c) = value.chars().last() else {
        return Ok(0.);
    };
    let (glyphs, sx, _) = selected(id, w, h);
    let g = glyph_from(glyphs, c)?;
    Ok(((g.advance as f64 - g.left as f64 - g.width as f64) * sx - 1.).max(0.))
}
#[cfg(test)]
fn text(s: &str, w: f64, h: f64) -> Result<Path, String> {
    text_for('0', s, w, h)
}
// Preserve individual glyph ink for printer edge placement. The regular
// text path remains merged, so unclamped output does not change.
pub(super) fn text_parts_for(id: char, s: &str, w: f64, h: f64) -> Result<Vec<Path>, String> {
    let mut parts = Vec::new();
    let mut pen = 0.;
    for c in s.chars() {
        let value = c.to_string();
        let mut path = text_for(id, &value, w, h)?;
        path.transform(|p| crate::output::Point::new(p.x + pen, p.y));
        pen += width_for(id, &value, w, h)?;
        if !path.segments.is_empty() {
            parts.push(path);
        }
    }
    Ok(parts)
}
pub(super) fn text_for(id: char, s: &str, w: f64, h: f64) -> Result<Path, String> {
    let (glyphs, sx, sy) = selected(id, w, h);
    // Merge ink spans before emitting even-odd subpaths. Proportional glyphs can
    // overhang their advance; overlapping strokes must remain black, not XOR.
    let mut rows: BTreeMap<i32, Vec<(f64, f64)>> = BTreeMap::new();
    let mut pen = 0.;
    for c in s.chars() {
        let g = glyph_from(glyphs, c)?;
        for (y, row) in g.bitmap.iter().enumerate() {
            let mut start = None;
            for x in 0..=g.width as usize {
                let black = x < g.width as usize && row[x / 8] & (128 >> (x % 8)) != 0;
                match (start, black) {
                    (None, true) => start = Some(x),
                    (Some(a), false) => {
                        rows.entry(g.top + y as i32).or_default().push((
                            pen + g.left as f64 + a as f64,
                            pen + g.left as f64 + x as f64,
                        ));
                        start = None
                    }
                    _ => {}
                }
            }
        }
        pen += g.advance as f64;
    }
    let mut path = Path::default();
    for (y, mut spans) in rows {
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Option<(f64, f64)> = None;
        for (a, b) in spans {
            if let Some((left, right)) = merged {
                if a <= right {
                    merged = Some((left, right.max(b)));
                } else {
                    path.rect(
                        left * sx,
                        baseline_for(id, h) + y as f64 * sy,
                        (right - left) * sx,
                        sy,
                    );
                    merged = Some((a, b));
                }
            } else {
                merged = Some((a, b));
            }
        }
        if let Some((left, right)) = merged {
            path.rect(
                left * sx,
                baseline_for(id, h) + y as f64 * sy,
                (right - left) * sx,
                sy,
            );
        }
    }
    Ok(path)
}
/// Union rectangular ink from multiple lines. Descenders and rounded glyph
/// overshoots can touch the next line even with zero line spacing.
pub(super) fn union_lines(path: Path) -> Path {
    use crate::output::Segment;
    let mut rectangles = Vec::new();
    let mut events = Vec::new();
    for segments in path.segments.as_chunks::<5>().0 {
        let [Segment::Move(a), Segment::Line(b), Segment::Line(c), Segment::Line(_), Segment::Close] =
            segments
        else {
            unreachable!("font ink consists of rectangles")
        };
        let index = rectangles.len();
        rectangles.push((a.x, b.x));
        events.push((a.y, index, true));
        events.push((c.y, index, false));
    }
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut active = BTreeMap::new();
    let mut output = Path::default();
    let mut i = 0;
    while i < events.len() {
        let y = events[i].0;
        while i < events.len() && events[i].0 == y {
            let (_, id, add) = events[i];
            if add {
                active.insert(id, rectangles[id]);
            } else {
                active.remove(&id);
            }
            i += 1;
        }
        let Some(&(next_y, _, _)) = events.get(i) else {
            break;
        };
        let mut spans: Vec<_> = active.values().copied().collect();
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Option<(f64, f64)> = None;
        for (left, right) in spans {
            if let Some((a, b)) = merged {
                if left <= b {
                    merged = Some((a, b.max(right)))
                } else {
                    output.rect(a, y, b - a, next_y - y);
                    merged = Some((left, right));
                }
            } else {
                merged = Some((left, right));
            }
        }
        if let Some((a, b)) = merged {
            output.rect(a, y, b - a, next_y - y);
        }
    }
    output
}

/// Captured bitmap-font FT dot placement. Native glyph metrics describe the
/// cell; rotated FT anchors include a final dot boundary. See resident-bc-zd621-v1
/// and the subsequent resident-font suites (through resident-h-zd621-v1),
/// with ^FT p. 205
/// Table 7 in the Programming Guide.
pub(super) fn printer_ft_offset(id: char, height: f64, rotation: u8) -> (f64, f64) {
    let native = match id {
        'A' => 9.,
        'B' => 11.,
        'C' | 'D' => 18.,
        'E' => 28.,
        'F' => 26.,
        'G' => 60.,
        'H' => 21.,
        'S' => 24.,
        _ => return (0., 0.),
    };
    let scale = height / native;
    match rotation {
        b'R' => (scale, 0.),
        b'I' => (1., scale),
        b'B' => (1. - scale, 1.),
        _ => (0., 1. - scale),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_strike_is_complete_and_compact() {
        let (s, g) = strike();
        assert_eq!((s.font, s.height, s.width, s.dpi), ('0', 32, 0, 203));
        assert_eq!(
            g.iter().map(|g| g.codepoint).collect::<Vec<_>>(),
            (32..=126).chain([173, 233, 240]).collect::<Vec<_>>()
        );
        assert!(DATA.len() < 4500);
        assert_eq!(width("Wi i", 32.).unwrap(), 51.);
        assert_eq!(glyph('é').unwrap().codepoint, 233);
    }
    #[test]
    fn lowercase_and_baselines() {
        assert_ne!(text("a", 32., 32.).unwrap(), text("A", 32., 32.).unwrap());
        assert_eq!(
            glyph('j').unwrap().top + glyph('j').unwrap().height as i32,
            6
        );
        assert_eq!(baseline(32.), 24.);
    }
}
