//! Rasterize scenes into monochrome pixels.

use crate::output::{OutputError, Paint, Path, Point, Scene, Segment, MAX_SEGMENTS};
pub use raster_diff::Raster;

/// Rasterize filled paths into shared monochrome pixels.
pub fn rasterize(scene: &Scene) -> Result<Raster, OutputError> {
    scene.validate()?;
    let mut image = Raster {
        width: scene.width,
        height: scene.height,
        pixels: vec![255; scene.width as usize * scene.height as usize],
    };
    let mut work = 0u64;
    for draw in &scene.draws {
        let edges = flatten(&draw.path)?;
        if edges.is_empty() {
            continue;
        }
        let min = edges
            .iter()
            .map(|(a, b)| a.y.min(b.y))
            .fold(f64::INFINITY, f64::min)
            .floor()
            .max(0.0)
            .min(scene.height as f64) as u32;
        let max = edges
            .iter()
            .map(|(a, b)| a.y.max(b.y))
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil()
            .max(0.0)
            .min(scene.height as f64) as u32;
        work = work.saturating_add(u64::from(max - min) * edges.len() as u64);
        if work > 100_000_000 {
            return Err(OutputError("raster scan budget exceeded"));
        }
        let mut intersections = Vec::new();
        for y in min..max {
            intersections.clear();
            let scan = y as f64 + 0.5;
            for &(a, b) in &edges {
                if (a.y <= scan && b.y > scan) || (b.y <= scan && a.y > scan) {
                    intersections.push(a.x + (scan - a.y) * (b.x - a.x) / (b.y - a.y));
                }
            }
            intersections.sort_by(f64::total_cmp);
            for pair in intersections.chunks_exact(2) {
                let start = (pair[0] - 0.5).ceil().max(0.0).min(scene.width as f64) as usize;
                let end = (pair[1] - 0.5).ceil().max(0.0).min(scene.width as f64) as usize;
                for x in start..end {
                    let pixel = &mut image.pixels[y as usize * scene.width as usize + x];
                    *pixel = match draw.paint {
                        Paint::Black => 0,
                        Paint::White => 255,
                        Paint::Invert => 255 - *pixel,
                    };
                }
            }
        }
    }
    Ok(image)
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
