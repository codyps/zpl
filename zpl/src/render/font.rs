//! Embedded, captured resident font 0. Bitmap pixels become output-neutral paths.
use crate::{
    bitmap_font::{self, Glyph, Settings},
    output::Path,
};
use std::{collections::BTreeMap, sync::OnceLock};
const DATA: &[u8] = include_bytes!("../../assets/font0-32.zbf");
fn strike() -> &'static (Settings, Vec<Glyph>) {
    static FONT: OnceLock<(Settings, Vec<Glyph>)> = OnceLock::new();
    FONT.get_or_init(|| {
        bitmap_font::unpack(DATA).expect("embedded font strike is validated by tests")
    })
}
fn glyph(c: char) -> Result<&'static Glyph, String> {
    if !(' '..='~').contains(&c) {
        return Err(format!("unsupported embedded font glyph {c:?}"));
    }
    let g = &strike().1[(c as u32 - 32) as usize];
    Ok(g)
}
pub(super) fn baseline(h: f64) -> f64 {
    h * 0.75
}
pub(super) fn width(s: &str, w: f64) -> Result<f64, String> {
    s.chars()
        .try_fold(0., |sum, c| Ok(sum + glyph(c)?.advance as f64 * w / 32.))
}
pub(super) fn text(s: &str, w: f64, h: f64) -> Result<Path, String> {
    // Merge ink spans before emitting even-odd subpaths. Proportional glyphs can
    // overhang their advance; overlapping strokes must remain black, not XOR.
    let mut rows: BTreeMap<i32, Vec<(f64, f64)>> = BTreeMap::new();
    let mut pen = 0.;
    for c in s.chars() {
        let g = glyph(c)?;
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
                        left * w / 32.,
                        baseline(h) + y as f64 * h / 32.,
                        (right - left) * w / 32.,
                        h / 32.,
                    );
                    merged = Some((a, b));
                }
            } else {
                merged = Some((a, b));
            }
        }
        if let Some((left, right)) = merged {
            path.rect(
                left * w / 32.,
                baseline(h) + y as f64 * h / 32.,
                (right - left) * w / 32.,
                h / 32.,
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
    for segments in path.segments.chunks_exact(5) {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_strike_is_complete_and_compact() {
        let (s, g) = strike();
        assert_eq!((s.font, s.height, s.width, s.dpi), ('0', 32, 0, 203));
        assert_eq!(g.len(), 95);
        for (i, g) in g.iter().enumerate() {
            assert_eq!(g.codepoint, i as u8 + 32)
        }
        assert!(DATA.len() < 4500);
        assert_eq!(width("Wi i", 32.).unwrap(), 51.);
        assert!(glyph('é').is_err());
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
