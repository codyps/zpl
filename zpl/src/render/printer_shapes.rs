//! Integer rounded-box scan conversion measured from ZD621 previews.
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
