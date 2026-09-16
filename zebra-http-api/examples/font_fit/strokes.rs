//! Axis-aligned stem constraints with learned, size-dependent quantization.
use super::{loss, Model, Sample, ANCHOR};
use raster_diff::Raster;
use std::collections::BTreeMap;
#[derive(Clone, Debug)]
pub struct Axis {
    /// Source-size ink intervals [low, high, supporting scan lines].
    pub bands: Vec<[f64; 3]>,
    /// Source width adjustment, rounding bias, inverse-size bias, position
    /// rounding bias, interpolation strength, constant pixel phase.
    pub params: [f64; 6],
}
#[derive(Clone, Debug)]
pub struct Strokes {
    pub axes: [Axis; 2],
}
/// Identify recurring thin ink runs in the source strike. Keep up to three
/// disjoint bands per axis, ranked by support, with deterministic tie breaking.
pub fn detect(r: &Raster) -> Strokes {
    let axes = std::array::from_fn(|axis| {
        let mut counts = BTreeMap::<(usize, usize), usize>::new();
        for scan in 0..192 {
            let mut start = None;
            for pos in 0..=192 {
                let black = pos < 192
                    && r.pixels[if axis == 0 {
                        scan * 192 + pos
                    } else {
                        pos * 192 + scan
                    }] == 0;
                match (start, black) {
                    (None, true) => start = Some(pos),
                    (Some(lo), false) => {
                        if (2..=24).contains(&(pos - lo)) {
                            *counts.entry((lo, pos)).or_default() += 1;
                        }
                        start = None;
                    }
                    _ => {}
                }
            }
        }
        let mut candidates: Vec<_> = counts.into_iter().filter(|(_, n)| *n >= 6).collect();
        candidates.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut bands = Vec::<[f64; 3]>::new();
        for ((lo, hi), n) in candidates {
            let (lo, hi) = (lo as f64 - ANCHOR[axis], hi as f64 - ANCHOR[axis]);
            if bands.iter().all(|b| hi < b[0] || lo > b[1]) {
                bands.push([lo, hi, n as f64]);
                if bands.len() == 3 {
                    break;
                }
            }
        }
        bands.sort_by(|a, b| a[0].total_cmp(&b[0]));
        Axis {
            bands,
            params: [0., 0.5, 0., 0.5, 0., 0.],
        }
    });
    Strokes { axes }
}
/// A monotone piecewise-linear warp holds both edges of a detected stem.
/// Width = max(1, floor((source_width + delta)*size/128 + bias + small/size)).
/// Position = floor(source_low*size/128 + position_bias) + phase.
pub fn warp(value: f64, size: u32, axis: &Axis) -> f64 {
    let [delta, bias, small, position, strength, phase] = axis.params;
    if axis.bands.is_empty() || strength == 0. {
        return value;
    }
    let scale = size as f64 / 128.;
    let mut knots = Vec::new();
    for &[lo, hi, _] in &axis.bands {
        let low = lo * scale;
        let high = hi * scale;
        let target_low = (low + position).floor() + phase;
        let width = ((hi - lo + delta) * scale + bias + small / size as f64)
            .floor()
            .max(1.);
        knots.push((low, low + strength * (target_low - low)));
        knots.push((high, high + strength * (target_low + width - high)));
    }
    // Preserve contour order even when separate stems meet at a small size.
    for i in 1..knots.len() {
        knots[i].1 = knots[i].1.max(knots[i - 1].1);
    }
    if value <= knots[0].0 {
        return value + knots[0].1 - knots[0].0;
    }
    for pair in knots.windows(2) {
        if value <= pair[1].0 {
            let t = (value - pair[0].0) / (pair[1].0 - pair[0].0);
            return pair[0].1 + t * (pair[1].1 - pair[0].1);
        }
    }
    let last = knots.last().unwrap();
    value + last.1 - last.0
}
/// No validation pixels enter this search. Zero strength keeps the old outline
/// as an explicit candidate, avoiding a forced regression on training loss.
pub fn fit(model: &mut Model, samples: &[Sample], source: &Raster) -> Vec<f64> {
    let mut candidate = model.clone();
    let fixed = fit_trial(model, samples, source, &[0.]);
    let expanded = fit_trial(&mut candidate, samples, source, &[0., -8., 8.]);
    if loss(&candidate, samples) < loss(model, samples) - 1e-12 {
        *model = candidate;
        expanded
    } else {
        fixed
    }
}
fn fit_trial(
    model: &mut Model,
    samples: &[Sample],
    source: &Raster,
    inverse_seeds: &[f64],
) -> Vec<f64> {
    model.strokes = Some(detect(source));
    let mut best = loss(model, samples);
    let mut history = vec![best];
    for axis in 0..2 {
        if model.strokes.as_ref().unwrap().axes[axis].bands.is_empty() {
            continue;
        }
        let original = model.strokes.as_ref().unwrap().axes[axis].params;
        let mut chosen = original;
        for strength in [0.5, 1.] {
            for delta in [0., -1., 1.] {
                for bias in [0.25, 0.5, 0.75] {
                    for position in [0., 0.5, 1.] {
                        for &small in inverse_seeds {
                            let p = [delta, bias, small, position, strength, 0.];
                            model.strokes.as_mut().unwrap().axes[axis].params = p;
                            let score = loss(model, samples);
                            if score < best - 1e-12 {
                                best = score;
                                chosen = p;
                            }
                        }
                    }
                }
            }
        }
        model.strokes.as_mut().unwrap().axes[axis].params = chosen;
        history.push(best);
    }
    let limits = [
        (-2., 2.),
        (0., 1.),
        (-16., 16.),
        (0., 1.),
        (0., 1.),
        (-0.5, 0.5),
    ];
    for factor in [1., 0.5, 0.25] {
        for _ in 0..2 {
            for axis in 0..2 {
                if model.strokes.as_ref().unwrap().axes[axis].bands.is_empty() {
                    continue;
                }
                for param in 0..6 {
                    let step = [0.5, 0.125, 4., 0.125, 0.25, 0.125][param] * factor;
                    let old = model.strokes.as_ref().unwrap().axes[axis].params[param];
                    let mut chosen = old;
                    for sign in [-1., 1.] {
                        let value = old + sign * step;
                        if value < limits[param].0 || value > limits[param].1 {
                            continue;
                        }
                        model.strokes.as_mut().unwrap().axes[axis].params[param] = value;
                        let score = loss(model, samples);
                        if score < best - 1e-12 {
                            best = score;
                            chosen = value;
                        }
                    }
                    model.strokes.as_mut().unwrap().axes[axis].params[param] = chosen;
                }
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
    fn rounding_threshold_depends_on_size_and_preserves_monotonicity() {
        let a = Axis {
            bands: vec![[0., 10., 20.], [20., 30., 20.]],
            params: [0., 0.5, 8., 0.5, 1., 0.],
        };
        assert_eq!(warp(2.5, 32, &a) - warp(0., 32, &a), 3.);
        assert_eq!(warp(1.25, 16, &a) - warp(0., 16, &a), 2.);
        for size in [12, 16, 31, 33, 128] {
            let values: Vec<_> = (-20..100).map(|x| warp(x as f64 / 4., size, &a)).collect();
            assert!(values.windows(2).all(|p| p[1] >= p[0]));
        }
    }
    #[test]
    fn zero_strength_is_identity() {
        let a = Axis {
            bands: vec![[0., 10., 20.]],
            params: [1., 0.75, 16., 0.25, 0., 0.5],
        };
        assert_eq!(warp(1.23, 31, &a), 1.23);
    }
    #[test]
    fn learns_stem_quantization_from_training_only() {
        let mut r = Raster {
            width: 192,
            height: 192,
            pixels: vec![255; 192 * 192],
        };
        for y in 60..140 {
            r.pixels[y * 192 + 32..y * 192 + 42].fill(0);
        }
        let mut truth = super::super::trace(&r, 0.);
        truth.strokes = Some(detect(&r));
        truth.strokes.as_mut().unwrap().axes[0].params = [0., 0.5, 8., 0.5, 1., 0.];
        let samples: Vec<_> = [12, 16, 24, 32, 48, 64, 96, 128]
            .into_iter()
            .map(|height| Sample {
                height,
                width: height,
                image: super::super::render(&truth, height, height),
            })
            .collect();
        let mut candidate = super::super::trace(&r, 0.);
        let before = loss(&candidate, &samples);
        let history = fit(&mut candidate, &samples, &r);
        assert!(loss(&candidate, &samples) < before);
        assert!(history.windows(2).all(|p| p[1] <= p[0]));
        assert_eq!(
            super::super::render(&candidate, 20, 20),
            super::super::render(&truth, 20, 20)
        );
    }
}
