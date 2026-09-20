//! ^TB bounded text, Zebra Programming Guide p. 356:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use super::{font, TextLayout};
use crate::output::{Path, Point};

pub(super) struct LayoutOptions {
    pub right: bool,
    pub printer_pitch: bool,
    pub bidi: Option<super::compatibility::Compatibility>,
}

pub(super) fn layout(
    font: font::Font,
    text: &str,
    size: (f64, f64),
    requested_height: f64,
    bounds: (f64, f64),
    options: LayoutOptions,
) -> Result<TextLayout, String> {
    let LayoutOptions {
        right,
        printer_pitch,
        bidi,
    } = options;
    let (w, h) = size;
    let (width, height) = bounds;
    let mut decoded = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\u{ad}' => {} // ^TB explicitly ignores soft hyphens.
            '<' => {
                let mut escape = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '>' {
                        closed = true;
                        break;
                    }
                    escape.push(c);
                }
                if !closed {
                    break;
                }
                if escape.trim_end() == "<" {
                    decoded.push('<');
                }
            }
            c if c.is_whitespace() => decoded.push(' '),
            c => decoded.push(c),
        }
    }
    let measure = |s: &str| {
        if let Some(compat) = bidi {
            let visual = super::advanced_text::reorder(
                s,
                compat.bidi_skips_paired_bracket_resolution,
                compat.bidi_isolates_as_missing_glyphs,
            );
            font::width_for(font, &visual, w, h)
        } else {
            font::width_for(font, s, w, h)
        }
    };
    let mut lines = Vec::new();
    let mut line = String::new();
    let chars: Vec<_> = decoded.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            let next = format!("{line} ");
            if !line.is_empty() && measure(&next)? > width {
                let overflow = measure(&line)? > width;
                lines.push(std::mem::take(&mut line));
                if overflow {
                    line.push(' ');
                }
            } else {
                line.push(' ');
            }
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();
        if !line.is_empty() && measure(&format!("{line}{word}"))? > width {
            if !line.trim_end().is_empty() {
                line.truncate(line.trim_end().len());
            }
            lines.push(std::mem::take(&mut line));
        }
        for c in word.chars() {
            if !line.is_empty() && measure(&format!("{line}{c}"))? > width {
                lines.push(std::mem::take(&mut line));
            }
            line.push(c);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    let pitch = if printer_pitch {
        font.bounded_pitch(w, h)? * requested_height / h
    } else {
        h
    };
    let mut cursor = 0;
    let mut path = Path::default();
    let mut parts = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let y = (i as f64 * pitch).round();
        if y >= height {
            break;
        }
        let visual;
        let line = if let Some(compat) = bidi {
            let start = cursor
                + decoded[cursor..]
                    .find(line)
                    .ok_or("invalid TB wrap range")?;
            cursor = start + line.len();
            visual = super::advanced_text::reorder_range(
                &decoded,
                start..cursor,
                compat.bidi_skips_paired_bracket_resolution,
                compat.bidi_isolates_as_missing_glyphs,
            );
            &visual
        } else {
            line
        };
        let x = if right { -measure(line)? } else { 0. };
        for mut part in font::text_parts_for(font, line, w, h)? {
            // Captured TB previews clip the bottom, retaining ascender ink
            // above a line origin. Font paths are unions of axis-aligned runs.
            part.transform(|p| Point::new(p.x + x, (p.y + y).min(height)));
            path.segments.extend(part.segments.iter().cloned());
            parts.push(part);
        }
    }
    Ok(TextLayout {
        path,
        baseline: height,
        center_overflow: vec![false; parts.len()],
        parts,
    })
}

pub(super) fn position(
    p: Point,
    bounds: (f64, f64),
    rotation: u8,
    baseline: bool,
    right: bool,
    proportional: bool,
    printer: bool,
) -> Point {
    let (w, h) = bounds;
    let dot = if proportional && printer { 1. } else { 0. };
    let (a, b) = match rotation {
        b'R' => (-p.y, p.x),
        b'I' => (-p.x, -p.y),
        b'B' => (p.y, -p.x),
        _ => (p.x, p.y),
    };
    if !printer {
        // Ordinary rectangle pivots with FT at the block's bottom edge;
        // per-line justification has already selected its horizontal origin.
        let (dx, dy) = if baseline {
            match rotation {
                b'R' => (h, 0.),
                b'I' => (0., h),
                b'B' => (-h, 0.),
                _ => (0., -h),
            }
        } else {
            let width = if right { 0. } else { w };
            match rotation {
                b'R' => (h, 0.),
                b'I' => (width, h),
                b'B' => (0., width),
                _ => (0., 0.),
            }
        };
        return Point::new(a + dx, b + dy);
    }
    let (dx, dy) = if baseline {
        match rotation {
            b'R' => (h - dot, 0.),
            b'I' => (1. - dot, h - dot),
            b'B' => (-h, 1. - dot),
            _ => (0., -h),
        }
    } else if right {
        match rotation {
            b'R' => (-dot, w),
            b'I' => (1. - w - dot, h - dot),
            b'B' => (-h, 1. - dot),
            _ => (0., 0.),
        }
    } else {
        match rotation {
            b'R' => (h - dot, 0.),
            b'I' => (w + 1. - dot, h - dot),
            b'B' => (0., w + 1. - dot),
            _ => (0., 0.),
        }
    };
    Point::new(a + dx, b + dy)
}
