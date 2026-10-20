use videocut_model::{effect_ids, EffectInstance};

use crate::error::ColorError;
use crate::hsl::hsl_to_rgb;
use crate::params::{effect, float_param, optional_float};
use crate::rgb::{luma, Rgb};

pub const SHADOWS_X: &str = "shadowsX";
pub const SHADOWS_Y: &str = "shadowsY";
pub const SHADOWS_MASTER: &str = "shadowsMaster";
pub const MIDTONES_X: &str = "midtonesX";
pub const MIDTONES_Y: &str = "midtonesY";
pub const MIDTONES_MASTER: &str = "midtonesMaster";
pub const HIGHLIGHTS_X: &str = "highlightsX";
pub const HIGHLIGHTS_Y: &str = "highlightsY";
pub const HIGHLIGHTS_MASTER: &str = "highlightsMaster";

pub fn color_wheels_effect() -> EffectInstance {
    effect(
        effect_ids::COLOR_WHEELS,
        vec![
            float_param(SHADOWS_X, 0.0),
            float_param(SHADOWS_Y, 0.0),
            float_param(SHADOWS_MASTER, 0.0),
            float_param(MIDTONES_X, 0.0),
            float_param(MIDTONES_Y, 0.0),
            float_param(MIDTONES_MASTER, 0.0),
            float_param(HIGHLIGHTS_X, 0.0),
            float_param(HIGHLIGHTS_Y, 0.0),
            float_param(HIGHLIGHTS_MASTER, 0.0),
        ],
    )
}

/// Color balance for one wheel. `x` and `y` are the puck position in -1..1.
/// Angle 0 (+X) is red, 120° is green, and 240° is blue. Radius is the amount.
pub fn wheel_balance(x: f32, y: f32) -> Rgb {
    let radius = x.hypot(y).min(1.0);
    if radius <= f32::EPSILON {
        return Rgb::splat(0.0);
    }
    let hue = y.atan2(x).to_degrees();
    let [r, g, b] = hsl_to_rgb(hue, 1.0, 0.5);
    Rgb::new(
        (r * 2.0 - 1.0) * radius,
        (g * 2.0 - 1.0) * radius,
        (b * 2.0 - 1.0) * radius,
    )
}

pub(crate) fn apply(effect: &EffectInstance, pixels: &mut [Rgb]) -> Result<(), ColorError> {
    let shadows = delta(effect, SHADOWS_X, SHADOWS_Y, SHADOWS_MASTER)?;
    let midtones = delta(effect, MIDTONES_X, MIDTONES_Y, MIDTONES_MASTER)?;
    let highlights = delta(effect, HIGHLIGHTS_X, HIGHLIGHTS_Y, HIGHLIGHTS_MASTER)?;

    for pixel in pixels.iter_mut() {
        let y = luma(*pixel).clamp(0.0, 1.0);
        let shadow_w = shadow_weight(y);
        let midtone_w = midtone_weight(y);
        let highlight_w = highlight_weight(y);
        let lifted = *pixel + shadows * shadow_w + midtones * midtone_w;
        let gain = Rgb::splat(1.0) + highlights * highlight_w;
        *pixel = lifted.mul_rgb(gain);
    }
    Ok(())
}

fn delta(
    effect: &EffectInstance,
    x_id: &str,
    y_id: &str,
    master_id: &str,
) -> Result<Rgb, ColorError> {
    let x = optional_float(effect, x_id, 0.0)?;
    let y = optional_float(effect, y_id, 0.0)?;
    let master = optional_float(effect, master_id, 0.0)?;
    Ok(wheel_balance(x, y) + Rgb::splat(master))
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Full weight at black, gone by mid gray.
fn shadow_weight(y: f32) -> f32 {
    1.0 - smoothstep(0.15, 0.55, y)
}

/// Peaks at mid gray and is zero at black and white.
fn midtone_weight(y: f32) -> f32 {
    let rise = smoothstep(0.15, 0.5, y);
    let fall = 1.0 - smoothstep(0.5, 0.85, y);
    rise * fall
}

/// Zero through the lower half, full weight at white.
fn highlight_weight(y: f32) -> f32 {
    smoothstep(0.45, 0.85, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grade(edits: &[(&str, f64)]) -> EffectInstance {
        let mut effect = color_wheels_effect();
        for (id, value) in edits {
            let param = effect
                .params
                .iter_mut()
                .find(|param| param.id == *id)
                .unwrap();
            param.value = videocut_model::ParamValue::Float(*value);
        }
        effect
    }

    fn apply_one(effect: &EffectInstance, pixel: Rgb) -> Rgb {
        let mut pixels = [pixel];
        apply(effect, &mut pixels).unwrap();
        pixels[0]
    }

    #[test]
    fn neutral_wheels_leave_pixels_unchanged() {
        let effect = color_wheels_effect();
        let samples = [
            Rgb::new(0.0, 0.0, 0.0),
            Rgb::new(0.25, 0.5, 0.75),
            Rgb::new(1.0, 1.0, 1.0),
        ];
        for sample in samples {
            assert_eq!(apply_one(&effect, sample), sample);
        }
    }

    #[test]
    fn shadows_master_lifts_black_and_spares_white() {
        let effect = grade(&[(SHADOWS_MASTER, 0.2)]);
        let black = apply_one(&effect, Rgb::splat(0.0));
        let white = apply_one(&effect, Rgb::splat(1.0));
        assert!(black.max_abs_diff(Rgb::splat(0.2)) < 1e-5);
        assert_eq!(white, Rgb::splat(1.0));
    }

    #[test]
    fn midtones_master_moves_gray_only() {
        let effect = grade(&[(MIDTONES_MASTER, 0.2)]);
        let black = apply_one(&effect, Rgb::splat(0.0));
        let gray = apply_one(&effect, Rgb::splat(0.5));
        let white = apply_one(&effect, Rgb::splat(1.0));
        assert_eq!(black, Rgb::splat(0.0));
        assert_eq!(white, Rgb::splat(1.0));
        assert!(gray.max_abs_diff(Rgb::splat(0.7)) < 1e-5);
    }

    #[test]
    fn highlights_master_scales_white_and_spares_black() {
        let effect = grade(&[(HIGHLIGHTS_MASTER, 0.2)]);
        let black = apply_one(&effect, Rgb::splat(0.0));
        let white = apply_one(&effect, Rgb::splat(1.0));
        assert_eq!(black, Rgb::splat(0.0));
        assert!(white.max_abs_diff(Rgb::splat(1.2)) < 1e-5);
    }

    #[test]
    fn shadows_puck_toward_red_pushes_red_in_the_shadows() {
        let effect = grade(&[(SHADOWS_X, 1.0), (SHADOWS_Y, 0.0)]);
        let black = apply_one(&effect, Rgb::splat(0.0));
        let white = apply_one(&effect, Rgb::splat(1.0));
        assert!(black.max_abs_diff(Rgb::new(1.0, -1.0, -1.0)) < 1e-5);
        assert_eq!(white, Rgb::splat(1.0));
    }

    #[test]
    fn positive_x_balance_is_red() {
        let balance = wheel_balance(1.0, 0.0);
        assert!(balance.max_abs_diff(Rgb::new(1.0, -1.0, -1.0)) < 1e-5);
        assert_eq!(wheel_balance(0.0, 0.0), Rgb::splat(0.0));
    }
}
