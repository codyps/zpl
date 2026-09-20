//! ZD621 edge behavior measured in barcode-edges-zd621-v1. The ^FO/^FT
//! nominal placement is described in the ZPL Programming Guide pp. 201/205:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use crate::output::{Path, Segment, MAX_SEGMENTS};

pub(super) fn clamp(path: &mut Path, split: &[usize], reverse: bool) -> Result<(), String> {
    if split.is_empty() {
        return Ok(());
    }
    let mut rects = Vec::new();
    let mut changed = false;
    for pair in split.windows(2) {
        let part = &path.segments[pair[0]..pair[1]];
        let start = rects.len();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        // Barcode bars and resident bitmap glyphs are disjoint rectangles.
        // Preserve other path representations rather than approximate curves.
        let (chunks, remainder) = part.as_chunks::<5>();
        for chunk in chunks {
            let [Segment::Move(a), Segment::Line(b), Segment::Line(c), Segment::Line(d), Segment::Close] =
                chunk
            else {
                return Ok(());
            };
            if !((a.x == b.x && b.y == c.y && c.x == d.x && d.y == a.y)
                || (a.y == b.y && b.x == c.x && c.y == d.y && d.x == a.x))
            {
                return Ok(());
            }
            let (x0, y0, x1, y1) = (a.x.min(c.x), a.y.min(c.y), a.x.max(c.x), a.y.max(c.y));
            min_x = min_x.min(x0);
            min_y = min_y.min(y0);
            rects.push([x0, y0, x1, y1]);
        }
        if !remainder.is_empty() {
            return Ok(());
        }
        let dx = -min_x.min(0.);
        let dy = -min_y.min(0.);
        changed |= dx > 0. || dy > 0.;
        for r in &mut rects[start..] {
            r[0] += dx;
            r[2] += dx;
            r[1] += dy;
            r[3] += dy;
        }
    }
    if !changed && reverse {
        return Ok(());
    }
    if reverse {
        // ^FR/^LR invert each component independently: coincident bar and
        // caption ink toggles twice. The raw reverse-print controls pin this.
        let mut out = Path::default();
        for [x0, y0, x1, y1] in rects {
            out.rect(x0, y0, x1 - x0, y1 - y0);
        }
        *path = out;
        return Ok(());
    }
    // Union even without clamping: splitting a proportional caption into
    // glyphs must preserve the font renderer's union of overhanging strokes.
    // Sweep rectangle boundaries. Emit disjoint horizontal strips, so even-odd
    // adapters paint overlapping black ink exactly once.
    let mut events = Vec::with_capacity(2 * rects.len());
    for (i, r) in rects.iter().enumerate() {
        events.push((r[1], i, true));
        events.push((r[3], i, false));
    }
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut active = std::collections::BTreeSet::new();
    let mut out = Path::default();
    let mut index = 0;
    while index < events.len() {
        let y = events[index].0;
        while index < events.len() && events[index].0 == y {
            let (_, i, add) = events[index];
            if add {
                active.insert(i);
            } else {
                active.remove(&i);
            }
            index += 1;
        }
        if index == events.len() {
            break;
        }
        let h = events[index].0 - y;
        let mut spans: Vec<_> = active.iter().map(|&i| (rects[i][0], rects[i][2])).collect();
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Option<(f64, f64)> = None;
        for (left, right) in spans {
            if let Some((x, end)) = merged {
                if left <= end {
                    merged = Some((x, end.max(right)));
                    continue;
                }
                out.rect(x, y, end - x, h);
            }
            merged = Some((left, right));
        }
        if let Some((x, end)) = merged {
            out.rect(x, y, end - x, h);
        }
        if out.segments.len() > MAX_SEGMENTS {
            return Err("document path limit exceeded".into());
        }
    }
    *path = out;
    Ok(())
}
