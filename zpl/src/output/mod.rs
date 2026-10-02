//! Output-independent filled paths and adapters. Coordinates are printer dots.
use std::{error::Error, fmt};

mod limits;
mod pdf;
pub use limits::Limits;
mod png;
pub mod raster;
mod svg;
pub use pdf::Pdf;
pub use png::Png;
pub use raster_diff::MAX_PIXELS;
pub use svg::Svg;

pub const MAX_SEGMENTS: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    Move(Point),
    Line(Point),
    Cubic(Point, Point, Point),
    Close,
}

/// All subpaths use the even-odd fill rule. Shapes, glyphs, and pixels can share
/// this representation; adapters do not need to understand ZPL or fonts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Path {
    pub segments: Vec<Segment>,
}
impl Path {
    /// Validate newly constructed geometry without rescanning earlier draws.
    pub(crate) fn validate(&self, coordinate_abs: f64) -> Result<(), OutputError> {
        let point = |p: &Point| {
            p.x.is_finite()
                && p.y.is_finite()
                && p.x.abs() <= coordinate_abs
                && p.y.abs() <= coordinate_abs
        };
        let mut started = false;
        for s in &self.segments {
            match s {
                Segment::Move(_) => started = true,
                Segment::Line(_) | Segment::Cubic(..) if !started => {
                    return Err(OutputError("path must begin with Move"))
                }
                _ => {}
            }
            let valid = match s {
                Segment::Move(p) | Segment::Line(p) => point(p),
                Segment::Cubic(a, b, c) => point(a) && point(b) && point(c),
                Segment::Close => true,
            };
            if !valid {
                return Err(OutputError("invalid path coordinate"));
            }
        }
        Ok(())
    }

    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        self.segments.extend([
            Segment::Move(Point::new(x, y)),
            Segment::Line(Point::new(x + w, y)),
            Segment::Line(Point::new(x + w, y + h)),
            Segment::Line(Point::new(x, y + h)),
            Segment::Close,
        ]);
    }
    pub fn ellipse(&mut self, x: f64, y: f64, w: f64, h: f64) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let (cx, cy, rx, ry) = (x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0);
        let k = 0.5522847498307936;
        self.segments.extend([
            Segment::Move(Point::new(cx + rx, cy)),
            Segment::Cubic(
                Point::new(cx + rx, cy + ry * k),
                Point::new(cx + rx * k, cy + ry),
                Point::new(cx, cy + ry),
            ),
            Segment::Cubic(
                Point::new(cx - rx * k, cy + ry),
                Point::new(cx - rx, cy + ry * k),
                Point::new(cx - rx, cy),
            ),
            Segment::Cubic(
                Point::new(cx - rx, cy - ry * k),
                Point::new(cx - rx * k, cy - ry),
                Point::new(cx, cy - ry),
            ),
            Segment::Cubic(
                Point::new(cx + rx * k, cy - ry),
                Point::new(cx + rx, cy - ry * k),
                Point::new(cx + rx, cy),
            ),
            Segment::Close,
        ]);
    }
    pub fn rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, r: f64) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let r = r.max(0.0).min(w / 2.0).min(h / 2.0);
        if r == 0.0 {
            self.rect(x, y, w, h);
            return;
        }
        let k = r * 0.5522847498307936;
        self.segments.extend([
            Segment::Move(Point::new(x + r, y)),
            Segment::Line(Point::new(x + w - r, y)),
            Segment::Cubic(
                Point::new(x + w - r + k, y),
                Point::new(x + w, y + r - k),
                Point::new(x + w, y + r),
            ),
            Segment::Line(Point::new(x + w, y + h - r)),
            Segment::Cubic(
                Point::new(x + w, y + h - r + k),
                Point::new(x + w - r + k, y + h),
                Point::new(x + w - r, y + h),
            ),
            Segment::Line(Point::new(x + r, y + h)),
            Segment::Cubic(
                Point::new(x + r - k, y + h),
                Point::new(x, y + h - r + k),
                Point::new(x, y + h - r),
            ),
            Segment::Line(Point::new(x, y + r)),
            Segment::Cubic(
                Point::new(x, y + r - k),
                Point::new(x + r - k, y),
                Point::new(x + r, y),
            ),
            Segment::Close,
        ]);
    }
    pub fn transform(&mut self, mut f: impl FnMut(Point) -> Point) {
        for s in &mut self.segments {
            match s {
                Segment::Move(p) | Segment::Line(p) => *p = f(*p),
                Segment::Cubic(a, b, c) => {
                    *a = f(*a);
                    *b = f(*b);
                    *c = f(*c);
                }
                Segment::Close => {}
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paint {
    Black,
    White,
    Invert,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Draw {
    pub path: Path,
    pub paint: Paint,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    pub draws: Vec<Draw>,
}
impl Scene {
    pub fn new(width: u32, height: u32, dpi: u32) -> Result<Self, OutputError> {
        Self::new_with_limits(width, height, dpi, Limits::default())
    }
    /// Construct a scene with caller-selected resource ceilings.
    pub fn new_with_limits(
        width: u32,
        height: u32,
        dpi: u32,
        limits: Limits,
    ) -> Result<Self, OutputError> {
        let s = Self {
            width,
            height,
            dpi,
            draws: Vec::new(),
        };
        s.validate_with_limits(limits)?;
        Ok(s)
    }
    pub fn validate(&self) -> Result<(), OutputError> {
        self.validate_with_limits(Limits::default())
    }
    pub fn validate_with_limits(&self, limits: Limits) -> Result<(), OutputError> {
        let pixels = (self.width as usize)
            .checked_mul(self.height as usize)
            .ok_or(OutputError("image dimensions overflow"))?;
        if self.width == 0 || self.height == 0 || self.dpi == 0 || pixels > limits.pixels {
            return Err(OutputError("invalid or excessive image dimensions"));
        }
        let mut count = 0usize;
        for draw in &self.draws {
            count = count
                .checked_add(draw.path.segments.len())
                .ok_or(OutputError("too many path segments"))?;
            if count > limits.segments {
                return Err(OutputError("too many path segments"));
            }
            draw.path.validate(limits.coordinate_abs)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputError(pub &'static str);
impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl Error for OutputError {}

pub trait Adapter {
    fn encode(&self, scene: &Scene) -> Result<Vec<u8>, OutputError>;
}
