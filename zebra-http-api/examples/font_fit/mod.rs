//! Deterministic outline fitting; coordinates are dots at the 128-dot source size.
use raster_diff::Raster;
pub mod strokes;
use std::collections::BTreeMap;
#[derive(Clone, Debug)]
pub struct Model {
    pub contours: Vec<Vec<[f64; 2]>>,
    /// Constant pixel phase, followed by bounding-box grid-fit strengths (0..1).
    pub hints: [f64; 4],
    pub strokes: Option<strokes::Strokes>,
}
#[derive(Clone)]
pub struct Sample {
    pub height: u32,
    pub width: u32,
    pub image: Raster,
}
pub const CELL: usize = 192;
pub const ANCHOR: [f64; 2] = [32., 144.];
fn distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let t = (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1])
        / (d[0] * d[0] + d[1] * d[1]).max(1e-20))
    .clamp(0., 1.);
    (p[0] - a[0] - t * d[0]).hypot(p[1] - a[1] - t * d[1])
}
fn simplify(p: &[[f64; 2]], epsilon: f64) -> Vec<[f64; 2]> {
    if p.len() < 3 {
        return p.to_vec();
    }
    let (i, d) = p
        .iter()
        .enumerate()
        .skip(1)
        .take(p.len() - 2)
        .map(|(i, &x)| (i, distance(x, p[0], p[p.len() - 1])))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    if d <= epsilon {
        return vec![p[0], p[p.len() - 1]];
    }
    let mut a = simplify(&p[..=i], epsilon);
    a.pop();
    a.extend(simplify(&p[i..], epsilon));
    a
}
/// Trace oriented pixel edges, keeping holes as separate even-odd contours.
pub fn trace(r: &Raster, epsilon: f64) -> Model {
    type P = (i32, i32);
    let mut edges: BTreeMap<P, Vec<P>> = BTreeMap::new();
    let black = |x: i32, y: i32| {
        x >= 0
            && y >= 0
            && x < r.width as i32
            && y < r.height as i32
            && r.pixels[(y as u32 * r.width + x as u32) as usize] == 0
    };
    for y in 0..r.height as i32 {
        for x in 0..r.width as i32 {
            if black(x, y) {
                for (neighbor, a, b) in [
                    ((x, y - 1), (x, y), (x + 1, y)),
                    ((x + 1, y), (x + 1, y), (x + 1, y + 1)),
                    ((x, y + 1), (x + 1, y + 1), (x, y + 1)),
                    ((x - 1, y), (x, y + 1), (x, y)),
                ] {
                    if !black(neighbor.0, neighbor.1) {
                        edges.entry(a).or_default().push(b);
                    }
                }
            }
        }
    }
    let mut contours = Vec::new();
    while let Some(&start) = edges.keys().next() {
        let mut path = vec![start];
        let mut at = start;
        let mut previous = (start.0 - 1, start.1);
        loop {
            let options = edges.get_mut(&at).expect("closed pixel boundary");
            // At diagonal contacts, turn right to keep each component separate.
            let incoming = (at.0 - previous.0, at.1 - previous.1);
            let index = (0..options.len())
                .max_by_key(|&i| {
                    let d = (options[i].0 - at.0, options[i].1 - at.1);
                    let cross = incoming.0 * d.1 - incoming.1 * d.0;
                    let dot = incoming.0 * d.0 + incoming.1 * d.1;
                    if cross > 0 {
                        3
                    } else if dot > 0 {
                        2
                    } else if cross < 0 {
                        1
                    } else {
                        0
                    }
                })
                .unwrap();
            let next = options.remove(index);
            if options.is_empty() {
                edges.remove(&at);
            }
            previous = at;
            at = next;
            path.push(at);
            if at == start {
                break;
            }
        }
        let points: Vec<_> = path
            .iter()
            .map(|&(x, y)| [x as f64 - ANCHOR[0], y as f64 - ANCHOR[1]])
            .collect();
        let opposite = (1..points.len() - 1)
            .max_by(|&a, &b| {
                let d = |i: usize| (points[i][0] - points[0][0]).hypot(points[i][1] - points[0][1]);
                d(a).total_cmp(&d(b))
            })
            .unwrap();
        let mut p = simplify(&points[..=opposite], epsilon);
        p.pop();
        p.extend(simplify(&points[opposite..], epsilon));
        p.pop();
        if p.len() >= 3 {
            contours.push(p);
        }
    }
    Model {
        contours,
        hints: [0.; 4],
        strokes: None,
    }
}
pub fn transformed(model: &Model, h: u32, w: u32) -> Vec<Vec<[f64; 2]>> {
    let scale = [w as f64 / 128., h as f64 / 128.];
    let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
    for p in model.contours.iter().flatten() {
        for a in 0..2 {
            bounds[a][0] = bounds[a][0].min(p[a] * scale[a]);
            bounds[a][1] = bounds[a][1].max(p[a] * scale[a]);
        }
    }
    model
        .contours
        .iter()
        .map(|c| {
            c.iter()
                .map(|p| {
                    let mut q = [0.; 2];
                    for a in 0..2 {
                        let v = p[a] * scale[a];
                        let v = model.strokes.as_ref().map_or(v, |s| {
                            strokes::warp(v, if a == 0 { w } else { h }, &s.axes[a])
                        });
                        let [lo, hi] = bounds[a];
                        let t = (v - lo) / (hi - lo).max(1e-9);
                        let snap = (lo.round() - lo) * (1. - t) + (hi.round() - hi) * t;
                        q[a] = ANCHOR[a] + v + model.hints[a] + model.hints[a + 2] * snap;
                    }
                    q
                })
                .collect()
        })
        .collect()
}
/// Center-sampled even-odd scan conversion, matching the renderer's polygon rule.
pub fn render(model: &Model, h: u32, w: u32) -> Raster {
    let contours = transformed(model, h, w);
    let mut r = Raster {
        width: CELL as u32,
        height: CELL as u32,
        pixels: vec![255; CELL * CELL],
    };
    let mut intersections = Vec::new();
    for y in 0..CELL {
        intersections.clear();
        let scan = y as f64 + 0.5;
        for c in &contours {
            for i in 0..c.len() {
                let a = c[i];
                let b = c[(i + 1) % c.len()];
                if (a[1] <= scan && b[1] > scan) || (b[1] <= scan && a[1] > scan) {
                    intersections.push(a[0] + (scan - a[1]) * (b[0] - a[0]) / (b[1] - a[1]));
                }
            }
        }
        intersections.sort_by(f64::total_cmp);
        for pair in intersections.chunks_exact(2) {
            let start = (pair[0] - 0.5).ceil().clamp(0., CELL as f64) as usize;
            let end = (pair[1] - 0.5).ceil().clamp(0., CELL as f64) as usize;
            r.pixels[y * CELL + start..y * CELL + end].fill(0);
        }
    }
    r
}
pub fn loss(model: &Model, samples: &[Sample]) -> f64 {
    samples
        .iter()
        .map(|s| {
            let r = render(model, s.height, s.width);
            let mismatch = r
                .pixels
                .iter()
                .zip(&s.image.pixels)
                .filter(|(a, b)| a != b)
                .count();
            let ink = s.image.pixels.iter().filter(|&&p| p == 0).count().max(1);
            mismatch as f64 / ink as f64
        })
        .sum::<f64>()
        / samples.len() as f64
}
/// Bound every vertex to 1.5 source dots from its traced origin. Only training
/// samples enter this optimizer; finite steps minimize normalized binary XOR.
pub fn fit(model: &mut Model, samples: &[Sample], passes: usize) -> Vec<f64> {
    let origin = model.clone();
    let mut best = loss(model, samples);
    let mut history = vec![best];
    for step in [0.5, 0.25, 0.125] {
        for _ in 0..passes {
            for c in 0..model.contours.len() {
                for i in 0..model.contours[c].len() {
                    for axis in 0..2 {
                        let old = model.contours[c][i][axis];
                        let mut chosen = old;
                        for sign in [-1., 1.] {
                            let value = old + sign * step;
                            if (value - origin.contours[c][i][axis]).abs() > 1.5 {
                                continue;
                            }
                            model.contours[c][i][axis] = value;
                            let score = loss(model, samples);
                            if score < best - 1e-12 {
                                best = score;
                                chosen = value;
                            }
                        }
                        model.contours[c][i][axis] = chosen;
                    }
                }
            }
            history.push(best);
        }
    }
    history
}
pub fn fit_hints(model: &mut Model, samples: &[Sample]) -> Vec<f64> {
    let mut best = loss(model, samples);
    let mut history = vec![best];
    for step in [0.25, 0.125, 0.0625] {
        for _ in 0..2 {
            for a in 0..4 {
                let old = model.hints[a];
                let mut chosen = old;
                for sign in [-1., 1.] {
                    let value = old + step * sign;
                    let range = if a < 2 { -0.5..=0.5 } else { 0.0..=1.0 };
                    if !range.contains(&value) {
                        continue;
                    }
                    model.hints[a] = value;
                    let score = loss(model, samples);
                    if score < best - 1e-12 {
                        best = score;
                        chosen = value;
                    }
                }
                model.hints[a] = chosen;
            }
            history.push(best);
        }
    }
    history
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trace_preserves_holes_and_diagonal_components() {
        let mut r = Raster {
            width: 192,
            height: 192,
            pixels: vec![255; 192 * 192],
        };
        for y in 30..50 {
            for x in 20..45 {
                if !(25..40).contains(&x) || !(35..45).contains(&y) {
                    r.pixels[y * 192 + x] = 0;
                }
            }
        }
        r.pixels[50 * 192 + 45] = 0;
        let model = trace(&r, 0.);
        assert_eq!(render(&model, 128, 128), r);
    }
    #[test]
    fn fitted_polygon_rasterizer_matches_generic_path_adapter() {
        use zpl::output::{Draw, Paint, Path, Point, Scene, Segment};
        let model = Model {
            contours: vec![
                vec![[1.2, -44.], [26., -43.7], [33.5, -10.], [7.1, -3.]],
                vec![[11., -31.], [20., -30.], [21., -17.], [13., -15.]],
            ],
            hints: [0.125, -0.25, 0.5, 0.75],
            strokes: None,
        };
        let mut with_strokes = model.clone();
        with_strokes.strokes = Some(strokes::Strokes {
            axes: [
                strokes::Axis {
                    bands: vec![[1.2, 7.1, 8.], [26., 33.5, 8.]],
                    params: [-0.5, 0.625, 8., 0.5, 0.5, 0.],
                },
                strokes::Axis {
                    bands: vec![[-44., -31., 8.]],
                    params: [0., 0.25, -8., 0.5, 0.75, 0.125],
                },
            ],
        });
        for model in [model, with_strokes] {
            for (h, w) in [(20, 20), (31, 31), (32, 48), (64, 32), (128, 128)] {
                let mut path = Path::default();
                for c in transformed(&model, h, w) {
                    for (i, p) in c.iter().enumerate() {
                        path.segments.push(if i == 0 {
                            Segment::Move(Point::new(p[0], p[1]))
                        } else {
                            Segment::Line(Point::new(p[0], p[1]))
                        });
                    }
                    path.segments.push(Segment::Close);
                }
                let mut scene = Scene::new(192, 192, 203).unwrap();
                scene.draws.push(Draw {
                    path,
                    paint: Paint::Black,
                });
                assert_eq!(
                    render(&model, h, w),
                    zpl::output::raster::rasterize(&scene).unwrap()
                );
            }
        }
    }
    #[test]
    fn optimizer_recovers_a_shift_without_validation_samples() {
        let mut r = Raster {
            width: 192,
            height: 192,
            pixels: vec![255; 192 * 192],
        };
        for y in 90..120 {
            r.pixels[y * 192 + 35..y * 192 + 45].fill(0);
        }
        let truth = trace(&r, 0.);
        let mut m = truth.clone();
        for p in m.contours.iter_mut().flatten() {
            p[0] += 0.8;
        }
        let samples = vec![
            Sample {
                height: 64,
                width: 64,
                image: render(&truth, 64, 64),
            },
            Sample {
                height: 128,
                width: 128,
                image: r,
            },
        ];
        let before = loss(&m, &samples);
        let history = fit(&mut m, &samples, 1);
        assert!(loss(&m, &samples) < before);
        assert!(history.windows(2).all(|x| x[1] <= x[0]));
    }
}
