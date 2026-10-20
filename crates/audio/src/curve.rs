use serde::{Deserialize, Serialize};
use videocut_model::Interp;

use crate::units::clamp_gain_db;

/// One gain keyframe. `time_ticks` is clip-local, zero at `timeline_in`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GainKeyframe {
    pub time_ticks: i64,
    pub gain_db: f64,
    pub interp: Interp,
}

pub fn eval_gain_db(base_db: f64, keyframes: &[GainKeyframe], time_ticks: i64) -> f64 {
    let sorted = sorted_gain_keys(keyframes);
    eval_sorted_gain_db(base_db, &sorted, time_ticks)
}

pub(crate) fn sorted_gain_keys(keyframes: &[GainKeyframe]) -> Vec<GainKeyframe> {
    let mut points = keyframes.to_vec();
    points.sort_by(|left, right| left.time_ticks.cmp(&right.time_ticks));
    let mut deduped = Vec::with_capacity(points.len());
    for mut point in points {
        point.gain_db = clamp_gain_db(point.gain_db);
        if deduped
            .last()
            .is_some_and(|previous: &GainKeyframe| previous.time_ticks == point.time_ticks)
        {
            deduped.pop();
        }
        deduped.push(point);
    }
    deduped
}

pub(crate) fn eval_sorted_gain_db(
    base_db: f64,
    keyframes: &[GainKeyframe],
    time_ticks: i64,
) -> f64 {
    let Some(first) = keyframes.first() else {
        return clamp_gain_db(base_db);
    };
    if time_ticks <= first.time_ticks {
        return first.gain_db;
    }
    let last = keyframes[keyframes.len() - 1];
    if time_ticks >= last.time_ticks {
        return last.gain_db;
    }
    for pair in keyframes.windows(2) {
        let left = pair[0];
        let right = pair[1];
        if time_ticks >= left.time_ticks && time_ticks < right.time_ticks {
            return interpolate(left, right, time_ticks);
        }
    }
    last.gain_db
}

fn interpolate(left: GainKeyframe, right: GainKeyframe, time_ticks: i64) -> f64 {
    let span = (right.time_ticks - left.time_ticks) as f64;
    if span <= 0.0 {
        return right.gain_db;
    }
    let unit = (time_ticks - left.time_ticks) as f64 / span;
    match left.interp {
        Interp::Hold => left.gain_db,
        Interp::Linear => left.gain_db + (right.gain_db - left.gain_db) * unit,
        Interp::Bezier => {
            let eased = unit * unit * (3.0 - 2.0 * unit);
            left.gain_db + (right.gain_db - left.gain_db) * eased
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(time_ticks: i64, gain_db: f64, interp: Interp) -> GainKeyframe {
        GainKeyframe {
            time_ticks,
            gain_db,
            interp,
        }
    }

    #[test]
    fn static_gain_is_the_base_value() {
        assert_eq!(eval_gain_db(-3.0, &[], 50), -3.0);
    }

    #[test]
    fn linear_hold_and_bezier_keyframes() {
        let linear = [key(0, 0.0, Interp::Linear), key(1000, 8.0, Interp::Hold)];
        assert!((eval_gain_db(5.0, &linear, 500) - 4.0).abs() < 1e-9);
        assert_eq!(eval_gain_db(5.0, &linear, -10), 0.0);
        assert_eq!(eval_gain_db(5.0, &linear, 1000), 8.0);

        let hold = [key(0, 0.0, Interp::Hold), key(1000, 8.0, Interp::Linear)];
        assert_eq!(eval_gain_db(0.0, &hold, 999), 0.0);

        let bezier = [key(0, 0.0, Interp::Bezier), key(1000, 8.0, Interp::Linear)];
        assert!((eval_gain_db(0.0, &bezier, 250) - 1.25).abs() < 1e-9);
    }

    #[test]
    fn duplicate_keyframe_times_keep_the_later_value() {
        let keys = [key(0, 1.0, Interp::Linear), key(0, 4.0, Interp::Linear)];
        assert_eq!(eval_gain_db(0.0, &keys, 0), 4.0);
    }
}
