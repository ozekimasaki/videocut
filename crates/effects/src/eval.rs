use videocut_model::{Interp, Keyframe, Param, ParamValue, Point2, Rgba};

/// Unit cubic Bezier with control points `(0,0), (0,0), (1,1), (1,1)`.
///
/// `t = 0.25` eases to `0.15625`. `t = 0.5` stays `0.5`.
pub fn bezier_ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Value of `param` at a clip-local tick.
///
/// An empty keyframe list returns the static value. Outside the first and last
/// keys, the value is clamped to that end key. The segment uses the left key's
/// [`Interp`]. Bool steps at eased progress `0.5`. Text holds the left value
/// until the next key. Mismatched value types hold the left value.
pub fn evaluate_param(param: &Param, local_ticks: i64) -> ParamValue {
    if param.keyframes.is_empty() {
        return param.value.clone();
    }
    let keys = collapse_keys(&param.keyframes);
    if local_ticks <= keys[0].time_ticks {
        return keys[0].value.clone();
    }
    let last = keys.last().expect("keyframes is non-empty");
    if local_ticks >= last.time_ticks {
        return last.value.clone();
    }
    let index = keys.partition_point(|key| key.time_ticks <= local_ticks) - 1;
    interpolate(&keys[index], &keys[index + 1], local_ticks)
}

pub fn add_keyframe(param: &mut Param, keyframe: Keyframe) {
    if let Some(existing) = param
        .keyframes
        .iter_mut()
        .find(|key| key.time_ticks == keyframe.time_ticks)
    {
        *existing = keyframe;
        return;
    }
    param.keyframes.push(keyframe);
    param.keyframes.sort_by_key(|key| key.time_ticks);
}

pub fn remove_keyframe(param: &mut Param, time_ticks: i64) -> Option<Keyframe> {
    let index = param
        .keyframes
        .iter()
        .position(|key| key.time_ticks == time_ticks)?;
    Some(param.keyframes.remove(index))
}

fn collapse_keys(keys: &[Keyframe]) -> Vec<Keyframe> {
    let mut ordered = keys.to_vec();
    ordered.sort_by_key(|key| key.time_ticks);
    let mut collapsed: Vec<Keyframe> = Vec::with_capacity(ordered.len());
    for key in ordered {
        if collapsed
            .last()
            .is_some_and(|prev| prev.time_ticks == key.time_ticks)
        {
            collapsed.pop();
        }
        collapsed.push(key);
    }
    collapsed
}

fn interpolate(left: &Keyframe, right: &Keyframe, tick: i64) -> ParamValue {
    let span = (right.time_ticks - left.time_ticks) as f64;
    let progress = if span == 0.0 {
        1.0
    } else {
        ((tick - left.time_ticks) as f64 / span).clamp(0.0, 1.0)
    };
    let eased = match left.interp {
        Interp::Linear => progress,
        Interp::Hold => 0.0,
        Interp::Bezier => bezier_ease(progress),
    };
    match (&left.value, &right.value) {
        (ParamValue::Float(start), ParamValue::Float(end)) => {
            ParamValue::Float(lerp(*start, *end, eased))
        }
        (ParamValue::Int(start), ParamValue::Int(end)) => {
            ParamValue::Int(lerp(*start as f64, *end as f64, eased).round() as i64)
        }
        (ParamValue::Bool(start), ParamValue::Bool(end)) => {
            ParamValue::Bool(if eased >= 0.5 { *end } else { *start })
        }
        (ParamValue::Color(start), ParamValue::Color(end)) => {
            ParamValue::Color(lerp_rgba(*start, *end, eased))
        }
        (ParamValue::Point(start), ParamValue::Point(end)) => {
            ParamValue::Point(lerp_point(*start, *end, eased))
        }
        _ => left.value.clone(),
    }
}

fn lerp(start: f64, end: f64, t: f64) -> f64 {
    start + (end - start) * t
}

fn lerp_point(start: Point2, end: Point2, t: f64) -> Point2 {
    Point2 {
        x: lerp(start.x, end.x, t),
        y: lerp(start.y, end.y, t),
    }
}

fn lerp_rgba(start: Rgba, end: Rgba, t: f64) -> Rgba {
    let channel = |from: f32, to: f32| lerp(f64::from(from), f64::from(to), t) as f32;
    Rgba {
        r: channel(start.r, end.r),
        g: channel(start.g, end.g),
        b: channel(start.b, end.b),
        a: channel(start.a, end.a),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn float_param(value: f64, keys: Vec<Keyframe>) -> Param {
        Param {
            id: "opacity".into(),
            value: ParamValue::Float(value),
            keyframes: keys,
        }
    }

    fn key(time: i64, value: f64, interp: Interp) -> Keyframe {
        Keyframe {
            time_ticks: time,
            value: ParamValue::Float(value),
            interp,
        }
    }

    fn float_at(param: &Param, tick: i64) -> f64 {
        match evaluate_param(param, tick) {
            ParamValue::Float(value) => value,
            other => panic!("expected float, got {other:?}"),
        }
    }

    #[test]
    fn static_value_without_keys() {
        let param = float_param(0.25, Vec::new());
        assert_eq!(float_at(&param, 40), 0.25);
    }

    #[test]
    fn linear_hold_and_bezier_differ() {
        let endpoints = |interp| {
            float_param(
                0.0,
                vec![key(0, 0.0, interp), key(100, 100.0, Interp::Linear)],
            )
        };
        assert_close(float_at(&endpoints(Interp::Linear), 40), 40.0);
        assert_close(float_at(&endpoints(Interp::Hold), 40), 0.0);
        assert_close(float_at(&endpoints(Interp::Bezier), 40), 35.2);
        assert_close(bezier_ease(0.25), 0.15625);
        assert_close(float_at(&endpoints(Interp::Linear), 0), 0.0);
        assert_close(float_at(&endpoints(Interp::Linear), 100), 100.0);
        assert_close(float_at(&endpoints(Interp::Bezier), 100), 100.0);
    }

    #[test]
    fn clamps_outside_the_key_range() {
        let param = float_param(
            5.0,
            vec![key(10, 1.0, Interp::Linear), key(20, 2.0, Interp::Linear)],
        );
        assert_close(float_at(&param, 0), 1.0);
        assert_close(float_at(&param, 30), 2.0);
    }

    #[test]
    fn unsorted_and_duplicate_times() {
        let param = float_param(
            0.0,
            vec![
                key(100, 10.0, Interp::Linear),
                key(0, 0.0, Interp::Linear),
                key(0, 4.0, Interp::Linear),
            ],
        );
        assert_close(float_at(&param, 0), 4.0);
        assert_close(float_at(&param, 50), 7.0);
    }

    #[test]
    fn point_and_color_components() {
        let point = Param {
            id: "position".into(),
            value: ParamValue::Point(Point2 { x: 0.0, y: 0.0 }),
            keyframes: vec![
                Keyframe {
                    time_ticks: 0,
                    value: ParamValue::Point(Point2 { x: 0.0, y: 0.0 }),
                    interp: Interp::Linear,
                },
                Keyframe {
                    time_ticks: 10,
                    value: ParamValue::Point(Point2 { x: 10.0, y: 20.0 }),
                    interp: Interp::Linear,
                },
            ],
        };
        match evaluate_param(&point, 5) {
            ParamValue::Point(value) => {
                assert_close(value.x, 5.0);
                assert_close(value.y, 10.0);
            }
            other => panic!("expected point, got {other:?}"),
        }

        let color = Param {
            id: "color".into(),
            value: ParamValue::Color(Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
            keyframes: vec![
                Keyframe {
                    time_ticks: 0,
                    value: ParamValue::Color(Rgba {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    interp: Interp::Bezier,
                },
                Keyframe {
                    time_ticks: 100,
                    value: ParamValue::Color(Rgba {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    }),
                    interp: Interp::Linear,
                },
            ],
        };
        match evaluate_param(&color, 40) {
            ParamValue::Color(value) => {
                assert!((f64::from(value.r) - 0.352).abs() < 1e-5);
                assert!((f64::from(value.a) - 1.0).abs() < 1e-5);
            }
            other => panic!("expected color, got {other:?}"),
        }
    }

    #[test]
    fn bool_steps_at_midpoint_and_text_holds() {
        let flag = Param {
            id: "flag".into(),
            value: ParamValue::Bool(false),
            keyframes: vec![
                Keyframe {
                    time_ticks: 0,
                    value: ParamValue::Bool(false),
                    interp: Interp::Linear,
                },
                Keyframe {
                    time_ticks: 10,
                    value: ParamValue::Bool(true),
                    interp: Interp::Linear,
                },
            ],
        };
        assert_eq!(evaluate_param(&flag, 4), ParamValue::Bool(false));
        assert_eq!(evaluate_param(&flag, 5), ParamValue::Bool(true));

        let held = Param {
            id: "flag".into(),
            value: ParamValue::Bool(false),
            keyframes: vec![
                Keyframe {
                    time_ticks: 0,
                    value: ParamValue::Bool(false),
                    interp: Interp::Hold,
                },
                Keyframe {
                    time_ticks: 10,
                    value: ParamValue::Bool(true),
                    interp: Interp::Hold,
                },
            ],
        };
        assert_eq!(evaluate_param(&held, 9), ParamValue::Bool(false));
        assert_eq!(evaluate_param(&held, 10), ParamValue::Bool(true));

        let text = Param {
            id: "label".into(),
            value: ParamValue::Str("a".into()),
            keyframes: vec![
                Keyframe {
                    time_ticks: 0,
                    value: ParamValue::Str("a".into()),
                    interp: Interp::Linear,
                },
                Keyframe {
                    time_ticks: 10,
                    value: ParamValue::Str("b".into()),
                    interp: Interp::Linear,
                },
            ],
        };
        assert_eq!(evaluate_param(&text, 9), ParamValue::Str("a".into()));
        assert_eq!(evaluate_param(&text, 10), ParamValue::Str("b".into()));
    }

    #[test]
    fn add_and_remove_keyframe() {
        let mut param = float_param(1.0, Vec::new());
        add_keyframe(&mut param, key(10, 2.0, Interp::Hold));
        add_keyframe(&mut param, key(10, 3.0, Interp::Linear));
        assert_eq!(param.keyframes.len(), 1);
        assert_close(float_at(&param, 10), 3.0);
        assert!(remove_keyframe(&mut param, 10).is_some());
        assert!(param.keyframes.is_empty());
        assert_close(float_at(&param, 10), 1.0);
    }

    fn assert_close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }
}
