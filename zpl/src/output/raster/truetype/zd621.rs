//! Native ZD621 V93 scan-conversion compatibility.
//!
//! Pixel-center winding and dropout terminology follow OpenType's scan
//! converter rules 1–4:
//! https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01#the-scan-converter
//! Directed ties and the minimum-height behavior are printer measurements.
//! See tests/fixtures/truetype-raster-zd621-v1/README.md for the fixed witnesses
//! and limits of this model; it is not the specification scan converter.
use super::{err, Edge};
use crate::truetype::Error;
use std::ops::Range;

fn monotonic_bounds(edges: &[Edge], contours: &[Range<usize>], axis: usize) -> Vec<(f64, f64)> {
    // Stub suppression concerns a complete monotonic chain, not each short
    // segment produced by quadratic subdivision. Visit each edge a bounded
    // number of times, and never join chains belonging to different contours.
    let mut bounds = vec![(0., 0.); edges.len()];
    for contour in contours {
        let indices: Vec<_> = contour
            .clone()
            .filter(|&i| edges[i].0[axis] != edges[i].1[axis])
            .collect();
        if indices.is_empty() {
            continue;
        }
        let direction = |i: usize| edges[i].1[axis] > edges[i].0[axis];
        let start = (0..indices.len())
            .find(|&i| {
                direction(indices[i]) != direction(indices[(i + indices.len() - 1) % indices.len()])
            })
            .unwrap_or(0);
        let mut position = 0;
        while position < indices.len() {
            let first = position;
            let sign = direction(indices[(start + position) % indices.len()]);
            let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
            while position < indices.len()
                && direction(indices[(start + position) % indices.len()]) == sign
            {
                let (a, b) = edges[indices[(start + position) % indices.len()]];
                low = low.min(a[axis]).min(b[axis]);
                high = high.max(a[axis]).max(b[axis]);
                position += 1;
            }
            for offset in first..position {
                bounds[indices[(start + offset) % indices.len()]] = (low, high);
            }
        }
    }
    bounds
}

struct Bitmap<'a> {
    rows: &'a mut [Vec<u8>],
    left: i32,
    top: i32,
    width: usize,
}
impl Bitmap<'_> {
    fn get(&self, x: i32, y: i32) -> bool {
        let (x, y) = (
            i64::from(x) - i64::from(self.left),
            i64::from(y) - i64::from(self.top),
        );
        x >= 0
            && y >= 0
            && x < self.width as i64
            && y < self.rows.len() as i64
            && self.rows[y as usize][x as usize / 8] & (128 >> (x as usize % 8)) != 0
    }
    fn set(&mut self, x: i32, y: i32) -> Result<(), Error> {
        let (x, y) = (
            i64::from(x) - i64::from(self.left),
            i64::from(y) - i64::from(self.top),
        );
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.rows.len() as i64 {
            return Err(err("glyph coverage escaped bounds"));
        }
        self.rows[y as usize][x as usize / 8] |= 128 >> (x as usize % 8);
        Ok(())
    }
}

pub(super) fn rasterize(
    edges: &[Edge],
    contours: &[Range<usize>],
    rows: &mut [Vec<u8>],
    left: i32,
    top: i32,
    width: usize,
) -> Result<(), Error> {
    let height = rows.len();
    if edges
        .len()
        .checked_mul(width + height)
        .is_none_or(|n| n > 16_777_216)
    {
        return Err(err("TrueType scan-conversion work budget exceeded"));
    }
    let bounds = [
        monotonic_bounds(edges, contours, 0),
        monotonic_bounds(edges, contours, 1),
    ];
    let mut owners = vec![0; edges.len()];
    for (owner, contour) in contours.iter().enumerate() {
        owners[contour.clone()].fill(owner);
    }
    let mut bitmap = Bitmap {
        rows,
        left,
        top,
        width,
    };
    let mut crossings = Vec::new();
    let mut dropouts = Vec::new();
    let low = edges
        .iter()
        .flat_map(|(a, b)| [a[1], b[1]])
        .fold(f64::INFINITY, f64::min);
    let high = edges
        .iter()
        .flat_map(|(a, b)| [a[1], b[1]])
        .fold(f64::NEG_INFINITY, f64::max);
    // Positive-height glyphs smaller than a center-sampled row still receive
    // one row. Apply this to the whole glyph, not each disconnected contour.
    let forced = low < high && (low - 0.5).ceil() == (high - 0.5).ceil();
    for axis in [1, 0] {
        let (origin, length) = if axis == 1 {
            (top, height)
        } else {
            (left, width)
        };
        for offset in 0..length {
            let row = origin + offset as i32;
            let scan = if axis == 1 && forced {
                if row != (high - 0.5).floor() as i32 {
                    continue;
                }
                (low + high) / 2.
            } else {
                f64::from(row) + 0.5
            };
            crossings.clear();
            // The two sweeps have opposite directed ties. Perturbing the scan
            // coordinate by epsilon also perturbs the interpolated crossing;
            // a steep edge can then lose an otherwise exact vertex pixel.
            for (index, &(a, b)) in edges.iter().enumerate() {
                if if axis == 1 {
                    (a[axis] <= scan && scan < b[axis]) || (b[axis] <= scan && scan < a[axis])
                } else {
                    (a[axis] < scan && scan <= b[axis]) || (b[axis] < scan && scan <= a[axis])
                } {
                    let cross = a[1 - axis]
                        + (scan - a[axis]) * (b[1 - axis] - a[1 - axis]) / (b[axis] - a[axis]);
                    crossings.push((cross, if b[axis] > a[axis] { 1 } else { -1 }, index));
                }
            }
            crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
            let (mut winding, mut start, mut start_index, mut index) = (0, 0., 0, 0);
            while index < crossings.len() {
                let first = index;
                let x = crossings[index].0;
                let previous = winding;
                while index < crossings.len() && crossings[index].0 == x {
                    winding += crossings[index].1;
                    index += 1;
                }
                if previous == 0 && winding != 0 {
                    start = x;
                    start_index = crossings[first].2;
                }
                let closed = previous != 0 && winding == 0;
                let collapsed = axis == 1
                    && previous == 0
                    && winding == 0
                    && index - first == 2
                    && owners[crossings[first].2] == owners[crossings[index - 1].2];
                if collapsed {
                    start = x;
                    start_index = crossings[first].2;
                }
                if !closed && !collapsed {
                    continue;
                }
                let a = if axis == 1 {
                    (start + 0.5).floor()
                } else {
                    (start - 0.5).ceil()
                } as i32;
                let b = if axis == 1 {
                    (x + 0.5).floor()
                } else {
                    (x - 0.5).ceil()
                } as i32;
                if axis == 1 {
                    for column in a..b {
                        bitmap.set(column, row)?;
                    }
                    if a >= b
                        && x >= start
                        && (forced
                            || [start_index, crossings[index - 1].2].iter().all(|&i| {
                                let (lo, hi) = bounds[axis][i];
                                lo <= scan - 1. && hi > scan + 1.
                            }))
                    {
                        dropouts.push((a, row, a - 1, row));
                    }
                } else if a >= b && x > start {
                    // Perpendicular dropout restores a connected thin stroke.
                    // A lone tip must not grow a new pixel; require continuation
                    // in the neighboring column and along both monotonic chains.
                    let column = b - 1;
                    if !bitmap.get(row, column)
                        && !bitmap.get(row, b)
                        && [-1, 0, 1]
                            .iter()
                            .any(|&dy| bitmap.get(row + 1, column + dy))
                        && [start_index, crossings[index - 1].2].iter().all(|&i| {
                            let (lo, hi) = bounds[axis][i];
                            lo <= scan - 1. && hi > scan + 1.
                        })
                    {
                        dropouts.push((row, column, row, b));
                    }
                }
            }
        }
        for (x, y, other_x, other_y) in dropouts.drain(..) {
            if !bitmap.get(x, y) && !bitmap.get(other_x, other_y) {
                bitmap.set(x, y)?;
            }
        }
    }
    Ok(())
}
