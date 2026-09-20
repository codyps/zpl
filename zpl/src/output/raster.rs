//! Rasterize scenes into monochrome pixels.

use std::ops::Range;

use crate::output::{OutputError, Paint, Path, Point, Scene, Segment, MAX_SEGMENTS};
pub use raster_diff::Raster;

/// Destination for monochrome rasterization, independent of pixel storage.
///
/// Spans arrive in scene draw order, not necessarily row order. Implementations
/// must apply [`Paint::Invert`] to the pixels already present in the destination.
/// This is a compositing destination, not a stream of final scanlines.
pub trait RasterOutput {
    /// Prepare a white image of the given dimensions, discarding previous content.
    /// `rasterize_into` supplies nonzero, validated dimensions.
    fn reset(&mut self, width: u32, height: u32) -> Result<(), OutputError>;

    /// Apply `paint` to a nonempty, half-open horizontal span.
    /// `rasterize_into` guarantees `y < height` and `x.start < x.end <= width`
    /// for the dimensions passed to `reset`.
    fn paint_span(&mut self, y: u32, x: Range<u32>, paint: Paint) -> Result<(), OutputError>;
}

impl RasterOutput for Raster {
    fn reset(&mut self, width: u32, height: u32) -> Result<(), OutputError> {
        let len = (width as usize)
            .checked_mul(height as usize)
            .filter(|&len| width != 0 && height != 0 && len <= super::MAX_PIXELS)
            .ok_or(OutputError("invalid or excessive image dimensions"))?;
        self.pixels.resize(len, 255);
        self.pixels.fill(255);
        self.width = width;
        self.height = height;
        Ok(())
    }

    fn paint_span(&mut self, y: u32, x: Range<u32>, paint: Paint) -> Result<(), OutputError> {
        if y >= self.height || x.start >= x.end || x.end > self.width {
            return Err(OutputError("invalid raster span"));
        }
        let row = y as usize * self.width as usize;
        let pixels = self
            .pixels
            .get_mut(row + x.start as usize..row + x.end as usize)
            .ok_or(OutputError("invalid raster pixel buffer"))?;
        match paint {
            Paint::Black => pixels.fill(0),
            Paint::White => pixels.fill(255),
            Paint::Invert => pixels.iter_mut().for_each(|pixel| *pixel = 255 - *pixel),
        }
        Ok(())
    }
}

/// Rasterize filled paths into shared monochrome pixels.
pub fn rasterize(scene: &Scene) -> Result<Raster, OutputError> {
    let mut image = Raster {
        width: 0,
        height: 0,
        pixels: Vec::new(),
    };
    rasterize_into(scene, &mut image)?;
    Ok(image)
}

/// Rasterize filled paths into a caller-provided destination.
///
/// The scene is validated before the destination is touched, then the destination
/// is reset to white. Spans are clipped to the scene dimensions and emitted in
/// draw order using the even-odd fill rule. No intermediate pixel buffer is
/// allocated. Destination errors are propagated immediately; output may be
/// partially written if a destination, flattening, or scan-budget error occurs.
pub fn rasterize_into<O: RasterOutput + ?Sized>(
    scene: &Scene,
    output: &mut O,
) -> Result<(), OutputError> {
    scene.validate()?;
    output.reset(scene.width, scene.height)?;
    let mut work = 0u64;
    for draw in &scene.draws {
        let edges = flatten(&draw.path)?;
        if edges.is_empty() {
            continue;
        }
        // Only visit edges whose vertical extent intersects this scanline.
        // A long text field contains many short contours; scanning every edge
        // on every row falsely exhausts the work limit for ordinary labels.
        // Keep the original half-open crossing predicate below so shared
        // vertices, horizontal edges and even-odd holes retain their pixels.
        let row = |y: f64| y.max(0.0).min(scene.height as f64) as u32;
        let mut scheduled: Vec<_> = edges
            .iter()
            .enumerate()
            .filter_map(|(index, (a, b))| {
                if a.y == b.y {
                    return None;
                }
                let start = row(a.y.min(b.y).floor());
                let end = row(a.y.max(b.y).ceil());
                (start < end).then_some((start, end, index))
            })
            .collect();
        work = scheduled.iter().fold(work, |sum, &(start, end, _)| {
            sum.saturating_add(u64::from(end - start))
        });
        if work > 100_000_000 {
            return Err(OutputError("raster scan budget exceeded"));
        }
        scheduled.sort_unstable_by_key(|&(start, _, _)| start);
        let min = scheduled.first().map_or(0, |edge| edge.0);
        let max = scheduled.iter().map(|edge| edge.1).max().unwrap_or(0);
        let mut pending = scheduled.into_iter().peekable();
        let mut active = Vec::new();
        let mut intersections = Vec::new();
        for y in min..max {
            intersections.clear();
            let scan = y as f64 + 0.5;
            active.retain(|&(end, _)| end > y);
            while pending.peek().is_some_and(|edge| edge.0 <= y) {
                let (_, end, index) = pending.next().unwrap();
                active.push((end, index));
            }
            for &(_, index) in &active {
                let (a, b) = edges[index];
                if (a.y <= scan && b.y > scan) || (b.y <= scan && a.y > scan) {
                    intersections.push(a.x + (scan - a.y) * (b.x - a.x) / (b.y - a.y));
                }
            }
            intersections.sort_by(f64::total_cmp);
            for pair in intersections.as_chunks::<2>().0 {
                let start = (pair[0] - 0.5).ceil().max(0.0).min(scene.width as f64) as u32;
                let end = (pair[1] - 0.5).ceil().max(0.0).min(scene.width as f64) as u32;
                if start < end {
                    output.paint_span(y, start..end, draw.paint)?;
                }
            }
        }
    }
    Ok(())
}
fn midpoint(a: Point, b: Point) -> Point {
    Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}
fn flatten(path: &Path) -> Result<Vec<(Point, Point)>, OutputError> {
    fn curve(
        a: Point,
        b: Point,
        c: Point,
        d: Point,
        depth: u8,
        edges: &mut Vec<(Point, Point)>,
    ) -> Result<(), OutputError> {
        let deviation = (3.0 * b.x - 2.0 * a.x - d.x)
            .abs()
            .max((3.0 * b.y - 2.0 * a.y - d.y).abs())
            .max((3.0 * c.x - 2.0 * d.x - a.x).abs())
            .max((3.0 * c.y - 2.0 * d.y - a.y).abs());
        if depth == 12 || deviation < 0.3 {
            edges.push((a, d));
        } else {
            let ab = midpoint(a, b);
            let bc = midpoint(b, c);
            let cd = midpoint(c, d);
            let abc = midpoint(ab, bc);
            let bcd = midpoint(bc, cd);
            let m = midpoint(abc, bcd);
            curve(a, ab, abc, m, depth + 1, edges)?;
            curve(m, bcd, cd, d, depth + 1, edges)?;
        }
        if edges.len() > MAX_SEGMENTS {
            return Err(OutputError("curve flattening limit exceeded"));
        }
        Ok(())
    }
    let mut edges = Vec::new();
    let mut current = None;
    let mut start = None;
    for segment in &path.segments {
        match *segment {
            Segment::Move(p) => {
                if let (Some(a), Some(b)) = (current, start) {
                    edges.push((a, b));
                }
                current = Some(p);
                start = Some(p);
            }
            Segment::Line(p) => {
                let from = current.ok_or(OutputError("path must begin with Move"))?;
                edges.push((from, p));
                current = Some(p);
            }
            Segment::Cubic(b, c, d) => {
                let a = current.ok_or(OutputError("path must begin with Move"))?;
                curve(a, b, c, d, 0, &mut edges)?;
                current = Some(d);
            }
            Segment::Close => {
                if let (Some(a), Some(b)) = (current, start) {
                    edges.push((a, b));
                    current = Some(b);
                }
            }
        }
    }
    if let (Some(a), Some(b)) = (current, start) {
        edges.push((a, b));
    }
    if edges.len() > MAX_SEGMENTS {
        return Err(OutputError("path flattening limit exceeded"));
    }
    Ok(edges)
}
