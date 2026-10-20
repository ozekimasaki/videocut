use videocut_model::{effect_ids, EffectInstance};

use crate::curves;
use crate::error::ColorError;
use crate::hsl;
use crate::lut;
use crate::rgb::Rgb;
use crate::wheels;

pub use curves::curves_effect;
pub use hsl::hsl_effect;
pub use lut::lut_effect;
pub use wheels::color_wheels_effect;

const COLOR_EFFECT_IDS: [&str; 4] = [
    effect_ids::COLOR_WHEELS,
    effect_ids::CURVES,
    effect_ids::HSL,
    effect_ids::LUT,
];

pub fn is_color_effect(effect_id: &str) -> bool {
    COLOR_EFFECT_IDS.contains(&effect_id)
}

/// Evaluates one color effect. Disabled effects are left untouched.
/// Keyframes are not sampled here; the effect engine writes the value at the playhead first.
pub fn apply_effect(effect: &EffectInstance, pixels: &mut [Rgb]) -> Result<(), ColorError> {
    if !effect.enabled {
        return Ok(());
    }
    match effect.effect_id.as_str() {
        effect_ids::COLOR_WHEELS => wheels::apply(effect, pixels),
        effect_ids::CURVES => curves::apply(effect, pixels),
        effect_ids::HSL => hsl::apply(effect, pixels),
        effect_ids::LUT => lut::apply(effect, pixels),
        other => Err(ColorError::NotColorEffect(other.to_string())),
    }
}

/// Applies enabled color effects in order and skips every other effect id.
pub fn apply_color_stack(effects: &[EffectInstance], pixels: &mut [Rgb]) -> Result<(), ColorError> {
    for effect in effects {
        if is_color_effect(&effect.effect_id) {
            apply_effect(effect, pixels)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curves::MASTER;
    use crate::hsl::SATURATION;
    use crate::wheels::SHADOWS_MASTER;
    use videocut_model::{EffectInstance, ParamValue};

    #[test]
    fn effect_ids_match_the_model() {
        assert_eq!(color_wheels_effect().effect_id, effect_ids::COLOR_WHEELS);
        assert_eq!(curves_effect().effect_id, effect_ids::CURVES);
        assert_eq!(hsl_effect().effect_id, effect_ids::HSL);
        assert_eq!(lut_effect("").effect_id, effect_ids::LUT);
        assert!(is_color_effect(effect_ids::COLOR_WHEELS));
        assert!(!is_color_effect(effect_ids::OPACITY));
    }

    #[test]
    fn wheels_json_uses_the_shared_effect_id() {
        let json = serde_json::to_value(color_wheels_effect()).unwrap();
        assert_eq!(json["effectId"], effect_ids::COLOR_WHEELS);
        assert_eq!(json["params"][0]["id"], "shadowsX");
        assert_eq!(json["params"][2]["id"], SHADOWS_MASTER);
        assert!(json["params"][0].get("keyframes").is_none());
        assert_eq!(json["params"][2]["value"]["type"], "float");
    }

    #[test]
    fn stack_runs_in_order_and_skips_disabled_and_foreign_effects() {
        let mut wheels = color_wheels_effect();
        set_float(&mut wheels, SHADOWS_MASTER, 0.25);
        let mut gray = hsl_effect();
        set_float(&mut gray, SATURATION, 0.0);
        gray.enabled = false;
        let mut crush = curves_effect();
        set_string(&mut crush, MASTER, "0,0;1,0.5");
        let foreign = EffectInstance {
            id: "fx-other".into(),
            effect_id: effect_ids::OPACITY.into(),
            enabled: true,
            params: Vec::new(),
        };

        let mut pixels = [Rgb::splat(0.0), Rgb::splat(1.0)];
        apply_color_stack(&[foreign, wheels, gray, crush], &mut pixels).unwrap();
        assert!(pixels[0].max_abs_diff(Rgb::splat(0.125)) < 1e-5);
        assert!(pixels[1].max_abs_diff(Rgb::splat(0.5)) < 1e-5);
    }

    #[test]
    fn apply_effect_rejects_a_non_color_id() {
        let effect = EffectInstance {
            id: "fx-other".into(),
            effect_id: effect_ids::MOTION.into(),
            enabled: true,
            params: Vec::new(),
        };
        let error = apply_effect(&effect, &mut [Rgb::splat(1.0)]).unwrap_err();
        assert!(matches!(error, ColorError::NotColorEffect(_)));
    }

    fn set_float(effect: &mut EffectInstance, id: &str, value: f64) {
        let param = effect
            .params
            .iter_mut()
            .find(|param| param.id == id)
            .unwrap();
        param.value = ParamValue::Float(value);
    }

    fn set_string(effect: &mut EffectInstance, id: &str, value: &str) {
        let param = effect
            .params
            .iter_mut()
            .find(|param| param.id == id)
            .unwrap();
        param.value = ParamValue::Str(value.into());
    }
}
