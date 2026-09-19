//! Curved-shape scan conversion measured from ZD621 previews.
//!
//! Zebra ^GB (Programming Guide pp. 210–211) specifies the rounding percentage,
//! not this rasterization. The recurrence is derived from the raw radius atlases
//! and checked against independent radii 49–257 in rounded-boxes-zd621-v1.
use crate::output::{Path, MAX_SEGMENTS};

fn corner(radius: usize) -> Vec<usize> {
    let r = radius as i64;
    let mut edges = vec![0; radius + 1];
    let mut point = |x: i64, y: i64| {
        let edge = (r - x).max(0) as usize;
        let row = (r - y) as usize;
        edges[row] = edges[row].max(edge);
    };
    let (mut x, mut y, mut error) = (0, r, 1 - r);
    while x < y {
        point(x, y);
        x += 1;
        if error < 0 {
            error += 2 * x + 1;
        } else {
            y -= 1;
            error += 2 * (x - y) + 1;
        }
    }
    // The shallow part of the captured curve differs from a conventional
    // midpoint circle. Keep its integer bias explicit rather than adjusting
    // particular radii or storing a table of captured outlines.
    error = x * x + y * y - r * r + x - 4 * y + 6;
    while y >= 0 {
        point(x, y);
        y -= 1;
        if error > 0 {
            error += 3 - 2 * y;
        } else {
            x += 1;
            error += 2 * (x - y) + 3;
        }
    }
    edges
}

pub(super) fn rounded_rect(
    path: &mut Path,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) -> Result<(), String> {
    if width <= 0. || height <= 0. {
        return Ok(());
    }
    if radius <= 0. {
        path.rect(x, y, width, height);
        return Ok(());
    }
    let rows = height.floor() as usize;
    let edges = corner(radius.floor() as usize);
    let mut start = 0;
    let mut previous = None;
    for row in 0..=rows {
        let edge = (row < rows).then(|| edges.get(row.min(rows - 1 - row)).copied().unwrap_or(0));
        if edge != previous {
            if let Some(left) = previous {
                if path.segments.len() + 5 > MAX_SEGMENTS {
                    return Err("geometry resource limit exceeded".into());
                }
                path.rect(
                    x + left as f64,
                    y + start as f64,
                    width - 2. * left as f64,
                    (row - start) as f64,
                );
            }
            previous = edge;
            start = row;
        }
    }
    Ok(())
}

// ^GC uses the same two-region curve as rounded boxes, but retains a half-dot
// radius for odd diameters when choosing steps. These decisions and the cap /
// side endpoint convention are measured in circles-zd621-v1, not specified by
// the Programming Guide (^GC, pp. 212–213).
fn circle_half_widths(diameter: usize) -> Vec<usize> {
    let r = (diameter / 2) as i64;
    let mut widths = vec![r as usize; r as usize + 1];
    let mut point = |x: i64, y: i64| {
        widths[y as usize] = widths[y as usize].min(x as usize);
    };
    // Twice the decision value keeps odd diameters exact without floats.
    let (mut x, mut y, mut error) = (0, r, 2 - diameter as i64);
    while x < y {
        point(x, y);
        x += 1;
        if error < 0 {
            error += 4 * x + 2;
        } else {
            y -= 1;
            error += 4 * (x - y) + 2;
        }
    }
    error = x * x + y * y - r * r + x - 4 * y + 6 - (diameter % 2) as i64;
    while y >= 0 {
        point(x, y);
        y -= 1;
        if error > 0 {
            error += 3 - 2 * y;
        } else {
            x += 1;
            error += 2 * (x - y) + 3;
        }
    }
    widths
}

pub(super) fn circle(path: &mut Path, diameter: f64, thickness: f64) -> Result<(), String> {
    let diameter = (diameter.floor() as usize).max(2);
    let thickness = (thickness.floor() as usize).max(2);
    let radius = diameter / 2;
    let outer = circle_half_widths(diameter);
    let inner = (radius > thickness).then(|| circle_half_widths(diameter - 2 * thickness));
    for row in 0..=2 * radius {
        let distance = row.abs_diff(radius);
        let half = outer[distance];
        if half == 0 {
            continue;
        }
        if path.segments.len() + 10 > MAX_SEGMENTS {
            return Err("geometry resource limit exceeded".into());
        }
        let left = radius - half;
        let right = radius + half + usize::from(distance + thickness < radius);
        let gap = inner
            .as_ref()
            .and_then(|widths| widths.get(distance + 1))
            .and_then(|half| half.checked_sub(1));
        if let Some(gap) = gap {
            path.rect(left as f64, row as f64, (radius - gap - left) as f64, 1.);
            path.rect(
                (radius + gap + 1) as f64,
                row as f64,
                (right - radius - gap - 1) as f64,
                1.,
            );
        } else {
            path.rect(left as f64, row as f64, (right - left) as f64, 1.);
        }
    }
    Ok(())
}

// Zebra ^GE (Programming Guide p. 214) specifies dimensions, not scan conversion.
// The preview uses the minor-axis radius for its initial decision, with truncated
// aspect-ratio products in the steep region. These rules were measured from
// ellipses-zd621-v1, including independent nearly circular and flat shapes.
fn ellipse_half_widths(width: usize, height: usize) -> Vec<usize> {
    let radius = height / 2;
    if radius == 0 {
        return vec![1];
    }
    let ratio = (width as f64 / height as f64).powi(2);
    let mut widths = vec![usize::MAX; radius + 1];
    let mut point = |x: usize, y: usize| widths[y] = widths[y].min(x);
    let (mut x, mut y) = (0, radius);
    let mut error = 1. - height as f64 / 2.;
    loop {
        point(x, y);
        let next_x = x + 1;
        let next_y = y - usize::from(error >= 0.);
        // The shallow region starts at the last steep-region point. Advancing
        // first loses a center pixel in flat ellipses such as 19 by 7 dots.
        if next_x as f64 >= ratio * next_y as f64 {
            break;
        }
        if error < 0. {
            error += (2 * next_x + 1) as f64;
        } else {
            error += (2 * next_x + 1) as f64 - 2. * (ratio * next_y as f64).floor();
        }
        x = next_x;
        y = next_y;
    }
    // Truncate the aspect-ratio product before updating the decision value;
    // dividing the recurrence by the ratio changes its rounding decisions.
    error = (error - x as f64 - y as f64 - 2. * (ratio * (y as f64 - 1.)).trunc() + 3.).floor();
    while y > 0 {
        point(x, y);
        y -= 1;
        if error > 0. {
            error += 1. - 2. * (ratio * (y as f64 - 1.)).trunc();
        } else {
            x += 1;
            error += (2 * x + 1) as f64 - 2. * (ratio * (y as f64 - 1.)).trunc();
        }
    }
    // Include the final step to the axis; a two-dot minor axis still has ink.
    point(x.max(1), 0);
    for half in &mut widths {
        if *half == usize::MAX {
            *half = 0;
        }
    }
    widths
}

pub(super) fn ellipse(
    path: &mut Path,
    width: f64,
    height: f64,
    thickness: f64,
) -> Result<(), String> {
    let (width, height) = (
        (width.floor() as usize).max(2),
        (height.floor() as usize).max(2),
    );
    if width == height {
        return circle(path, width as f64, thickness);
    }
    let transposed = width < height;
    let (width, height) = (width.max(height), width.min(height));
    let thickness = (thickness.floor() as usize).max(2);
    let (cx, cy) = (width / 2, height / 2);
    let outer = ellipse_half_widths(width, height);
    let inner = (height > 2 * thickness)
        .then(|| ellipse_half_widths(width - 2 * thickness, height - 2 * thickness));
    for row in 0..=2 * cy {
        let distance = row.abs_diff(cy);
        let half = outer[distance];
        if half == 0 {
            continue;
        }
        if path.segments.len() + 10 > MAX_SEGMENTS {
            return Err("geometry resource limit exceeded".into());
        }
        let left = cx.saturating_sub(half);
        let right = cx + half + usize::from(distance + thickness < cy);
        let gap = inner
            .as_ref()
            .and_then(|widths| widths.get(distance + 1))
            .and_then(|half| half.checked_sub(1));
        let mut span = |left: usize, right: usize| {
            if right > left {
                if transposed {
                    path.rect(row as f64, left as f64, 1., (right - left) as f64);
                } else {
                    path.rect(left as f64, row as f64, (right - left) as f64, 1.);
                }
            }
        };
        if let Some(gap) = gap {
            span(left, right.min(cx.saturating_sub(gap)));
            span(left.max(cx + gap + 1), right);
        } else {
            span(left, right);
        }
    }
    Ok(())
}
