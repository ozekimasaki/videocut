use videocut_model::{new_id, EffectInstance, Param, ParamValue};

use crate::error::ColorError;

pub(crate) fn float_param(id: &str, value: f64) -> Param {
    Param {
        id: id.to_string(),
        value: ParamValue::Float(value),
        keyframes: Vec::new(),
    }
}

pub(crate) fn string_param(id: &str, value: impl Into<String>) -> Param {
    Param {
        id: id.to_string(),
        value: ParamValue::Str(value.into()),
        keyframes: Vec::new(),
    }
}

pub(crate) fn effect(effect_id: &str, params: Vec<Param>) -> EffectInstance {
    EffectInstance {
        id: new_id("fx"),
        effect_id: effect_id.to_string(),
        enabled: true,
        params,
    }
}

pub(crate) fn optional_float(
    effect: &EffectInstance,
    id: &str,
    default: f32,
) -> Result<f32, ColorError> {
    let Some(param) = effect.params.iter().find(|param| param.id == id) else {
        return Ok(default);
    };
    let value = match &param.value {
        ParamValue::Float(value) => *value as f32,
        ParamValue::Int(value) => *value as f32,
        _ => {
            return Err(ColorError::InvalidParam {
                id: id.to_string(),
                detail: "must be a number".to_string(),
            })
        }
    };
    if !value.is_finite() {
        return Err(ColorError::InvalidParam {
            id: id.to_string(),
            detail: "must be finite".to_string(),
        });
    }
    Ok(value)
}

pub(crate) fn optional_string(
    effect: &EffectInstance,
    id: &str,
    default: &str,
) -> Result<String, ColorError> {
    let Some(param) = effect.params.iter().find(|param| param.id == id) else {
        return Ok(default.to_string());
    };
    match &param.value {
        ParamValue::Str(value) => Ok(value.clone()),
        _ => Err(ColorError::InvalidParam {
            id: id.to_string(),
            detail: "must be a string".to_string(),
        }),
    }
}
