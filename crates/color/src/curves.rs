use videocut_model::{effect_ids, EffectInstance};

use crate::error::ColorError;
use crate::params::{effect, optional_string, string_param};
use crate::rgb::Rgb;

pub const MASTER: &str = "master";
pub const RED: &str = "red";
pub const GREEN: &str = "green";
pub const BLUE: &str = "blue";
pub const IDENTITY: &str = "0,0;1,1";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurvePoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    points: Vec<CurvePoint>,
}

impl Curve {
    pub fn points(&self) -> &[CurvePoint] {
        &self.points
    }

    pub fn sample(&self, x: f32) -> f32 {
        let points = &self.points;
        if points.is_empty() {
            return x;
        }
        if points.len() == 1 || x <= points[0].x {
            return points[0].y;
        }
        let last = points[points.len() - 1];
        if x >= last.x {
            return last.y;
        }
        for pair in points.windows(2) {
            let (left, right) = (pair[0], pair[1]);
            if x >= left.x && x <= right.x {
                let span = right.x - left.x;
                let t = if span.abs() < 1e-8 {
                    0.0
                } else {
                    (x - left.x) / span
                };
                return left.y + (right.y - left.y) * t;
            }
        }
        last.y
    }
}

pub fn identity_curve() -> Curve {
    parse_curve(IDENTITY).expect("identity curve")
}

/// Control points are `x,y` pairs separated by `;`. Points are sorted by x.
/// A later point replaces an earlier point with the same x.
pub fn parse_curve(source: &str) -> Result<Curve, ColorError> {
    let source = source.trim();
    if source.is_empty() {
        return Ok(identity_curve());
    }
    let mut points = Vec::new();
    for (index, part) in source.split(';').enumerate() {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let mut pieces = part.split(',');
        let x = parse_component(pieces.next(), index)?;
        let y = parse_component(pieces.next(), index)?;
        if pieces.next().is_some() {
            return Err(ColorError::InvalidCurve(format!(
                "point {index} has extra values"
            )));
        }
        points.push(CurvePoint { x, y });
    }
    if points.is_empty() {
        return Err(ColorError::InvalidCurve("no control points".into()));
    }
    points.sort_by(|a, b| a.x.total_cmp(&b.x));
    let mut collapsed: Vec<CurvePoint> = Vec::with_capacity(points.len());
    for point in points {
        if let Some(previous) = collapsed.last_mut() {
            if (previous.x - point.x).abs() <= 1e-6 {
                *previous = point;
                continue;
            }
        }
        collapsed.push(point);
    }
    Ok(Curve { points: collapsed })
}

pub fn format_curve(points: &[CurvePoint]) -> String {
    points
        .iter()
        .map(|point| format!("{},{}", point.x, point.y))
        .collect::<Vec<_>>()
        .join(";")
}

pub fn curves_effect() -> EffectInstance {
    effect(
        effect_ids::CURVES,
        vec![
            string_param(MASTER, IDENTITY),
            string_param(RED, IDENTITY),
            string_param(GREEN, IDENTITY),
            string_param(BLUE, IDENTITY),
        ],
    )
}

pub(crate) fn apply(effect: &EffectInstance, pixels: &mut [Rgb]) -> Result<(), ColorError> {
    let master = parse_curve(&optional_string(effect, MASTER, IDENTITY)?)?;
    let red = parse_curve(&optional_string(effect, RED, IDENTITY)?)?;
    let green = parse_curve(&optional_string(effect, GREEN, IDENTITY)?)?;
    let blue = parse_curve(&optional_string(effect, BLUE, IDENTITY)?)?;
    for pixel in pixels {
        let mastered = Rgb::new(
            master.sample(pixel.r),
            master.sample(pixel.g),
            master.sample(pixel.b),
        );
        *pixel = Rgb::new(
            red.sample(mastered.r),
            green.sample(mastered.g),
            blue.sample(mastered.b),
        );
    }
    Ok(())
}

fn parse_component(raw: Option<&str>, index: usize) -> Result<f32, ColorError> {
    let Some(raw) = raw.map(str::trim).filter(|text| !text.is_empty()) else {
        return Err(ColorError::InvalidCurve(format!(
            "point {index} needs x and y"
        )));
    };
    let value: f32 = raw
        .parse()
        .map_err(|_| ColorError::InvalidCurve(format!("point {index} is not a number")))?;
    if !value.is_finite() {
        return Err(ColorError::InvalidCurve(format!(
            "point {index} must be finite"
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use videocut_model::ParamValue;

    #[test]
    fn identity_curve_is_a_straight_line() {
        let curve = parse_curve(IDENTITY).unwrap();
        assert!((curve.sample(0.25) - 0.25).abs() < 1e-6);
        assert!((curve.sample(-1.0) - 0.0).abs() < 1e-6);
        assert!((curve.sample(2.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn samples_between_control_points() {
        let curve = parse_curve("0,0;0.5,0.25;1,1").unwrap();
        assert!((curve.sample(0.25) - 0.125).abs() < 1e-6);
        assert!((curve.sample(0.75) - 0.625).abs() < 1e-6);
    }

    #[test]
    fn sorts_points_and_keeps_the_later_duplicate() {
        let curve = parse_curve("1,1;0,0;0.5,0.25;0.5,0.5").unwrap();
        assert_eq!(curve.points()[0].x, 0.0);
        assert!((curve.sample(0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn rejects_malformed_points() {
        assert!(parse_curve("nope").is_err());
        assert!(parse_curve("0,0,1").is_err());
        assert!(parse_curve("0,nan;1,1").is_err());
    }

    #[test]
    fn master_then_channel_curves() {
        let mut effect = curves_effect();
        set(&mut effect, MASTER, "0,0;1,0.5");
        set(&mut effect, RED, "0,0;0.5,1;1,1");
        let mut pixels = [Rgb::splat(1.0)];
        apply(&effect, &mut pixels).unwrap();
        assert!(pixels[0].max_abs_diff(Rgb::new(1.0, 0.5, 0.5)) < 1e-5);
    }

    #[test]
    fn neutral_curves_match_the_source() {
        let effect = curves_effect();
        let mut pixels = [Rgb::new(0.2, 0.4, 0.8)];
        apply(&effect, &mut pixels).unwrap();
        assert!(pixels[0].max_abs_diff(Rgb::new(0.2, 0.4, 0.8)) < 1e-6);
    }

    fn set(effect: &mut EffectInstance, id: &str, value: &str) {
        let param = effect
            .params
            .iter_mut()
            .find(|param| param.id == id)
            .unwrap();
        param.value = ParamValue::Str(value.to_string());
    }
}
