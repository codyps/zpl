//! Original bounded SFNT reader. See the table-specific sections at
//! https://learn.microsoft.com/en-us/typography/opentype/spec/otff
use super::*;
use std::{collections::BTreeMap, ops::Range};

const MAX_POINTS: usize = 16_384;
const MAX_PROGRAM: usize = 65_536;

#[derive(Default)]
pub(super) struct Budget {
    nodes: usize,
    points: usize,
    work: usize,
}

fn slice(data: &[u8], offset: usize, count: usize) -> Result<&[u8]> {
    data.get(
        offset
            ..offset
                .checked_add(count)
                .ok_or_else(|| invalid("font offset overflow"))?,
    )
    .ok_or_else(|| invalid("truncated TrueType table"))
}
fn u16_at(data: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_be_bytes(
        slice(data, offset, 2)?.try_into().unwrap(),
    ))
}
fn i16_at(data: &[u8], offset: usize) -> Result<i16> {
    Ok(u16_at(data, offset)? as i16)
}
fn u32_at(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(
        slice(data, offset, 4)?.try_into().unwrap(),
    ))
}

/// A validated quadratic TrueType container. The caller retains the font bytes.
/// Limits are independent of font-supplied `maxp` claims.
pub struct Font<'a> {
    data: &'a [u8],
    tables: BTreeMap<[u8; 4], Range<usize>>,
    pub(crate) units: u16,
    glyphs: u16,
    metrics: u16,
    long_loca: bool,
    cmap: Range<usize>,
    pub(crate) twilight: usize,
    pub(crate) storage: usize,
}
impl<'a> Font<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        if data.len() > 16 * 1024 * 1024 || u32_at(data, 0)? != 0x00010000 {
            return Err(invalid("expected bounded quadratic TrueType SFNT"));
        }
        let count = u16_at(data, 4)? as usize;
        if count == 0 || count > 64 {
            return Err(invalid("unsupported SFNT table count"));
        }
        slice(data, 12, count * 16)?;
        let mut tables = BTreeMap::new();
        for i in 0..count {
            let p = 12 + i * 16;
            let tag = slice(data, p, 4)?.try_into().unwrap();
            let offset = u32_at(data, p + 8)? as usize;
            let length = u32_at(data, p + 12)? as usize;
            slice(data, offset, length)?;
            if offset < 12 + count * 16 || tables.insert(tag, offset..offset + length).is_some() {
                return Err(invalid("overlapping directory or duplicate SFNT table"));
            }
        }
        let table = |tag: &[u8; 4]| -> Result<&[u8]> {
            Ok(&data[tables
                .get(tag)
                .ok_or_else(|| invalid("required TrueType table absent"))?
                .clone()])
        };
        for tag in [b"CFF ", b"CFF2", b"fvar", b"gvar"] {
            if tables.contains_key(tag) {
                return Err(invalid("unsupported CFF or variable font"));
            }
        }
        let head = table(b"head")?;
        if u32_at(head, 12)? != 0x5f0f3cf5 || i16_at(head, 52)? != 0 {
            return Err(invalid("invalid TrueType head table"));
        }
        let units = u16_at(head, 18)?;
        if !(16..=16384).contains(&units) {
            return Err(invalid("invalid units per em"));
        }
        let long_loca = match i16_at(head, 50)? {
            0 => false,
            1 => true,
            _ => return Err(invalid("unsupported loca format")),
        };
        let maxp = table(b"maxp")?;
        if u32_at(maxp, 0)? != 0x00010000 {
            return Err(invalid("unsupported maxp version"));
        }
        slice(maxp, 0, 32)?;
        let glyphs = u16_at(maxp, 4)?;
        let twilight = u16_at(maxp, 16)? as usize;
        let storage = u16_at(maxp, 18)? as usize;
        if glyphs == 0 || twilight > 4096 || storage > 4096 {
            return Err(invalid("font exceeds interpreter allocation limits"));
        }
        let metrics = u16_at(table(b"hhea")?, 34)?;
        if metrics == 0 || metrics > glyphs {
            return Err(invalid("invalid horizontal metric count"));
        }
        slice(
            table(b"hmtx")?,
            0,
            metrics as usize * 4 + (glyphs - metrics) as usize * 2,
        )?;
        let loca = table(b"loca")?;
        slice(
            loca,
            0,
            (glyphs as usize + 1) * if long_loca { 4 } else { 2 },
        )?;
        let glyf_len = table(b"glyf")?.len();
        let mut previous = 0;
        for i in 0..=glyphs as usize {
            let offset = if long_loca {
                u32_at(loca, i * 4)? as usize
            } else {
                u16_at(loca, i * 2)? as usize * 2
            };
            if offset < previous || offset > glyf_len {
                return Err(invalid("invalid glyph location"));
            }
            previous = offset;
        }
        let cmap_table = table(b"cmap")?;
        let records = u16_at(cmap_table, 2)? as usize;
        if records > 64 {
            return Err(invalid("too many cmap records"));
        }
        slice(cmap_table, 4, records * 8)?;
        let mut chosen = None;
        for i in 0..records {
            let p = 4 + 8 * i;
            let platform = u16_at(cmap_table, p)?;
            let encoding = u16_at(cmap_table, p + 2)?;
            if !(platform == 0 || platform == 3 && matches!(encoding, 1 | 10)) {
                continue;
            }
            let offset = u32_at(cmap_table, p + 4)? as usize;
            let format = u16_at(cmap_table, offset)?;
            let length = match format {
                4 => u16_at(cmap_table, offset + 2)? as usize,
                12 => u32_at(cmap_table, offset + 4)? as usize,
                _ => continue,
            };
            slice(cmap_table, offset, length)?;
            if chosen
                .as_ref()
                .is_none_or(|(priority, _)| format > *priority)
            {
                chosen = Some((format, offset..offset + length));
            }
        }
        let (_, cmap) = chosen.ok_or_else(|| invalid("no supported Unicode cmap (4 or 12)"))?;
        let base = tables[b"cmap"].start;
        let font = Self {
            data,
            tables,
            units,
            glyphs,
            metrics,
            long_loca,
            cmap: base + cmap.start..base + cmap.end,
            twilight,
            storage,
        };
        font.validate_cmap()?;
        Ok(font)
    }
    pub fn units_per_em(&self) -> u16 {
        self.units
    }
    pub fn glyph_count(&self) -> u16 {
        self.glyphs
    }
    pub fn table(&self, tag: &[u8; 4]) -> Option<&'a [u8]> {
        self.tables.get(tag).map(|r| &self.data[r.clone()])
    }
    pub fn glyph_index(&self, character: char) -> Option<u16> {
        let c = character as u32;
        let data = &self.data[self.cmap.clone()];
        let glyph = if u16_at(data, 0).ok()? == 12 {
            let count = u32_at(data, 12).ok()? as usize;
            let mut lo = 0;
            let mut hi = count;
            while lo < hi {
                let mid = (lo + hi) / 2;
                if u32_at(data, 16 + mid * 12 + 4).ok()? < c {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            if lo == count {
                return None;
            }
            let p = 16 + lo * 12;
            let start = u32_at(data, p).ok()?;
            if c < start {
                return None;
            }
            u32_at(data, p + 8).ok()?.checked_add(c - start)?
        } else {
            if c > 65535 {
                return None;
            }
            let n = u16_at(data, 6).ok()? as usize / 2;
            let i =
                (0..n).find(|&i| u16_at(data, 14 + i * 2).ok().is_some_and(|v| v as u32 >= c))?;
            let start = u16_at(data, 16 + n * 2 + i * 2).ok()? as u32;
            if c < start {
                return None;
            }
            let delta = u16_at(data, 16 + n * 4 + i * 2).ok()?;
            let p = 16 + n * 6 + i * 2;
            let offset = u16_at(data, p).ok()? as usize;
            let value = if offset == 0 {
                c as u16
            } else {
                u16_at(data, p + offset + (c - start) as usize * 2).ok()?
            };
            if offset != 0 && value == 0 {
                return None;
            }
            value.wrapping_add(delta) as u32
        };
        (glyph > 0 && glyph < self.glyphs as u32).then_some(glyph as u16)
    }
    fn validate_cmap(&self) -> Result<()> {
        let data = &self.data[self.cmap.clone()];
        if u16_at(data, 0)? == 12 {
            let count = u32_at(data, 12)? as usize;
            if count > 65536 {
                return Err(invalid("cmap group limit exceeded"));
            }
            slice(data, 16, count * 12)?;
            let mut previous = None;
            for i in 0..count {
                let p = 16 + i * 12;
                let a = u32_at(data, p)?;
                let b = u32_at(data, p + 4)?;
                if a > b
                    || b > 0x10ffff
                    || previous.is_some_and(|v| a <= v)
                    || u32_at(data, p + 8)?
                        .checked_add(b - a)
                        .is_none_or(|g| g >= self.glyphs as u32)
                {
                    return Err(invalid("invalid cmap group"));
                }
                previous = Some(b);
            }
        } else {
            let n = u16_at(data, 6)? as usize / 2;
            if n == 0 || n > 8192 || u16_at(data, 6)? % 2 != 0 {
                return Err(invalid("invalid cmap segment count"));
            }
            slice(data, 0, 16 + n * 8)?;
            let mut previous = None;
            for i in 0..n {
                let a = u16_at(data, 16 + n * 2 + i * 2)?;
                let b = u16_at(data, 14 + i * 2)?;
                if a > b || previous.is_some_and(|v| a <= v) {
                    return Err(invalid("invalid cmap segment"));
                }
                let p = 16 + n * 6 + i * 2;
                let offset = u16_at(data, p)? as usize;
                if offset != 0 {
                    slice(data, p + offset, (b - a) as usize * 2 + 2)?;
                }
                previous = Some(b);
            }
        }
        Ok(())
    }
    pub fn instance(
        &'a self,
        size: Size,
        hinting: Hinting,
        environment: Environment,
    ) -> Result<Instance<'a>> {
        Size::new(size.x, size.y)?;
        let vm = hint::Vm::new(self, size, hinting, environment)?;
        Ok(Instance {
            font: self,
            size,
            hinting,
            environment,
            vm,
        })
    }
    pub(super) fn metric(&self, glyph: u16) -> Result<(u16, i16)> {
        if glyph >= self.glyphs {
            return Err(invalid("glyph index out of range"));
        }
        let data = self.table(b"hmtx").unwrap();
        let index = glyph.min(self.metrics - 1) as usize;
        let bearing = if glyph < self.metrics {
            glyph as usize * 4 + 2
        } else {
            self.metrics as usize * 4 + (glyph - self.metrics) as usize * 2
        };
        Ok((u16_at(data, index * 4)?, i16_at(data, bearing)?))
    }
    pub fn glyph_program(&self, glyph: u16) -> Result<&'a [u8]> {
        Ok(self.load(glyph)?.instructions)
    }
    fn load(&self, glyph: u16) -> Result<RawGlyph<'a>> {
        if glyph >= self.glyphs {
            return Err(invalid("glyph index out of range"));
        }
        let loca = self.table(b"loca").unwrap();
        let offset = |i: usize| {
            if self.long_loca {
                u32_at(loca, i * 4).map(|n| n as usize)
            } else {
                u16_at(loca, i * 2).map(|n| n as usize * 2)
            }
        };
        let data =
            &self.table(b"glyf").unwrap()[offset(glyph as usize)?..offset(glyph as usize + 1)?];
        if data.is_empty() {
            return Ok(RawGlyph::default());
        }
        let count = i16_at(data, 0)?;
        let x_min = i16_at(data, 2)?;
        slice(data, 0, 10)?;
        if count == 0 && data.len() == 10 {
            return Ok(RawGlyph {
                x_min,
                ..RawGlyph::default()
            });
        }
        let mut cursor = 10;
        let mut result = RawGlyph {
            x_min,
            ..RawGlyph::default()
        };
        if count >= 0 {
            if count as usize > 4096 {
                return Err(invalid("glyph contour limit exceeded"));
            }
            let mut ends = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let end = u16_at(data, cursor)? as usize;
                cursor += 2;
                if ends.last().is_some_and(|v| end <= *v) {
                    return Err(invalid("invalid contour endpoints"));
                }
                ends.push(end);
            }
            let points = ends.last().map_or(0, |v| v + 1);
            if points > MAX_POINTS {
                return Err(invalid("glyph point limit exceeded"));
            }
            let length = u16_at(data, cursor)? as usize;
            cursor += 2;
            result.instructions = slice(data, cursor, length)?;
            cursor += length;
            let mut flags = Vec::with_capacity(points);
            while flags.len() < points {
                let flag = *slice(data, cursor, 1)?.first().unwrap();
                cursor += 1;
                let repeat = if flag & 8 != 0 {
                    let n = slice(data, cursor, 1)?[0] as usize;
                    cursor += 1;
                    n + 1
                } else {
                    1
                };
                if repeat > points - flags.len() {
                    return Err(invalid("glyph flag repeat exceeds point count"));
                }
                flags.extend(std::iter::repeat_n(flag, repeat));
            }
            let mut coordinates = vec![[0_i32; 2]; points];
            for (axis, (short_mask, same_mask)) in [(2, 16), (4, 32)].into_iter().enumerate() {
                let mut coordinate = 0_i32;
                for (i, &flag) in flags.iter().enumerate() {
                    let short = flag & short_mask != 0;
                    let same = flag & same_mask != 0;
                    let delta = if short {
                        let n = slice(data, cursor, 1)?[0] as i32;
                        cursor += 1;
                        if same {
                            n
                        } else {
                            -n
                        }
                    } else if same {
                        0
                    } else {
                        let n = i16_at(data, cursor)? as i32;
                        cursor += 2;
                        n
                    };
                    coordinate = coordinate
                        .checked_add(delta)
                        .ok_or_else(|| invalid("glyph coordinate overflow"))?;
                    if !(-65536..=65536).contains(&coordinate) {
                        return Err(invalid("glyph coordinate limit exceeded"));
                    }
                    coordinates[i][axis] = coordinate;
                }
            }
            result.points = coordinates
                .into_iter()
                .zip(flags)
                .map(|(p, f)| Point {
                    x: p[0],
                    y: p[1],
                    on_curve: f & 1 != 0,
                })
                .collect();
            result.ends = ends;
        } else if count == -1 {
            let final_flags = loop {
                if result.components.len() >= 128 {
                    return Err(invalid("composite component limit exceeded"));
                }
                let flags = u16_at(data, cursor)?;
                let glyph = u16_at(data, cursor + 2)?;
                cursor += 4;
                let xy = flags & 2 != 0;
                let (a, b) = if flags & 1 != 0 {
                    let (a, b) = (u16_at(data, cursor)?, u16_at(data, cursor + 2)?);
                    cursor += 4;
                    if xy {
                        (a as i16 as i32, b as i16 as i32)
                    } else {
                        (a as i32, b as i32)
                    }
                } else {
                    let a = slice(data, cursor, 2)?;
                    cursor += 2;
                    if xy {
                        (a[0] as i8 as i32, a[1] as i8 as i32)
                    } else {
                        (a[0] as i32, a[1] as i32)
                    }
                };
                let mut matrix = [16384, 0, 0, 16384];
                match flags & (8 | 64 | 128) {
                    0 => {}
                    8 => {
                        matrix[0] = i16_at(data, cursor)? as i32;
                        matrix[3] = matrix[0];
                        cursor += 2;
                    }
                    64 => {
                        matrix[0] = i16_at(data, cursor)? as i32;
                        matrix[3] = i16_at(data, cursor + 2)? as i32;
                        cursor += 4;
                    }
                    128 => {
                        for m in &mut matrix {
                            *m = i16_at(data, cursor)? as i32;
                            cursor += 2;
                        }
                    }
                    _ => return Err(invalid("conflicting composite transforms")),
                }
                result.components.push(Component {
                    flags,
                    glyph,
                    a,
                    b,
                    matrix,
                });
                if flags & 32 == 0 {
                    break flags;
                }
            };
            if final_flags & 256 != 0 {
                let length = u16_at(data, cursor)? as usize;
                cursor += 2;
                result.instructions = slice(data, cursor, length)?;
            }
        } else {
            return Err(invalid("unsupported glyph contour type"));
        }
        if result.instructions.len() > MAX_PROGRAM {
            return Err(invalid("glyph program limit exceeded"));
        }
        Ok(result)
    }
    pub(super) fn outline(
        &self,
        instance: &Instance<'_>,
        glyph: u16,
        ancestors: &mut Vec<u16>,
        budget: &mut Budget,
    ) -> Result<(Outline, i32)> {
        budget.nodes += 1;
        if budget.nodes > 128 {
            return Err(invalid("composite expansion limit exceeded"));
        }
        if ancestors.len() >= 16 || ancestors.contains(&glyph) {
            return Err(invalid("recursive or excessively deep composite"));
        }
        ancestors.push(glyph);
        let raw = self.load(glyph)?;
        let simple = raw.components.is_empty();
        budget.points += raw.points.len();
        if budget.points > 65_536 {
            return Err(invalid("composite expansion point budget exceeded"));
        }
        let scale =
            |v: i32, axis: u16| -> Result<i32> { instance.environment.scale(v, axis, self.units) };
        let (advance, bearing) = self.metric(glyph)?;
        let mut linear = scale(advance as i32, instance.size.x)?;
        let mut points = raw
            .points
            .iter()
            .map(|p| {
                Ok(Point {
                    x: scale(p.x, instance.size.x)?,
                    y: scale(p.y, instance.size.y)?,
                    on_curve: p.on_curve,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut ends = raw.ends;
        let mut selected_metrics = None;
        for component in raw.components {
            if component.matrix != [16384, 0, 0, 16384] {
                return Err(invalid("transformed components are not supported"));
            }
            let (child, origin) = self.outline(instance, component.glyph, ancestors, budget)?;
            if component.flags & 512 != 0 {
                selected_metrics = Some((origin, child.advance));
                linear = child.linear_advance;
            }
            let mut added = Vec::new();
            let mut added_ends = Vec::new();
            for contour in child.contours {
                for p in contour {
                    let m = component.matrix;
                    added.push(Point {
                        x: bounded(div_round(
                            p.x as i64 * m[0] as i64 + p.y as i64 * m[2] as i64,
                            16384,
                        ))?,
                        y: bounded(div_round(
                            p.x as i64 * m[1] as i64 + p.y as i64 * m[3] as i64,
                            16384,
                        ))?,
                        on_curve: p.on_curve,
                    });
                }
                added_ends.push(added.len() - 1);
            }
            let (mut dx, mut dy) = if component.flags & 2 != 0 {
                (
                    scale(component.a, instance.size.x)?,
                    scale(component.b, instance.size.y)?,
                )
            } else {
                let p = points
                    .get(component.a as usize)
                    .ok_or_else(|| invalid("invalid parent component point"))?;
                let q = added
                    .get(component.b as usize)
                    .ok_or_else(|| invalid("invalid child component point"))?;
                (p.x - q.x, p.y - q.y)
            };
            if component.flags & 2050 == 2050 {
                if component.flags & 4096 != 0 {
                    return Err(invalid("conflicting component offset flags"));
                }
                let m = component.matrix;
                (dx, dy) = (
                    div_round(dx as i64 * m[0] as i64 + dy as i64 * m[2] as i64, 16384) as i32,
                    div_round(dx as i64 * m[1] as i64 + dy as i64 * m[3] as i64, 16384) as i32,
                );
            }
            if component.flags & 6 == 6 && instance.hinting == Hinting::Native {
                dx = (dx + 32) & !63;
                dy = (dy + 32) & !63;
            }
            if points.len() + added.len() > MAX_POINTS {
                return Err(invalid("composite point limit exceeded"));
            }
            ends.extend(added_ends.into_iter().map(|n| n + points.len()));
            for p in added {
                points.push(Point {
                    x: bounded(p.x as i64 + dx as i64)?,
                    y: bounded(p.y as i64 + dy as i64)?,
                    ..p
                });
            }
        }
        let mut left = selected_metrics.map_or(
            scale(raw.x_min as i32 - bearing as i32, instance.size.x)?,
            |m| m.0,
        );
        let mut right = left + selected_metrics.map_or(linear, |m| m.1);
        if instance.hinting == Hinting::Native && selected_metrics.is_none() {
            left = (left + 32) & !63;
            right = (right + 32) & !63;
        }
        let count = points.len();
        points.extend([
            Point {
                x: left,
                ..Point::default()
            },
            Point {
                x: right,
                ..Point::default()
            },
            Point::default(),
            Point::default(),
        ]);
        let (points, scan_control, scan_type) = if instance.hinting == Hinting::Native {
            let unscaled = if simple {
                let mut p = raw.points;
                let left = raw.x_min as i32 - bearing as i32;
                p.extend([
                    Point {
                        x: left,
                        ..Point::default()
                    },
                    Point {
                        x: left + advance as i32,
                        ..Point::default()
                    },
                    Point::default(),
                    Point::default(),
                ]);
                Some(p)
            } else {
                None
            };
            instance
                .vm
                .glyph(points, &ends, raw.instructions, unscaled, &mut budget.work)?
        } else {
            (points, 0, 0)
        };
        let advance = points[count + 1].x - points[count].x;
        let origin = points[count].x;
        let mut contours = Vec::with_capacity(ends.len());
        let mut start = 0;
        for end in ends {
            contours.push(points[start..=end].to_vec());
            start = end + 1;
        }
        ancestors.pop();
        Ok((
            Outline {
                contours,
                advance,
                linear_advance: linear,
                scan_control,
                scan_type,
            },
            origin,
        ))
    }
}

pub(super) fn div_round(a: i64, b: i64) -> i64 {
    let sign = if (a < 0) != (b < 0) { -1 } else { 1 };
    sign * ((a.abs() + b.abs() / 2) / b.abs())
}
#[derive(Default)]
struct RawGlyph<'a> {
    points: Vec<Point>,
    ends: Vec<usize>,
    instructions: &'a [u8],
    components: Vec<Component>,
    x_min: i16,
}
struct Component {
    flags: u16,
    glyph: u16,
    a: i32,
    b: i32,
    matrix: [i32; 4],
}
