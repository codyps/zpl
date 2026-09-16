//! Feasible intervals for empirical grid-rounding laws, not recovered bytecode.
use serde_json::{json, Value};
/// q=max(minimum,floor(size*t+bias)). Upper endpoint is excluded.
pub fn interval(samples: &[(u32, u32)], bias: f64, minimum: u32) -> Option<(f64, f64)> {
    if samples.is_empty() || !bias.is_finite() {
        return None;
    }
    let (mut lo, mut hi) = (0f64, f64::INFINITY);
    for &(size, q) in samples {
        if size == 0 || q < minimum {
            return None;
        }
        if q > minimum {
            lo = lo.max((q as f64 - bias) / size as f64);
        }
        hi = hi.min((q as f64 + 1. - bias) / size as f64);
    }
    (lo < hi).then_some((lo, hi))
}
pub fn hypotheses(samples: &[(u32, u32)], minimum: u32) -> Value {
    let mut models = vec![];
    for (name, bias) in [("floor", 0.), ("nearest", 0.5)] {
        if let Some((lo, hi)) = interval(samples, bias, minimum) {
            models.push(json!({"rounding":name,"bias":bias,"dimension_interval":[lo,hi],"upper_exclusive":true,"representative_dimension":(lo+hi)/2.}));
        }
    }
    if !samples.is_empty() {
        let (mut lo, mut hi) = (0f64, f64::INFINITY);
        for &(size, q) in samples {
            if size == 0 || q < minimum {
                return json!({"error":"invalid observation"});
            }
            if q > minimum {
                lo = lo.max((q as f64 - 1.) / size as f64);
            }
            hi = hi.min(q as f64 / size as f64);
        }
        if lo < hi {
            models.push(json!({"rounding":"ceil","dimension_interval":[lo,hi],"lower_exclusive":true,"upper_exclusive":false,"representative_dimension":(lo+hi)/2.}));
        }
    }
    json!({"samples":samples,"minimum":minimum,"feasible_rules":models,"identified_uniquely":false})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovers_rounding_interval() {
        let s: Vec<_> = (12..128)
            .map(|h| (h, (h as f64 * 0.258 + 0.5).floor() as u32))
            .collect();
        let (lo, hi) = interval(&s, 0.5, 0).unwrap();
        assert!(lo <= 0.258 && 0.258 < hi);
        assert!(hi - lo < 0.001);
    }
    #[test]
    fn contradictory_observations_reject_rule() {
        assert!(interval(&[(32, 2), (64, 9)], 0.5, 0).is_none());
    }
    #[test]
    fn ceil_does_not_use_a_floating_epsilon() {
        let h = hypotheses(&[(16, 4), (64, 17)], 0);
        assert!(!h["feasible_rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["rounding"] == "ceil"));
    }
    #[test]
    fn clamped_one_pixel_width_has_no_lower_thickness_bound() {
        let (lo, hi) = interval(&[(8, 1), (16, 1)], 0.5, 1).unwrap();
        assert_eq!(lo, 0.);
        assert_eq!(hi, 1.5 / 16.);
    }
}
