use videocut_model::{effect_ids, EffectInstance};

use crate::error::ColorError;
use crate::params::{effect, float_param, optional_float};
use crate::rgb::Rgb;

pub const HUE: &str = "hue";
pub const SATURATION: &str = "saturation";
pub const LIGHTNESS: &str = "lightness";
pub const HUE_CENTER: &str = "hueCenter";
pub const HUE_WIDTH: &str = "hueWidth";
pub const SAT_LOW: &str = "satLow";
pub const SAT_HIGH: &str = "satHigh";
pub const LUM_LOW: &str = "lumLow";
pub const LUM_HIGH: &str = "lumHigh";
pub const SOFTNESS: &str = "softness";

pub fn hsl_effect() -> EffectInstance {
    effect(
        effect_ids::HSL,
        vec![
            float_param(HUE, 0.0),
            float_param(SATURATION, 1.0),
            float_param(LIGHTNESS, 0.0),
            float_param(HUE_CENTER, 0.0),
            float_param(HUE_WIDTH, 360.0),
            float_param(SAT_LOW, 0.0),
            float_param(SAT_HIGH, 1.0),
            float_param(LUM_LOW, 0.0),
            float_param(LUM_HIGH, 1.0),
            float_param(SOFTNESS, 0.0),
        ],
    )
}

pub fn rgb_to_hsl(r: f32, g: f32, b: f32) -> [f32; 3] {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let lightness = (max + min) * 0.5;
    let delta = max - min;
    if delta <= 1e-6 {
        return [0.0, 0.0, lightness];
    }
    let saturation = if lightness > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };
    let hue = if (max - r).abs() <= 1e-6 {
        60.0 * (((g - b) / delta).rem_euclid(6.0))
    } else if (max - g).abs() <= 1e-6 {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    [hue.rem_euclid(360.0), saturation.clamp(0.0, 1.0), lightness]
}

pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    let h = h.rem_euclid(360.0);
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    if s <= 1e-6 {
        return [l, l, l];
    }
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let m = l - c * 0.5;
    let (r, g, b) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r + m, g + m, b + m]
}

pub(crate) fn apply(instance: &EffectInstance, pixels: &mut [Rgb]) -> Result<(), ColorError> {
    let hue_shift = optional_float(instance, HUE, 0.0)?;
    let saturation = optional_float(instance, SATURATION, 1.0)?;
    let lightness = optional_float(instance, LIGHTNESS, 0.0)?;
    let hue_center = optional_float(instance, HUE_CENTER, 0.0)?.rem_euclid(360.0);
    let hue_width = optional_float(instance, HUE_WIDTH, 360.0)?.clamp(0.0, 360.0);
    let sat_low = optional_float(instance, SAT_LOW, 0.0)?;
    let sat_high = optional_float(instance, SAT_HIGH, 1.0)?;
    let lum_low = optional_float(instance, LUM_LOW, 0.0)?;
    let lum_high = optional_float(instance, LUM_HIGH, 1.0)?;
    let softness = optional_float(instance, SOFTNESS, 0.0)?.clamp(0.0, 1.0);

    let qualifier = Qualifier {
        hue_center,
        hue_width,
        sat_low,
        sat_high,
        lum_low,
        lum_high,
        softness,
    };
    for pixel in pixels {
        let [h, s, l] = rgb_to_hsl(pixel.r, pixel.g, pixel.b);
        let weight = qualifier.weight(h, s, l);
        if weight <= 0.0 {
            continue;
        }
        let [r, g, b] = hsl_to_rgb(
            h + hue_shift,
            s * saturation,
            (l + lightness).clamp(0.0, 1.0),
        );
        let graded = Rgb::new(r, g, b);
        *pixel = pixel.lerp(graded, weight);
    }
    Ok(())
}

struct Qualifier {
    hue_center: f32,
    hue_width: f32,
    sat_low: f32,
    sat_high: f32,
    lum_low: f32,
    lum_high: f32,
    softness: f32,
}

impl Qualifier {
    fn weight(&self, hue: f32, saturation: f32, lightness: f32) -> f32 {
        let hue_w = if self.hue_width >= 360.0 - f32::EPSILON {
            1.0
        } else {
            let half = self.hue_width * 0.5;
            let feather = self.softness * 30.0;
            feather_inside(hue_distance(hue, self.hue_center), half, feather)
        };
        let sat_w = range_weight(
            saturation,
            self.sat_low,
            self.sat_high,
            self.softness * 0.25,
        );
        let lum_w = range_weight(lightness, self.lum_low, self.lum_high, self.softness * 0.25);
        hue_w * sat_w * lum_w
    }
}

fn hue_distance(a: f32, b: f32) -> f32 {
    let delta = (a - b).rem_euclid(360.0);
    delta.min(360.0 - delta)
}

fn range_weight(value: f32, low: f32, high: f32, feather: f32) -> f32 {
    let (low, high) = if low <= high {
        (low, high)
    } else {
        (high, low)
    };
    if feather <= f32::EPSILON {
        return if value >= low && value <= high {
            1.0
        } else {
            0.0
        };
    }
    let enter = smoothstep(low - feather, low, value);
    let exit = 1.0 - smoothstep(high, high + feather, value);
    enter * exit
}

fn feather_inside(distance: f32, limit: f32, feather: f32) -> f32 {
    if distance <= limit {
        1.0
    } else if feather <= f32::EPSILON {
        0.0
    } else {
        1.0 - smoothstep(limit, limit + feather, distance)
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use videocut_model::ParamValue;

    fn apply_one(effect: &EffectInstance, pixel: Rgb) -> Rgb {
        let mut pixels = [pixel];
        apply(effect, &mut pixels).unwrap();
        pixels[0]
    }

    fn set(effect: &mut EffectInstance, id: &str, value: f64) {
        let param = effect
            .params
            .iter_mut()
            .find(|param| param.id == id)
            .unwrap();
        param.value = ParamValue::Float(value);
    }

    #[test]
    fn round_trips_primary_colors() {
        for pixel in [
            Rgb::new(1.0, 0.0, 0.0),
            Rgb::new(0.0, 1.0, 0.0),
            Rgb::new(0.0, 0.0, 1.0),
        ] {
            let [h, s, l] = rgb_to_hsl(pixel.r, pixel.g, pixel.b);
            let [r, g, b] = hsl_to_rgb(h, s, l);
            assert!(
                pixel.max_abs_diff(Rgb::new(r, g, b)) < 1e-5,
                "{pixel:?} -> {h},{s},{l}"
            );
        }
    }

    #[test]
    fn neutral_hsl_is_identity() {
        let pixel = Rgb::new(0.2, 0.4, 0.6);
        assert!(apply_one(&hsl_effect(), pixel).max_abs_diff(pixel) < 1e-5);
    }

    #[test]
    fn hue_shift_rotates_red_to_green() {
        let mut effect = hsl_effect();
        set(&mut effect, HUE, 120.0);
        let graded = apply_one(&effect, Rgb::new(1.0, 0.0, 0.0));
        assert!(graded.max_abs_diff(Rgb::new(0.0, 1.0, 0.0)) < 1e-5);
    }

    #[test]
    fn zero_saturation_makes_gray() {
        let mut effect = hsl_effect();
        set(&mut effect, SATURATION, 0.0);
        let graded = apply_one(&effect, Rgb::new(1.0, 0.0, 0.0));
        assert!(graded.max_abs_diff(Rgb::splat(0.5)) < 1e-5);
    }

    #[test]
    fn qualifier_limits_the_hue_shift() {
        let mut effect = hsl_effect();
        set(&mut effect, HUE, 120.0);
        set(&mut effect, HUE_CENTER, 120.0);
        set(&mut effect, HUE_WIDTH, 20.0);
        set(&mut effect, SOFTNESS, 0.0);
        let red = apply_one(&effect, Rgb::new(1.0, 0.0, 0.0));
        let green = apply_one(&effect, Rgb::new(0.0, 1.0, 0.0));
        assert!(red.max_abs_diff(Rgb::new(1.0, 0.0, 0.0)) < 1e-5);
        assert!(green.max_abs_diff(Rgb::new(0.0, 0.0, 1.0)) < 1e-5);
    }
}
