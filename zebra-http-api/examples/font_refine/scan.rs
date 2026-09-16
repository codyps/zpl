//! Explicit research hypotheses, not a replacement for the production rasterizer.
use super::{campaign::Probe, sfnt};
use raster_diff::Raster;
#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub nonzero: bool,
    pub inclusive: bool,
    pub dropout: bool,
    pub fixed: bool,
    pub tie_shift: bool,
}
type P = [f64; 2];
fn curve(a: P, b: P, c: P, out: &mut Vec<P>, depth: u8) {
    let ab = [(a[0] + b[0]) / 2., (a[1] + b[1]) / 2.];
    let bc = [(b[0] + c[0]) / 2., (b[1] + c[1]) / 2.];
    let mid = [(ab[0] + bc[0]) / 2., (ab[1] + bc[1]) / 2.];
    if depth == 16
        || ((b[0] - (a[0] + c[0]) / 2.)
            .abs()
            .max((b[1] - (a[1] + c[1]) / 2.).abs())
            < 0.001)
    {
        out.push(c);
    } else {
        curve(a, ab, mid, out, depth + 1);
        curve(mid, bc, c, out, depth + 1);
    }
}
fn fill(r: &mut Raster, a: f64, b: f64, y: u32, rules: Rules) {
    let start = (a - 0.5 + if rules.tie_shift { 1e-7 } else { 0. }).ceil() as i32;
    let end = if rules.inclusive {
        (b - 0.5).floor() as i32 + 1
    } else {
        (b - 0.5 + if rules.tie_shift { 1e-7 } else { 0. }).ceil() as i32
    };
    for x in start.max(0)..end.min(r.width as i32) {
        r.pixels[(y * r.width + x as u32) as usize] = 0;
    }
    if rules.dropout && end <= start && b > a {
        let x = ((a + b) / 2.).floor() as i32;
        if x >= 0 && x < r.width as i32 {
            r.pixels[(y * r.width + x as u32) as usize] = 0;
        }
    }
}
pub fn render(p: &Probe, rules: Rules) -> Raster {
    let mut edges = vec![];
    let cs = sfnt::contours(p.text.as_bytes()[0]);
    let transform = |point: sfnt::Point| {
        let mut x = 24. + point.0 as f64 * if p.w == 0 { p.h } else { p.w } as f64 / 1024.;
        let mut y = 24. + p.h as f64 - point.1 as f64 * p.h as f64 / 1024.;
        if rules.fixed {
            x = (x * 64.).round() / 64.;
            y = (y * 64.).round() / 64.;
        }
        // Transform outlines before rasterization; normalization rotates pixel centers back.
        let side = p.cw as f64;
        match p.turns {
            0 => [x, y],
            1 => [side - y, x],
            2 => [side - x, side - y],
            3 => [y, side - x],
            _ => unreachable!(),
        }
    };
    for c in cs {
        let mut points = vec![transform(c[0])];
        let mut i = 1;
        while i < c.len() {
            if c[i].2 {
                points.push(transform(c[i]));
                i += 1;
            } else {
                let end = c[(i + 1) % c.len()];
                curve(
                    *points.last().unwrap(),
                    transform(c[i]),
                    transform(end),
                    &mut points,
                    0,
                );
                i += 2;
            }
        }
        points.push(points[0]);
        edges.extend(points.windows(2).map(|p| (p[0], p[1])));
    }
    let mut raster = Raster {
        width: p.cw,
        height: p.ch,
        pixels: vec![255; (p.cw * p.ch) as usize],
    };
    for y in 0..p.ch {
        let cy = y as f64 + 0.5 - if rules.tie_shift { 1e-7 } else { 0. };
        let mut crossings = vec![];
        for &(a, b) in &edges {
            if (a[1] <= cy && cy < b[1]) || (b[1] <= cy && cy < a[1]) {
                crossings.push((
                    a[0] + (cy - a[1]) * (b[0] - a[0]) / (b[1] - a[1]),
                    if b[1] > a[1] { 1 } else { -1 },
                ));
            }
            if rules.inclusive && (a[1] - cy).abs() < 1e-8 && (b[1] - cy).abs() < 1e-8 {
                fill(&mut raster, a[0].min(b[0]), a[0].max(b[0]), y, rules);
            }
        }
        crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (mut winding, mut start) = (0, 0.);
        for (x, d) in crossings {
            let inside = if rules.nonzero {
                winding != 0
            } else {
                winding % 2 != 0
            };
            winding += d;
            let next = if rules.nonzero {
                winding != 0
            } else {
                winding % 2 != 0
            };
            if !inside && next {
                start = x;
            }
            if inside && !next {
                fill(&mut raster, start, x, y, rules);
            }
        }
    }
    let mut q = p.clone();
    q.x = 0;
    q.y = 0;
    super::campaign::normalize(&raster, &q)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn winding_preserves_overlap_and_holes() {
        let p = super::super::campaign::plan()
            .into_iter()
            .find(|p| p.group == "calibration" && p.probes[0].h == 32)
            .unwrap();
        let base = Rules {
            nonzero: true,
            inclusive: false,
            dropout: false,
            fixed: false,
            tie_shift: false,
        };
        let c = &p.probes[2];
        let a = render(c, base);
        let b = render(
            c,
            Rules {
                nonzero: false,
                ..base
            },
        );
        assert!(
            a.pixels.iter().filter(|&&p| p == 0).count()
                > b.pixels.iter().filter(|&&p| p == 0).count()
        );
        let f = render(&p.probes[5], base);
        assert_eq!(f.pixels[(44 * f.width + 36) as usize], 255);
    }
    #[test]
    fn thin_bar_dropout_changes_empty_sample() {
        let p = super::super::campaign::plan()
            .into_iter()
            .find(|p| p.group == "calibration" && p.probes[0].h == 16)
            .unwrap();
        let base = Rules {
            nonzero: true,
            inclusive: false,
            dropout: false,
            fixed: false,
            tie_shift: false,
        };
        let a = render(&p.probes[7], base);
        let b = render(
            &p.probes[7],
            Rules {
                dropout: true,
                ..base
            },
        );
        assert!(a.pixels.iter().all(|&p| p == 255));
        assert!(b.pixels.contains(&0));
    }
}
