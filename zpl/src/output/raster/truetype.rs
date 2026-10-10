//! Original monochrome quadratic-outline scan converter.
//!
//! Nonzero winding and center/dropout concepts follow OpenType's scan converter:
//! https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01#the-scan-converter
//! This is separate from the scene's even-odd shape rasterizer. Coverage becomes
//! disjoint pixel runs before entering a scene; scene fill semantics are unchanged.
use crate::{
    bitmap_font::Glyph,
    truetype::{Error, Outline, Point},
};

/// Explicit scan-conversion hypotheses. The printer mode is experimental and
/// its native-origin residuals are retained in the font comparison reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScanMode {
    /// Nonzero winding at pixel centers, left/top included, right/bottom excluded.
    #[default]
    Center,
    /// Nonzero winding, positive edge tie, horizontal dropout. This is the
    /// shared research hypothesis documented at
    /// https://github.com/codyps/zpl-font-extract/blob/main/docs/font-refinement-results.md.
    ZebraExperimental,
    /// ZD621 V93 curve-subdivision hypothesis, paired with its explicit font environment.
    Zd621V93,
}

mod zd621;

type P = [f64; 2];
type Edge = (P, P);
fn err(message: &str) -> Error {
    Error(message.into())
}
fn midpoint(a: P, b: P) -> P {
    [(a[0] + b[0]) / 2., (a[1] + b[1]) / 2.]
}
fn printer_midpoint(a: P, b: P, implied: bool) -> P {
    // Implied on-curve points truncate in 26.6; subdivision midpoints round
    // with positive ties. Both operations use the device's Y-up coordinates.
    // Independent SCFS and explicit-on-curve witnesses are documented in
    // tests/fixtures/truetype-raster-zd621-v1/README.md.
    let bias = if implied { 0. } else { 0.5 };
    [
        ((a[0] + b[0]) * 32. + bias).floor() / 64.,
        -((-a[1] - b[1]) * 32. + bias).floor() / 64.,
    ]
}

fn printer_quadratic(
    a: P,
    b: P,
    c: P,
    depth: Option<u8>,
    edges: &mut Vec<Edge>,
) -> Result<(), Error> {
    if edges.len() >= 65_536 {
        return Err(err("TrueType curve segment limit exceeded"));
    }
    let depth = depth.unwrap_or_else(|| {
        // The initial straight-line test uses the unrounded second derivative.
        // Subsequent depth selection uses a rounded midpoint and the norm
        // max(dx,dy) + min(dx,dy)/2. Uniform subdivision preserves the original
        // curve's depth even when one of its children becomes flat sooner.
        let dx = (a[0] - 2. * b[0] + c[0]).abs();
        let dy = (a[1] - 2. * b[1] + c[1]).abs();
        if dx.max(dy) + dx.min(dy) / 2. <= 1. {
            return 0;
        }
        let mid = printer_midpoint(a, c, false);
        let dx = (b[0] - mid[0]).abs();
        let dy = (b[1] - mid[1]).abs();
        let error = dx.max(dy) + dx.min(dy) / 2.;
        let mut depth = 0;
        let mut limit = 0.5;
        while error > limit && depth < 16 {
            depth += 1;
            limit *= 4.;
        }
        depth.max(1)
    });
    if depth == 0 {
        edges.push((a, c));
    } else {
        let ab = printer_midpoint(a, b, false);
        let bc = printer_midpoint(b, c, false);
        let mid = printer_midpoint(ab, bc, false);
        printer_quadratic(a, ab, mid, Some(depth - 1), edges)?;
        printer_quadratic(mid, bc, c, Some(depth - 1), edges)?;
    }
    Ok(())
}
fn quadratic(
    a: P,
    b: P,
    c: P,
    depth: u8,
    tolerance: f64,
    edges: &mut Vec<Edge>,
) -> Result<(), Error> {
    if edges.len() >= 65_536 {
        return Err(err("TrueType curve segment limit exceeded"));
    }
    // A device-space error bound shared by every size and glyph; no per-size
    // coefficients. Preserve the original curve's segment boundaries.
    let error = (b[0] - (a[0] + c[0]) / 2.)
        .abs()
        .max((b[1] - (a[1] + c[1]) / 2.).abs());
    if error <= tolerance || depth == 16 {
        edges.push((a, c));
    } else {
        let ab = midpoint(a, b);
        let bc = midpoint(b, c);
        let m = midpoint(ab, bc);
        quadratic(a, ab, m, depth + 1, tolerance, edges)?;
        quadratic(m, bc, c, depth + 1, tolerance, edges)?;
    }
    Ok(())
}
fn transform(p: Point, rotation: u8) -> P {
    let (x, y) = (p.x as f64 / 64., -p.y as f64 / 64.);
    match rotation {
        0 => [x, y],
        1 => [-y, x],
        2 => [-x, -y],
        _ => [y, -x],
    }
}
fn edges(
    outline: &Outline,
    rotation: u8,
    tolerance: f64,
    printer: bool,
) -> Result<(Vec<Edge>, Vec<std::ops::Range<usize>>), Error> {
    let mut edges = Vec::new();
    let mut contours = Vec::new();
    let mid = |a, b| {
        if printer {
            printer_midpoint(a, b, true)
        } else {
            midpoint(a, b)
        }
    };
    let curve = |a, b, c, edges: &mut Vec<Edge>| {
        if printer {
            printer_quadratic(a, b, c, None, edges)
        } else {
            quadratic(a, b, c, 0, tolerance, edges)
        }
    };
    if outline.contours.len() > 4096 {
        return Err(err("TrueType contour limit exceeded"));
    }
    let mut count = 0;
    for contour in &outline.contours {
        count += contour.len();
        if count > 16_384 {
            return Err(err("TrueType outline point limit exceeded"));
        }
        if contour.is_empty() {
            continue;
        }
        if contour
            .iter()
            .any(|p| p.x.unsigned_abs() > 1 << 28 || p.y.unsigned_abs() > 1 << 28)
        {
            return Err(err("TrueType coordinate limit exceeded"));
        }
        let contour_start = edges.len();
        let first = contour[0];
        let last = contour[contour.len() - 1];
        let (start, begin, end) = if first.on_curve {
            (transform(first, rotation), 1, contour.len())
        } else if last.on_curve {
            (transform(last, rotation), 0, contour.len() - 1)
        } else {
            (
                mid(transform(first, rotation), transform(last, rotation)),
                0,
                contour.len(),
            )
        };
        let mut current = start;
        let mut control = None;
        for p in &contour[begin..end] {
            let next = transform(*p, rotation);
            if p.on_curve {
                if let Some(b) = control.take() {
                    curve(current, b, next, &mut edges)?;
                } else {
                    edges.push((current, next));
                }
                current = next;
            } else {
                if let Some(b) = control.replace(next) {
                    let point = mid(b, next);
                    curve(current, b, point, &mut edges)?;
                    current = point;
                }
            }
        }
        if let Some(b) = control {
            curve(current, b, start, &mut edges)?;
        } else {
            edges.push((current, start));
        }
        contours.push(contour_start..edges.len());
        if edges.len() > 65_536 {
            return Err(err("TrueType segment limit exceeded"));
        }
    }
    Ok((edges, contours))
}

/// Rasterize after the requested quarter-turn rotation, in baseline coordinates.
/// No clipping, rescaling, inferred alignment, or printer requests are performed.
/// `advance` is supplied separately because ZPL metrics can differ from phantom points.
pub fn rasterize(
    outline: &Outline,
    codepoint: u32,
    advance: u32,
    quarter_turns: u8,
    mode: ScanMode,
) -> Result<Glyph, Error> {
    if quarter_turns > 3 || advance > 4096 {
        return Err(err("invalid glyph rotation or advance"));
    }
    let (edges, contours) = edges(
        outline,
        quarter_turns,
        1. / 1024.,
        mode == ScanMode::Zd621V93,
    )?;
    let mut result = Glyph {
        codepoint,
        advance,
        left: 0,
        top: 0,
        width: 0,
        height: 0,
        bitmap: Vec::new(),
    };
    if edges.is_empty() {
        return Ok(result);
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for &(a, b) in &edges {
        for p in [a, b] {
            min_x = min_x.min(p[0]);
            min_y = min_y.min(p[1]);
            max_x = max_x.max(p[0]);
            max_y = max_y.max(p[1]);
        }
    }
    let left = min_x.floor() as i32 - 1;
    let top = min_y.floor() as i32 - 1;
    let width = (max_x.ceil() as i64 - left as i64 + 1) as usize;
    let height = (max_y.ceil() as i64 - top as i64 + 1) as usize;
    if width > 4096 || height > 4096 || width.checked_mul(height).is_none_or(|n| n > 4_194_304) {
        return Err(err("TrueType glyph bitmap budget exceeded"));
    }
    if edges
        .len()
        .checked_mul(height)
        .is_none_or(|n| n > 16_777_216)
    {
        return Err(err("TrueType scan-conversion work budget exceeded"));
    }
    let mut bitmap = vec![vec![0_u8; width.div_ceil(8)]; height];
    if mode == ScanMode::Zd621V93 {
        zd621::rasterize(&edges, &contours, &mut bitmap, left, top, width)?;
    } else {
        let epsilon = if mode != ScanMode::Center { 1e-7 } else { 0. };
        let mut crossings = Vec::new();
        for (row, bits) in bitmap.iter_mut().enumerate() {
            let scan = top as f64 + row as f64 + 0.5 - epsilon;
            crossings.clear();
            for &(a, b) in &edges {
                if (a[1] <= scan && scan < b[1]) || (b[1] <= scan && scan < a[1]) {
                    crossings.push((
                        a[0] + (scan - a[1]) * (b[0] - a[0]) / (b[1] - a[1]),
                        if b[1] > a[1] { 1_i32 } else { -1 },
                    ));
                }
            }
            crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut winding = 0;
            let mut start = 0.;
            let mut i = 0;
            while i < crossings.len() {
                let x = crossings[i].0;
                let previous = winding;
                // Shared contour edges must cancel before emitting spans/dropouts.
                while i < crossings.len() && crossings[i].0 == x {
                    winding += crossings[i].1;
                    i += 1;
                }
                if previous == 0 && winding != 0 {
                    start = x;
                }
                if previous != 0 && winding == 0 {
                    let a = (start - 0.5 + epsilon).ceil() as i32;
                    let b = (x - 0.5 + epsilon).ceil() as i32;
                    for col in a..b {
                        let col = (col - left) as usize;
                        if col >= width {
                            return Err(err("glyph coverage escaped bounds"));
                        }
                        bits[col / 8] |= 128 >> (col % 8);
                    }
                    if mode != ScanMode::Center && a >= b && x > start {
                        let col = (((start + x) / 2.).floor() as i32 - left) as usize;
                        if col >= width {
                            return Err(err("glyph dropout escaped bounds"));
                        }
                        bits[col / 8] |= 128 >> (col % 8);
                    }
                }
            }
        }
    }
    // Bounds are metadata over the generated coverage, not an alignment of a
    // reference image. Preserve absolute left/top when discarding empty borders.
    let (mut x0, mut y0, mut x1, mut y1) = (width, height, 0, 0);
    for (y, row) in bitmap.iter().enumerate() {
        for x in 0..width {
            if row[x / 8] & (128 >> (x % 8)) != 0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x1 == 0 {
        return Ok(result);
    }
    let w = x1 - x0;
    result.left = left + x0 as i32;
    result.top = top + y0 as i32;
    result.width = w as u32;
    result.height = (y1 - y0) as u32;
    result.bitmap = bitmap[y0..y1]
        .iter()
        .map(|row| {
            let mut bits = vec![0; w.div_ceil(8)];
            for x in 0..w {
                if row[(x + x0) / 8] & (128 >> ((x + x0) % 8)) != 0 {
                    bits[x / 8] |= 128 >> (x % 8);
                }
            }
            bits
        })
        .collect();
    Ok(result)
}

/// Keep layout in upright baseline coordinates after device-space scan conversion.
/// The scene will apply the field rotation once. Rotating before scanning matters
/// for directed edge ties and horizontal dropout (OpenType scan converter, above).
pub(crate) fn unrotate_glyph(mut glyph: Glyph, quarter_turns: u8) -> Glyph {
    if quarter_turns == 0 || glyph.width == 0 || glyph.height == 0 {
        return glyph;
    }
    let (w, h) = (glyph.width, glyph.height);
    let (left, top, width, height) = match quarter_turns {
        1 => (glyph.top, -glyph.left - w as i32, h, w),
        2 => (-glyph.left - w as i32, -glyph.top - h as i32, w, h),
        3 => (-glyph.top - h as i32, glyph.left, h, w),
        _ => unreachable!("validated quarter-turn rotation"),
    };
    let mut bitmap = vec![vec![0; width.div_ceil(8) as usize]; height as usize];
    for y in 0..h {
        for x in 0..w {
            if glyph.bitmap[y as usize][x as usize / 8] & (128 >> (x % 8)) != 0 {
                let (x, y) = match quarter_turns {
                    1 => (y, w - 1 - x),
                    2 => (w - 1 - x, h - 1 - y),
                    3 => (h - 1 - y, x),
                    _ => unreachable!(),
                };
                bitmap[y as usize][x as usize / 8] |= 128 >> (x % 8);
            }
        }
    }
    glyph.left = left;
    glyph.top = top;
    glyph.width = width;
    glyph.height = height;
    glyph.bitmap = bitmap;
    glyph
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonzero_overlap_and_baseline_rotations_preserve_coverage() {
        // OpenType TrueType Fundamentals, "The scan converter": same-winding
        // contours union their overlap, unlike the scene's even-odd rule.
        let rect = |x| {
            vec![
                Point {
                    x,
                    y: 0,
                    on_curve: true,
                },
                Point {
                    x,
                    y: 128,
                    on_curve: true,
                },
                Point {
                    x: x + 128,
                    y: 128,
                    on_curve: true,
                },
                Point {
                    x: x + 128,
                    y: 0,
                    on_curve: true,
                },
            ]
        };
        let mut outline = Outline {
            contours: vec![rect(0), rect(64)],
            advance: 192,
            linear_advance: 192,
            scan_control: 0,
            scan_type: 0,
        };
        for (turn, bounds) in [
            (0, (0, -2, 3, 2)),
            (1, (0, 0, 2, 3)),
            (2, (-3, 0, 3, 2)),
            (3, (-2, -3, 2, 3)),
        ] {
            let glyph = rasterize(&outline, 65, 3, turn, ScanMode::Center).unwrap();
            assert_eq!((glyph.left, glyph.top, glyph.width, glyph.height), bounds);
            assert_eq!(
                glyph
                    .bitmap
                    .iter()
                    .flatten()
                    .map(|b| b.count_ones())
                    .sum::<u32>(),
                6
            );
        }
        outline.contours[1].reverse();
        let glyph = rasterize(&outline, 65, 3, 0, ScanMode::Center).unwrap();
        assert_eq!(
            glyph
                .bitmap
                .iter()
                .flatten()
                .map(|b| b.count_ones())
                .sum::<u32>(),
            4
        );
    }

    #[test]
    fn caller_constructed_outlines_obey_numeric_and_bitmap_budgets() {
        let mut outline = Outline {
            contours: vec![vec![Point {
                x: i32::MIN,
                y: 0,
                on_curve: true,
            }]],
            advance: 0,
            linear_advance: 0,
            scan_control: 0,
            scan_type: 0,
        };
        assert!(rasterize(&outline, 65, 0, 0, ScanMode::Center)
            .unwrap_err()
            .0
            .contains("coordinate"));
        outline.contours = vec![vec![
            Point {
                x: 0,
                y: 0,
                on_curve: true,
            },
            Point {
                x: 1_000_000,
                y: 1_000_000,
                on_curve: true,
            },
            Point {
                x: 0,
                y: 1_000_000,
                on_curve: true,
            },
        ]];
        assert!(rasterize(&outline, 65, 0, 0, ScanMode::Center)
            .unwrap_err()
            .0
            .contains("bitmap budget"));
    }
}
