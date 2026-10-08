//! Shared native bitmap fonts and captured scalable-font strikes.
//! Bitmap pixels become output-neutral paths.
//! Metrics: ZPL Programming Guide Tables 29/31, pp. 1582–1583:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
#[cfg(test)]
use crate::bitmap_font::GRAPHIC_SYMBOLS;
pub(super) use crate::fonts::resident::printer_ft_offset;
#[cfg(test)]
use crate::fonts::resident::{compact_glyph, resident, CP850};
use crate::fonts::resident::{glyph_from, selected, GlyphSet, GlyphView, Pixels, Selection};
use crate::output::{Path, Point, Segment};
use std::collections::BTreeMap;

/// Emit sorted bitmap rows, extending identical runs that touch vertically.
/// Joining only exactly shared edges preserves the even-odd region, including
/// fractional scales whose adjacent row boundaries can differ by an ULP.
#[derive(Default)]
struct RowPath {
    // Keep rectangles small while joining rows. Expanding every intermediate
    // rectangle to five general segments makes buffer growth depend heavily
    // on allocator fragmentation, including the font initialization history.
    rectangles: Vec<(Point, Point)>,
    previous: Vec<usize>,
    current: Vec<usize>,
    row: Option<f64>,
    next: usize,
}
impl RowPath {
    fn rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        if width <= 0. || height <= 0. {
            return;
        }
        if self.row != Some(y) {
            std::mem::swap(&mut self.previous, &mut self.current);
            self.current.clear();
            self.next = 0;
            self.row = Some(y);
        }
        let right = x + width;
        let bottom = y + height;
        while let Some(&index) = self.previous.get(self.next) {
            let (a, c) = self.rectangles[index];
            if a.x < x {
                self.next += 1;
                continue;
            }
            if a.x == x && c.x == right && c.y == y {
                self.rectangles[index].1.y = bottom;
                self.current.push(index);
                self.next += 1;
                return;
            }
            break;
        }
        self.current.push(self.rectangles.len());
        self.rectangles
            .push((Point::new(x, y), Point::new(right, bottom)));
    }

    fn into_path(self) -> Path {
        let mut segments = Vec::with_capacity(self.rectangles.len() * 5);
        for (a, c) in self.rectangles {
            // Preserve computed endpoints exactly, including fractional row
            // boundaries; do not subtract and re-add widths or heights.
            segments.extend([
                Segment::Move(a),
                Segment::Line(Point::new(c.x, a.y)),
                Segment::Line(c),
                Segment::Line(Point::new(a.x, c.y)),
                Segment::Close,
            ]);
        }
        Path { segments }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Font<'a> {
    custom: Option<&'a super::fonts::Face<'a>>,
    face: Selection,
    block_flow: Option<BlockFlow>,
}
impl std::ops::Deref for Font<'_> {
    type Target = Selection;
    fn deref(&self) -> &Selection {
        &self.face
    }
}
impl From<Font<'_>> for Selection {
    fn from(font: Font<'_>) -> Self {
        font.face
    }
}
#[derive(Clone, Copy)]
struct BlockFlow {
    direction: u8,
    gap: f64,
    vertical_gap: f64,
    printer_layout: bool,
    skip_space_gap: bool,
}
impl BlockFlow {
    fn gap_for(self, c: char) -> f64 {
        if self.skip_space_gap && self.direction != b'V' && c == ' ' {
            0.
        } else {
            self.gap
        }
    }
}

impl<'a> Font<'a> {
    pub(super) fn new(id: char, legacy_backslash: bool) -> Self {
        Self {
            custom: None,
            face: Selection::new(id, legacy_backslash),
            block_flow: None,
        }
    }
    pub(super) fn with_serial_zero_source(mut self, enabled: bool) -> Self {
        self.face = self.face.with_serial_zero_source(enabled);
        self
    }
    pub(super) fn with_fonts(mut self, fonts: &'a super::fonts::RenderFonts<'_, '_, '_>) -> Self {
        self.custom = fonts.get(self.id);
        let compact = self.custom.and_then(|face| face.compact());
        self.face = self
            .face
            .with_custom(self.custom.is_some() && compact.is_none())
            .with_compact(compact);
        self
    }
    pub(super) fn with_encoding(mut self, encoding: u8) -> Self {
        self.face = self.face.with_encoding(encoding);
        self
    }
    pub(super) fn with_legacy_codepage(mut self, enabled: bool) -> Self {
        self.face = self.face.with_legacy_codepage(enabled);
        self
    }
    pub(super) fn with_tab_stops(mut self, enabled: bool) -> Self {
        self.face = self.face.with_tab_stops(enabled);
        self
    }
    pub(super) fn with_character_map(mut self, map: Option<[u8; 256]>) -> Self {
        self.face = self.face.with_character_map(map);
        self
    }
    pub(super) fn with_explicit_sources(mut self, mask: Option<[u8; 32]>) -> Self {
        self.face = self.face.with_explicit_sources(mask);
        self
    }
    pub(super) fn with_control_glyphs(mut self, legacy: bool, spaces: bool) -> Self {
        self.face = self.face.with_control_glyphs(legacy, spaces);
        self
    }
    pub(super) fn with_default_glyph(mut self, enabled: bool) -> Self {
        self.face = self.face.with_default_glyph(enabled);
        self
    }
    pub(super) fn with_block_flow(
        mut self,
        direction: (u8, f64),
        compatibility: super::compatibility::Compatibility,
    ) -> Self {
        if direction != (b'H', 0.) {
            self.block_flow = Some(BlockFlow {
                direction: direction.0,
                gap: direction.1,
                vertical_gap: if compatibility.field_vertical_ignores_gap {
                    0.
                } else {
                    direction.1
                },
                printer_layout: compatibility.block_field_direction_printer_layout,
                skip_space_gap: compatibility.block_spaces_ignore_character_gap,
            });
        }
        self
    }
    pub(super) fn bounded_pitch(self, w: f64, h: f64) -> Result<f64, String> {
        if self.custom.is_some() {
            return Ok(h);
        }
        self.face.bounded_pitch(h, || width_for(self, "A", w, h))
    }
    pub(super) fn block_overprints(self) -> bool {
        self.block_flow
            .is_some_and(|flow| flow.printer_layout && flow.direction == b'V')
    }
    pub(super) fn block_position(self, position: f64) -> f64 {
        if self.block_reverses() {
            -position
        } else {
            position
        }
    }
    pub(super) fn block_reverses(self) -> bool {
        self.block_flow
            .is_some_and(|flow| flow.printer_layout && flow.direction == b'R')
    }
}
impl From<char> for Font<'_> {
    fn from(id: char) -> Self {
        Self::new(id, false)
    }
}
impl GlyphView<'_> {
    // Select storage once per glyph and scan packed bytes by runs. Skip
    // uniform bits together instead of dispatching and tracking span state
    // at every pixel, including for the retained scalable strikes.
    fn for_each_span(&self, mut emit: impl FnMut(usize, usize, usize)) {
        fn row_spans(bits: &[u8], offset: usize, width: usize, mut emit: impl FnMut(usize, usize)) {
            let mut x = 0;
            let mut start = None;
            while x < width {
                let bit = offset + x;
                let byte = bits[bit / 8] << (bit % 8);
                let remaining = (8 - bit % 8).min(width - x);
                if byte & 128 != 0 {
                    start.get_or_insert(x);
                    x += (byte.leading_ones() as usize).min(remaining);
                } else {
                    if let Some(left) = start.take() {
                        emit(left, x);
                    }
                    x += (byte.leading_zeros() as usize).min(remaining);
                }
            }
            if let Some(left) = start {
                emit(left, width);
            }
        }
        let width = self.width as usize;
        match &self.pixels {
            Pixels::Compact(g) => {
                for y in 0..self.height as usize {
                    row_spans(g.bitmap(), g.row_offset(y), width, |left, right| {
                        emit(y, left, right)
                    });
                }
            }
            Pixels::Custom(g) => {
                for (y, row) in g.bitmap.iter().enumerate() {
                    row_spans(row, 0, width, |left, right| emit(y, left, right));
                }
            }
        }
    }
    #[cfg(test)]
    fn pixel(&self, x: usize, y: usize) -> bool {
        if x >= self.width as usize || y >= self.height as usize {
            return false;
        }
        match &self.pixels {
            Pixels::Compact(g) => g.pixel(x as u16, y as u16),
            Pixels::Custom(g) => g.bitmap[y][x / 8] & (128 >> (x % 8)) != 0,
        }
    }
}
fn selected_for_char(
    font: Font<'_>,
    c: crate::fonts::resident::GlyphKey,
    w: f64,
    h: f64,
) -> (GlyphSet, f64, f64) {
    crate::fonts::resident::selected_for_char(font.face, c, w, h)
}
#[cfg(test)]
fn glyph(c: char) -> Result<GlyphView<'static>, String> {
    glyph_from(selected('0', 32., 32.).0, c)
}
#[cfg(test)]
fn baseline(h: f64) -> f64 {
    baseline_for('0', h)
}
#[cfg(test)]
fn width(s: &str, w: f64) -> Result<f64, String> {
    width_for('0', s, w, 32.)
}
// tabs-zd621-v1: TAB advances to the next 80-dot stop relative
// to the field or line origin, independently of the selected font matrix.
fn next_tab(pen: f64) -> f64 {
    (pen / 80.).floor().mul_add(80., 80.)
}
pub(super) fn width_for<'a>(
    id: impl Into<Font<'a>> + Copy,
    s: &str,
    w: f64,
    h: f64,
) -> Result<f64, String> {
    let font = id.into();
    s.chars().try_fold(0., |sum, c| {
        if font.is_tab(c) {
            return Ok(next_tab(sum));
        }
        let gap = font.block_flow.map_or(0., |flow| flow.gap_for(c));
        let c = font.map_char(c)?;
        let (g, sx, _) = resolved_glyph(font, c, w, h)?;
        Ok(sum + g.advance as f64 * sx + gap)
    })
}
/// ZD621 font-0 FO/I/right anchor: measure ink with a backwards pen.
/// The glyphs themselves are still printed in their normal field order.
/// See field-direction-zd621-v1 pair controls (notably jW versus Wj),
/// and Zebra Programming Guide ^FO p. 201 / Field Interactions pp. 1606–1611.
pub(super) fn inverted_text_margin(
    font: Font<'_>,
    value: &str,
    w: f64,
    h: f64,
    gap: f64,
) -> Result<f64, String> {
    let mut pen = 0.;
    let mut right = 0_f64;
    let mut first_advance = 0.;
    for (i, c) in value.chars().enumerate() {
        if font.is_tab(c) {
            let next = next_tab(pen);
            if i == 0 {
                first_advance = next;
            }
            pen = next;
            continue;
        }
        let c = font.map_char(c)?;
        let (g, sx, _) = resolved_glyph(font, c, w, h)?;
        if i == 0 {
            first_advance = g.advance as f64 * sx;
        }
        if g.width != 0 && g.height != 0 {
            right = right.max((g.left as f64 + g.width as f64) * sx - pen);
        }
        pen += g.advance as f64 * sx + gap;
    }
    Ok(first_advance - right)
}

pub(super) fn inverted_margin<'a>(
    id: impl Into<Font<'a>> + Copy,
    value: &str,
    w: f64,
    h: f64,
) -> Result<f64, String> {
    let font = id.into();
    let id = if font.custom.is_some() { '\0' } else { font.id };
    if let Some(margin) = crate::fonts::resident::inverted_margin(id, w) {
        return Ok(margin);
    }
    let Some(c) = value.chars().last() else {
        return Ok(0.);
    };
    if font.is_tab(c) {
        return Ok(0.);
    }
    let c = font.map_char(c)?;
    let (g, sx, _) = resolved_glyph(font, c, w, h)?;
    Ok(((g.advance as f64 - g.left as f64 - g.width as f64) * sx - 1.).max(0.))
}
#[cfg(test)]
fn text(s: &str, w: f64, h: f64) -> Result<Path, String> {
    text_for('0', s, w, h)
}
// Preserve individual glyph ink for printer edge placement. The regular
// text path remains merged, so unclamped output does not change.
pub(super) fn text_parts_for<'a>(
    id: impl Into<Font<'a>> + Copy,
    s: &str,
    w: f64,
    h: f64,
) -> Result<Vec<Path>, String> {
    let font = id.into();
    if let Some(flow) = font.block_flow {
        let plain = Font {
            block_flow: None,
            ..font
        };
        let (mut x, mut y) = (0., 0.);
        let mut parts = Vec::new();
        for (i, c) in s.chars().enumerate() {
            let value = c.to_string();
            let advance = width_for(plain, &value, w, h)? + flow.gap_for(c);
            if flow.direction == b'R' && (flow.printer_layout || i != 0) {
                x -= advance;
            }
            let mut path = text_for(plain, &value, w, h)?;
            path.transform(|p| crate::output::Point::new(p.x + x, p.y + y));
            if !path.segments.is_empty() {
                parts.push(path);
            }
            match flow.direction {
                b'H' => x += advance,
                b'V' if !flow.printer_layout => y += h + flow.vertical_gap,
                _ => {}
            }
        }
        return Ok(parts);
    }
    let mut parts = Vec::new();
    let mut pen = 0.;
    let baseline = baseline_for(font, h);
    for c in s.chars() {
        if font.is_tab(c) {
            pen = next_tab(pen);
            continue;
        }
        let c = font.map_char(c)?;
        let (g, sx, sy) = resolved_glyph(font, c, w, h)?;
        // A single bitmap glyph already has disjoint row spans. Unlike a
        // complete proportional string, it needs no BTreeMap or union/sort
        // pass to prevent even-odd cancellation of overlapping glyph ink.
        let mut path = RowPath::default();
        g.for_each_span(|y, left, right| {
            path.rect(
                (g.left as f64 + left as f64) * sx,
                baseline + (g.top + y as i32) as f64 * sy,
                (right - left) as f64 * sx,
                sy,
            );
        });
        let mut path = path.into_path();
        path.transform(|p| crate::output::Point::new(p.x + pen, p.y));
        pen += g.advance as f64 * sx;
        if !path.segments.is_empty() {
            parts.push(path);
        }
    }
    Ok(parts)
}
pub(super) fn text_for<'a>(
    id: impl Into<Font<'a>> + Copy,
    s: &str,
    w: f64,
    h: f64,
) -> Result<Path, String> {
    let font = id.into();
    if font.block_flow.is_some() || font.custom.is_some() {
        // ^FB pp. 186–187 wraps by advances including the ^FP gap (p. 202).
        // Captured printer V fields overprint each line; R predecrements even
        // the first character. Union preserves overlapping glyph ink.
        let mut path = Path::default();
        for part in text_parts_for(font, s, w, h)? {
            path.segments.extend(part.segments);
        }
        return Ok(union_lines(path));
    }
    let (mut glyphs, mut sx, mut sy) = selected(font, w, h);
    if font.has_glyph_fallback() {
        for c in s.chars().filter(|&c| !font.is_tab(c)) {
            let c = font.map_char(c)?;
            if glyph_from(glyphs, c).is_err() {
                if s.chars().count() == 1 {
                    (glyphs, sx, sy) = selected_for_char(font, c, w, h);
                } else {
                    // Mixed strikes have different bitmap units. Compose in
                    // output coordinates, using each glyph's own advances.
                    let mut path = Path::default();
                    for part in text_parts_for(font, s, w, h)? {
                        path.segments.extend(part.segments);
                    }
                    return Ok(union_lines(path));
                }
            }
        }
    }
    // Merge ink spans before emitting even-odd subpaths. Proportional glyphs can
    // overhang their advance; overlapping strokes must remain black, not XOR.
    let mut rows: BTreeMap<i32, Vec<(f64, f64)>> = BTreeMap::new();
    let mut pen = 0.;
    let baseline = baseline_for(font, h);
    // Cap the estimate: a long field may contain mostly advancing blanks.
    let row_capacity = (s.chars().count() * 2).min(64);
    for c in s.chars() {
        if font.is_tab(c) {
            pen = next_tab(pen * sx) / sx;
            continue;
        }
        let g = glyph_from(glyphs, font.map_char(c)?)?;
        g.for_each_span(|y, left, right| {
            rows.entry(g.top + y as i32)
                .or_insert_with(|| Vec::with_capacity(row_capacity))
                .push((
                    pen + g.left as f64 + left as f64,
                    pen + g.left as f64 + right as f64,
                ));
        });
        pen += g.advance as f64;
    }
    let mut path = RowPath::default();
    for (y, mut spans) in rows {
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Option<(f64, f64)> = None;
        for (a, b) in spans {
            if let Some((left, right)) = merged {
                if a <= right {
                    merged = Some((left, right.max(b)));
                } else {
                    path.rect(left * sx, baseline + y as f64 * sy, (right - left) * sx, sy);
                    merged = Some((a, b));
                }
            } else {
                merged = Some((a, b));
            }
        }
        if let Some((left, right)) = merged {
            path.rect(left * sx, baseline + y as f64 * sy, (right - left) * sx, sy);
        }
    }
    Ok(path.into_path())
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

/// Discard wholly invisible text rectangles before applying the document path
/// budget. Long hanging-indent continuations can extend far beyond the label;
/// partially visible rectangles keep their original geometry and clipping.
pub(super) fn cull_outside(path: &mut Path, width: u32, height: u32) {
    use crate::output::Segment;
    let mut visible = 0;
    for start in (0..path.segments.len()).step_by(5) {
        let [Segment::Move(a), Segment::Line(_), Segment::Line(c), Segment::Line(_), Segment::Close] =
            &path.segments[start..start + 5]
        else {
            unreachable!("font ink consists of rectangles")
        };
        // Quarter-turn text rotations preserve axis-aligned rectangles. A/C
        // are opposite corners, including inverted and bottom-up placement.
        let (left, right) = (a.x.min(c.x), a.x.max(c.x));
        let (top, bottom) = (a.y.min(c.y), a.y.max(c.y));
        if right > 0. && bottom > 0. && left < width as f64 && top < height as f64 {
            if start != visible {
                for offset in 0..5 {
                    path.segments.swap(visible + offset, start + offset);
                }
            }
            visible += 5;
        }
    }
    path.segments.truncate(visible);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_span_scans_preserve_every_glyph_pixel() {
        let check = |g: GlyphView<'_>| {
            let mut pixels = vec![vec![false; g.width as usize]; g.height as usize];
            g.for_each_span(|y, left, right| {
                assert!(left < right);
                for pixel in &mut pixels[y][left..right] {
                    assert!(!*pixel, "spans must not overlap");
                    *pixel = true;
                }
            });
            for (y, row) in pixels.iter().enumerate() {
                for (x, &pixel) in row.iter().enumerate() {
                    assert_eq!(pixel, g.pixel(x, y), "pixel {x},{y}");
                }
            }
        };
        for id in ['A', 'B', 'D', 'E', 'F', 'G', 'H', GRAPHIC_SYMBOLS] {
            let face = resident(id).unwrap();
            for key in 0..=255 {
                let c = crate::fonts::resident::GlyphKey::Source(key as u8);
                if let Ok(g) = glyph_from(GlyphSet::Compact(face, false, 28), c) {
                    check(g);
                }
            }
        }
        for id in ['0', 'P', 'Q', 'R', 'S', 'T', 'U', 'V'] {
            for size in [18., 28., 32., 40., 64.] {
                let (set, _, _) = selected(id, size, size);
                for c in ' '..='~' {
                    check(glyph_from(set, c).unwrap());
                }
            }
        }
    }

    #[test]
    fn measured_input_maps_render_characters_outside_cp850() {
        // ZD621 measured CI27/28 maps include these inputs, which the old
        // Unicode-to-CP850 source lookup could not resolve.
        use zpl_bitmap_fonts::collection::Encoding;
        let face = resident('A').unwrap();
        for (character, byte) in [('Œ', 0x8c), ('…', 0x85), ('™', 0x99), ('€', 0x80)] {
            assert!(!CP850.contains(&character));
            let expected = face
                .encoded_glyph(Encoding::Input { ci: 27 }, byte)
                .unwrap();
            for ci in [27, 28] {
                let font = Font::new('A', false).with_encoding(ci);
                let (set, _, _) = selected(font, 5., 9.);
                let actual = glyph_from(set, font.map_char(character).unwrap()).unwrap();
                assert_eq!(
                    (
                        actual.advance,
                        actual.left,
                        actual.top,
                        actual.width,
                        actual.height
                    ),
                    (
                        u32::from(expected.advance),
                        i32::from(expected.left),
                        i32::from(expected.top),
                        u32::from(expected.width),
                        u32::from(expected.height)
                    )
                );
                for y in 0..actual.height {
                    for x in 0..actual.width {
                        assert_eq!(
                            actual.pixel(x as usize, y as usize),
                            expected.pixel(x as u16, y as u16)
                        );
                    }
                }
            }
        }
        assert!(compact_glyph('漢', face, false, 28).is_none());
    }

    #[test]
    fn resident_bitmap_faces_read_shared_pool_and_preserve_source_mapping() {
        for id in ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', GRAPHIC_SYMBOLS] {
            let native = resident(id).unwrap();
            let metrics = native.cell_metrics().unwrap();
            let font = Font::new(id, false);
            let (set, sx, sy) = selected(
                font,
                f64::from(metrics.cell_width),
                f64::from(metrics.cell_height),
            );
            assert!(matches!(set, GlyphSet::Compact(..)));
            assert_eq!((sx, sy), (1., 1.));
            assert_eq!(
                baseline_for(font, f64::from(metrics.cell_height)),
                f64::from(metrics.baseline) - 1.
            );
            for (c, key) in [
                ('A', 65),
                ('¢', 189),
                ('é', 130),
                ('\u{ad}', 240),
                ('ð', 208),
            ] {
                let g = glyph_from(set, c).unwrap();
                let expected = native.glyph(key).unwrap();
                assert_eq!(g.advance, u32::from(expected.advance));
                for y in 0..g.height as usize {
                    for x in 0..g.width as usize {
                        assert_eq!(g.pixel(x, y), expected.pixel(x as u16, y as u16));
                    }
                }
            }
        }
        let mut map = std::array::from_fn(|i| i as u8);
        map[usize::from(b'A')] = b'\\';
        let f = Font::new('A', false).with_character_map(Some(map));
        let (set, _, _) = selected(f, 5., 9.);
        let remapped = glyph_from(set, f.map_char('A').unwrap()).unwrap();
        let legacy = resident('A').unwrap().glyph(92).unwrap();
        assert_eq!(
            (remapped.width, remapped.height),
            (u32::from(legacy.width), u32::from(legacy.height))
        );
        assert_eq!(
            compact_glyph('\\', resident('A').unwrap(), false, 28)
                .unwrap()
                .width,
            resident('A').unwrap().glyph(31).unwrap().width
        );
        assert_eq!(
            compact_glyph('\\', resident('A').unwrap(), true, 28)
                .unwrap()
                .width,
            resident('A').unwrap().glyph(92).unwrap().width
        );
        assert!(matches!(selected('0', 32., 32.).0, GlyphSet::Captured(..)));
        assert!(matches!(selected('P', 18., 20.).0, GlyphSet::Captured(..)));
    }

    #[test]
    fn joined_rows_preserve_exact_edges_and_even_odd_pixels() {
        use crate::output::{raster::rasterize, Draw, Paint, Scene};
        // Output-independent even-odd path contract: docs/local-renderer.md.
        // Test exact joins, gaps, changing widths, and fractional rounding.
        for scale in [0.1, 0.5, 1., 1.1, 1.5, 2.3] {
            let mut rows = RowPath::default();
            let mut original = Path::default();
            for y in 0..12 {
                for (x, width) in [(1., 2.), (5., if y % 3 == 0 { 2. } else { 3. })] {
                    if y == 7 {
                        continue;
                    }
                    let args = (x * scale, y as f64 * scale, width * scale, scale);
                    rows.rect(args.0, args.1, args.2, args.3);
                    original.rect(args.0, args.1, args.2, args.3);
                }
            }
            let rows = rows.into_path();
            if scale == 1. {
                assert!(rows.segments.len() < original.segments.len() / 2);
            }
            for paint in [Paint::Black, Paint::White, Paint::Invert] {
                let mut background = Path::default();
                background.rect(0., 3., 20., 12.);
                let mut scene = Scene::new(32, 32, 203).unwrap();
                scene.draws.push(Draw {
                    path: background,
                    paint: Paint::Black,
                });
                scene.draws.push(Draw {
                    path: original.clone(),
                    paint,
                });
                let expected = rasterize(&scene).unwrap();
                scene.draws[1].path = rows.clone();
                assert_eq!(
                    rasterize(&scene).unwrap(),
                    expected,
                    "scale={scale}, paint={paint:?}"
                );
            }
        }
        let mut path = RowPath::default();
        path.rect(0., 0., 1., 1.);
        path.rect(0., 1f64.next_up(), 1., 1.);
        assert_eq!(
            path.into_path().segments.len(),
            10,
            "do not fill even a subpixel gap"
        );
    }

    #[test]
    fn individual_parts_match_full_glyph_layout() {
        // Preserve captured strike geometry and advances, including scaled
        // fallbacks and remapped controls (Zebra ^CI p. 158, Tables 29/31).
        for id in [
            '0', 'A', 'B', 'D', 'E', 'F', 'G', 'H', 'P', 'Q', 'R', 'S', 'T', 'U', 'V',
        ] {
            for (w, h) in [(10., 16.), (16., 24.), (32., 32.), (17.3, 29.7)] {
                for font in [
                    Font::new(id, false),
                    Font::new(id, true)
                        .with_default_glyph(true)
                        .with_control_glyphs(true, false),
                ] {
                    for c in (' '..='~').chain(['¢', 'é', 'א', '\u{1b}', '\u{7f}']) {
                        let value = c.to_string();
                        let expected = text_for(font, &value, w, h);
                        let actual = text_parts_for(font, &value, w, h);
                        match (expected, actual) {
                            (Ok(expected), Ok(parts)) => {
                                let actual = Path {
                                    segments: parts.into_iter().flat_map(|p| p.segments).collect(),
                                };
                                assert_eq!(actual, expected, "{id} {c:?} {w}x{h}");
                            }
                            (Err(expected), Err(actual)) => assert_eq!(actual, expected),
                            other => panic!("glyph layout disagrees: {other:?}"),
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn ascii_strikes_preserve_enriched_fallback_glyphs() {
        // ^CI / ^PA and legacy control behavior: use the same enriched
        // 32-dot base face and scaling available before these ASCII captures.
        for (h, w) in [
            (18., 22.),
            (20., 24.),
            (22., 26.),
            (23., 23.),
            (26., 30.),
            (28., 32.),
            (28., 28.),
            (30., 34.),
            (39., 42.),
            (45., 44.),
            (72., 68.),
        ] {
            for font in [
                Font::new('0', false),
                Font::new('0', true),
                Font::new('0', false).with_default_glyph(true),
                Font::new('0', true).with_default_glyph(true),
                Font::new('0', true).with_control_glyphs(true, false),
                Font::new('0', true)
                    .with_default_glyph(true)
                    .with_control_glyphs(true, false),
            ] {
                let mut characters = vec!['¢', 'é', 'ð', '\u{378}', 'א', 'ب'];
                if font.legacy_backslash {
                    characters.push('\\');
                }
                if font.legacy_controls {
                    characters.extend(['\u{1b}', '\u{7f}']);
                }
                for c in characters {
                    let value = c.to_string();
                    let mut expected = text_for(font, &value, 32., 32.).unwrap();
                    expected.transform(|p| crate::output::Point::new(p.x * w / 32., p.y * h / 32.));
                    let actual = text_for(font, &value, w, h).unwrap();
                    assert_eq!(actual, expected, "{c:?} at {h}x{w}");
                    assert_eq!(
                        width_for(font, &value, w, h).unwrap(),
                        width_for(font, &value, 32., 32.).unwrap() * w / 32.
                    );
                }
                // ASCII remains native, even in a field containing fallback
                // glyphs; width and ink composition agree across both scales.
                let value = "A¢Wé";
                let expected_width: f64 = value
                    .chars()
                    .map(|c| width_for(font, &c.to_string(), w, h).unwrap())
                    .sum();
                assert_eq!(width_for(font, value, w, h).unwrap(), expected_width);
                let mut expected = Path::default();
                for part in text_parts_for(font, value, w, h).unwrap() {
                    expected.segments.extend(part.segments);
                }
                assert_eq!(text_for(font, value, w, h).unwrap(), union_lines(expected));
            }
        }
    }
    #[test]
    fn embedded_strike_is_complete_and_compact() {
        let s = *zpl_bitmap_fonts::captures::zd621::VARIANTS[0]
            .last()
            .unwrap();
        assert_eq!((s.font, s.height, s.width, s.dpi), ('0', 32, 0, 203));
        assert_eq!(
            s.keys,
            &(32..=126)
                .chain([162, 173, 233, 240])
                .chain([0x378])
                .chain(0x5d0..=0x5ea)
                .chain([0x627, 0x628, 0x62d, 0x631, 0x645])
                .collect::<Vec<_>>()
        );
        assert_eq!(width("Wi i", 32.).unwrap(), 51.);
        assert!(glyph('é').is_ok());
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

#[derive(Clone, Default)]
pub(super) struct DirectionMetrics {
    pub pivot: Option<f64>,
    pub end_margin: f64,
    pub bottom_margin: f64,
    pub end_left: f64,
    pub first_delta: f64,
    pub first_ink: (f64, f64),
    pub count: usize,
    pub vertical_extent: (f64, f64),
    pub leading_descent: f64,
    pub leading_top: f64,
}
pub(super) struct DirectedText {
    pub path: Path,
    pub parts: Vec<Path>,
    pub size: (f64, f64),
    pub metrics: DirectionMetrics,
}
/// ^FP, Zebra Programming Guide p. 202 and Field Interactions pp. 1606–1611.
pub(super) fn directed_text(
    font: Font<'_>,
    value: &str,
    w: f64,
    h: f64,
    direction: (u8, f64),
    compatibility: super::compatibility::Compatibility,
    right_justified: bool,
) -> Result<DirectedText, String> {
    let (d, gap) = direction;
    let mut parts = Vec::new();
    let mut path = Path::default();
    let (mut x, mut y) = (0., 0.);
    let mut width = 0_f64;
    let mut tab_pen = 0.;
    let mut end_left = 0_f64;
    let mut first_advance = 0_f64;
    let mut first_ink = (0_f64, 0_f64);
    let mut leading_top = f64::INFINITY;
    let mut leading_descent = 0.;
    let mut max_advance = 0_f64;
    let mut max_right = 0_f64;
    let (mut end_margin, mut bottom) = (0_f64, 0_f64);
    let last_advance = value
        .chars()
        .last()
        .map(|c| width_for(font, &c.to_string(), w, h))
        .transpose()?
        .unwrap_or(0.)
        + gap;
    for (index, c) in value.chars().enumerate() {
        let text = c.to_string();
        let advance = if font.is_tab(c) && d != b'V' {
            next_tab(tab_pen) - tab_pen
        } else {
            width_for(font, &text, w, h)? + gap
        };
        tab_pen += advance;
        max_advance = max_advance.max(advance);
        let mut part = text_for(font, &text, w, h)?;
        if d == b'R' && index != 0 {
            x -= advance;
        }
        if d == b'V' && right_justified {
            x = last_advance - advance;
        }
        let mut right = 0_f64;
        let mut left = f64::INFINITY;
        if index == 0 {
            first_advance = advance;
        }
        for segment in &part.segments {
            if let crate::output::Segment::Move(p) | crate::output::Segment::Line(p) = segment {
                left = left.min(p.x);
                right = right.max(p.x);
                bottom = bottom.max(p.y);
                if index == 0 {
                    leading_top = leading_top.min(p.y);
                    first_ink.0 = first_ink.0.max(p.x);
                    first_ink.1 = first_ink.1.max(p.y);
                }
            }
        }
        max_right = max_right.max(right);
        end_margin = advance - right;
        end_left = if left.is_finite() { left } else { 0. };
        part.transform(|p| crate::output::Point::new(p.x + x, p.y + y));
        path.segments.extend(part.segments.iter().cloned());
        parts.push(part);
        width = if d == b'V' { last_advance } else { x + advance };
        if d == b'H' {
            x += advance;
        }
        if d == b'V' {
            y += h + if compatibility.field_vertical_ignores_gap {
                0.
            } else {
                gap
            };
        }
    }
    if d == b'V' && compatibility.field_direction_printer_anchors {
        let capital = text_for(font, "H", w, h)?;
        let capital_bottom = capital
            .segments
            .iter()
            .filter_map(|s| match s {
                crate::output::Segment::Move(p) | crate::output::Segment::Line(p) => Some(p.y),
                _ => None,
            })
            .fold(0_f64, f64::max);
        leading_descent = (first_ink.1 - capital_bottom).max(0.);
        y += leading_descent;
    }
    Ok(DirectedText {
        path: union_lines(path),
        parts,
        size: (width, if d == b'V' { y } else { h }),
        metrics: DirectionMetrics {
            pivot: (d != b'H').then_some(w - font.direction_pivot_inset()),
            end_left,
            first_ink,
            count: value.chars().count(),
            vertical_extent: (max_right, max_advance - last_advance),
            leading_descent,
            leading_top: if leading_top.is_finite() {
                leading_top
            } else {
                0.
            },
            first_delta: first_advance - last_advance,
            end_margin,
            bottom_margin: h - bottom,
        },
    })
}

pub(super) fn baseline_for<'a>(id: impl Into<Font<'a>> + Copy, h: f64) -> f64 {
    let font = id.into();
    font.custom.map_or_else(
        || crate::fonts::resident::baseline_for(font.face, h),
        |custom| custom.baseline(h),
    )
}
pub(super) fn block_metrics<'a>(id: impl Into<Font<'a>>, h: f64, printer_s: bool) -> (f64, f64) {
    let font = id.into();
    if font.custom.is_some() {
        (h, 0.)
    } else {
        crate::fonts::resident::block_metrics(font.face, h, printer_s)
    }
}
fn resolved_glyph<'a>(
    font: Font<'a>,
    key: crate::fonts::resident::GlyphKey,
    w: f64,
    h: f64,
) -> Result<(GlyphView<'a>, f64, f64), String> {
    crate::fonts::resolve_glyph(font.face, font.custom, key, w, h)
}
