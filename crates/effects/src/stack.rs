use serde::{Deserialize, Serialize};
use videocut_model::{effect_ids, Clip, EffectInstance, ParamValue, Point2};

use crate::affine::Affine;
use crate::catalog::definition;
use crate::eval::evaluate_param;
use crate::param_ids;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EffectError {
    #[error("effect instance `{0}` was not found")]
    NotFound(String),
    #[error("parameter `{param}` was not found on effect `{effect}`")]
    ParamNotFound { effect: String, param: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluatedParam {
    pub id: String,
    pub value: ParamValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluatedEffect {
    pub id: String,
    pub effect_id: String,
    pub enabled: bool,
    pub params: Vec<EvaluatedParam>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Crop {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Crop {
    pub fn none() -> Self {
        Self {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        }
    }

    pub fn intersect(self, other: Self) -> Self {
        Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }
}

/// Enabled filters composed in stack order. The first effect is applied first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackEvaluation {
    pub effects: Vec<EvaluatedEffect>,
    pub affine: Affine,
    pub opacity: f64,
    pub crop: Crop,
}

pub fn local_ticks(clip: &Clip, timeline_ticks: i64) -> i64 {
    timeline_ticks.saturating_sub(clip.timeline_in)
}

pub fn evaluate_stack(effects: &[EffectInstance], local_ticks: i64) -> StackEvaluation {
    let mut affine = Affine::identity();
    let mut opacity = 1.0;
    let mut crop = Crop::none();
    let mut evaluated = Vec::with_capacity(effects.len());

    for effect in effects {
        let params = evaluate_effect_params(effect, local_ticks);
        if effect.enabled {
            match effect.effect_id.as_str() {
                effect_ids::MOTION | effect_ids::TRANSFORM => {
                    affine = placement_matrix(&params, effect.effect_id.as_str()) * affine;
                }
                effect_ids::OPACITY => {
                    opacity *= param_float(&params, param_ids::OPACITY, 1.0).clamp(0.0, 1.0);
                }
                effect_ids::CROP => {
                    crop = crop.intersect(crop_from(&params));
                }
                _ => {}
            }
        }
        evaluated.push(EvaluatedEffect {
            id: effect.id.clone(),
            effect_id: effect.effect_id.clone(),
            enabled: effect.enabled,
            params,
        });
    }

    StackEvaluation {
        effects: evaluated,
        affine,
        opacity,
        crop,
    }
}

pub fn insert_effect(stack: &mut Vec<EffectInstance>, index: usize, effect: EffectInstance) {
    let index = index.min(stack.len());
    stack.insert(index, effect);
}

pub fn remove_effect(stack: &mut Vec<EffectInstance>, id: &str) -> Option<EffectInstance> {
    let index = stack.iter().position(|effect| effect.id == id)?;
    Some(stack.remove(index))
}

pub fn move_effect(
    stack: &mut Vec<EffectInstance>,
    id: &str,
    new_index: usize,
) -> Result<(), EffectError> {
    let current = stack
        .iter()
        .position(|effect| effect.id == id)
        .ok_or_else(|| EffectError::NotFound(id.to_string()))?;
    let effect = stack.remove(current);
    let insert_at = new_index.min(stack.len());
    stack.insert(insert_at, effect);
    Ok(())
}

pub fn set_enabled(
    stack: &mut [EffectInstance],
    id: &str,
    enabled: bool,
) -> Result<(), EffectError> {
    let effect = stack
        .iter_mut()
        .find(|effect| effect.id == id)
        .ok_or_else(|| EffectError::NotFound(id.to_string()))?;
    effect.enabled = enabled;
    Ok(())
}

pub fn set_param_value(
    stack: &mut [EffectInstance],
    effect_instance_id: &str,
    param_id: &str,
    value: ParamValue,
) -> Result<(), EffectError> {
    let effect = stack
        .iter_mut()
        .find(|effect| effect.id == effect_instance_id)
        .ok_or_else(|| EffectError::NotFound(effect_instance_id.to_string()))?;
    let param = effect
        .params
        .iter_mut()
        .find(|param| param.id == param_id)
        .ok_or_else(|| EffectError::ParamNotFound {
            effect: effect_instance_id.to_string(),
            param: param_id.to_string(),
        })?;
    param.value = value;
    Ok(())
}

fn evaluate_effect_params(effect: &EffectInstance, local_ticks: i64) -> Vec<EvaluatedParam> {
    let mut used = Vec::new();
    let mut out = Vec::new();
    if let Some(spec) = definition(&effect.effect_id) {
        for param_spec in spec.params {
            used.push(param_spec.id);
            let value = effect
                .params
                .iter()
                .find(|param| param.id == param_spec.id)
                .map(|param| evaluate_param(param, local_ticks))
                .unwrap_or_else(|| (param_spec.default_value)());
            out.push(EvaluatedParam {
                id: param_spec.id.to_string(),
                value,
            });
        }
    }
    for param in &effect.params {
        if !used.iter().any(|id| *id == param.id) {
            out.push(EvaluatedParam {
                id: param.id.clone(),
                value: evaluate_param(param, local_ticks),
            });
        }
    }
    out
}

fn placement_matrix(params: &[EvaluatedParam], effect_id: &str) -> Affine {
    let anchor = param_point(params, param_ids::ANCHOR, Point2 { x: 0.5, y: 0.5 });
    let position = param_point(params, param_ids::POSITION, Point2 { x: 0.5, y: 0.5 });
    let scale = param_point(params, param_ids::SCALE, Point2 { x: 1.0, y: 1.0 });
    let rotation = param_float(params, param_ids::ROTATION, 0.0);
    let (skew, skew_axis) = if effect_id == effect_ids::TRANSFORM {
        (
            param_float(params, param_ids::SKEW, 0.0),
            param_float(params, param_ids::SKEW_AXIS, 0.0),
        )
    } else {
        (0.0, 0.0)
    };
    let skew_matrix = Affine::rotate(skew_axis) * Affine::skew_x(skew) * Affine::rotate(-skew_axis);
    Affine::translate(position.x, position.y)
        * Affine::rotate(rotation)
        * skew_matrix
        * Affine::scale(scale.x, scale.y)
        * Affine::translate(-anchor.x, -anchor.y)
}

fn crop_from(params: &[EvaluatedParam]) -> Crop {
    Crop {
        left: param_float(params, param_ids::LEFT, 0.0).clamp(0.0, 1.0),
        top: param_float(params, param_ids::TOP, 0.0).clamp(0.0, 1.0),
        right: param_float(params, param_ids::RIGHT, 0.0).clamp(0.0, 1.0),
        bottom: param_float(params, param_ids::BOTTOM, 0.0).clamp(0.0, 1.0),
    }
}

fn param_float(params: &[EvaluatedParam], id: &str, default: f64) -> f64 {
    params
        .iter()
        .find(|param| param.id == id)
        .and_then(|param| match param.value {
            ParamValue::Float(value) => Some(value),
            ParamValue::Int(value) => Some(value as f64),
            _ => None,
        })
        .unwrap_or(default)
}

fn param_point(params: &[EvaluatedParam], id: &str, default: Point2) -> Point2 {
    params
        .iter()
        .find(|param| param.id == id)
        .and_then(|param| match param.value {
            ParamValue::Point(value) => Some(value),
            _ => None,
        })
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{
        crop_effect, default_video_stack, motion_effect, opacity_effect, transform_effect,
    };
    use crate::param_ids;
    use videocut_model::{EffectInstance, Interp, Keyframe, ParamValue, Point2};

    fn assert_close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    fn set_float(effect: &mut EffectInstance, id: &str, value: f64) {
        effect
            .params
            .iter_mut()
            .find(|param| param.id == id)
            .unwrap()
            .value = ParamValue::Float(value);
    }

    fn set_point(effect: &mut EffectInstance, id: &str, x: f64, y: f64) {
        effect
            .params
            .iter_mut()
            .find(|param| param.id == id)
            .unwrap()
            .value = ParamValue::Point(Point2 { x, y });
    }

    #[test]
    fn default_stack_is_identity() {
        let evaluated = evaluate_stack(&default_video_stack(), 0);
        let (x, y) = evaluated.affine.apply(0.25, 0.75);
        assert_close(x, 0.25);
        assert_close(y, 0.75);
        assert_close(evaluated.opacity, 1.0);
        assert_eq!(evaluated.crop, Crop::none());
        assert_eq!(evaluated.effects.len(), 2);
    }

    #[test]
    fn opacity_multiplies_and_disabled_is_ignored() {
        let mut first = opacity_effect();
        let mut second = opacity_effect();
        set_float(&mut first, param_ids::OPACITY, 0.5);
        set_float(&mut second, param_ids::OPACITY, 0.5);
        let stacked = evaluate_stack(&[first.clone(), second.clone()], 0);
        assert_close(stacked.opacity, 0.25);

        second.enabled = false;
        let disabled = evaluate_stack(&[first, second], 0);
        assert_close(disabled.opacity, 0.5);
    }

    #[test]
    fn crop_intersects_in_stack_order() {
        let mut wide = crop_effect();
        let mut tight = crop_effect();
        set_float(&mut wide, param_ids::LEFT, 0.1);
        set_float(&mut wide, param_ids::RIGHT, 0.1);
        set_float(&mut tight, param_ids::LEFT, 0.25);
        set_float(&mut tight, param_ids::TOP, 0.2);
        let evaluated = evaluate_stack(&[wide, tight], 0);
        assert_close(evaluated.crop.left, 0.25);
        assert_close(evaluated.crop.top, 0.2);
        assert_close(evaluated.crop.right, 0.1);
        assert_close(evaluated.crop.bottom, 0.0);
    }

    #[test]
    fn scale_and_rotation_compose() {
        let mut scaled = motion_effect();
        set_point(&mut scaled, param_ids::SCALE, 2.0, 2.0);
        let mut halved = transform_effect();
        set_point(&mut halved, param_ids::SCALE, 0.5, 0.5);
        let composed = evaluate_stack(&[scaled, halved], 0);
        let (x, y) = composed.affine.apply(0.0, 0.5);
        assert_close(x, 0.0);
        assert_close(y, 0.5);

        let mut turned = motion_effect();
        set_float(&mut turned, param_ids::ROTATION, 90.0);
        let rotated = evaluate_stack(&[turned], 0);
        let (x, y) = rotated.affine.apply(0.5, 0.0);
        assert_close(x, 1.0);
        assert_close(y, 0.5);
    }

    #[test]
    fn keyframed_opacity_uses_local_ticks() {
        let mut effect = opacity_effect();
        effect.params[0].keyframes = vec![
            Keyframe {
                time_ticks: 0,
                value: ParamValue::Float(0.0),
                interp: Interp::Linear,
            },
            Keyframe {
                time_ticks: 100,
                value: ParamValue::Float(1.0),
                interp: Interp::Linear,
            },
        ];
        let evaluated = evaluate_stack(std::slice::from_ref(&effect), 40);
        assert_close(evaluated.opacity, 0.4);
    }

    #[test]
    fn unknown_effect_is_kept_and_does_not_move_placement() {
        let custom = EffectInstance {
            id: "fx-custom".into(),
            effect_id: "custom.glow".into(),
            enabled: true,
            params: vec![videocut_model::Param {
                id: "amount".into(),
                value: ParamValue::Float(0.5),
                keyframes: Vec::new(),
            }],
        };
        let evaluated = evaluate_stack(&[custom], 0);
        assert_close(evaluated.opacity, 1.0);
        assert_eq!(evaluated.effects[0].params[0].id, "amount");
        let (x, y) = evaluated.affine.apply(0.2, 0.3);
        assert_close(x, 0.2);
        assert_close(y, 0.3);
    }

    #[test]
    fn move_remove_and_enable() {
        let mut stack = default_video_stack();
        let motion_id = stack[0].id.clone();
        move_effect(&mut stack, &motion_id, 1).unwrap();
        assert_eq!(stack[1].effect_id, effect_ids::MOTION);
        set_enabled(&mut stack, &motion_id, false).unwrap();
        assert!(!stack[1].enabled);
        assert!(remove_effect(&mut stack, &motion_id).is_some());
        assert_eq!(stack.len(), 1);
        assert!(matches!(
            move_effect(&mut stack, "missing", 0),
            Err(EffectError::NotFound(_))
        ));
    }

    #[test]
    fn evaluation_json_uses_camel_case() {
        let evaluated = evaluate_stack(&default_video_stack(), 0);
        let json = serde_json::to_string(&evaluated).unwrap();
        assert!(json.contains("\"effectId\""));
        assert!(json.contains("\"opacity\":1.0") || json.contains("\"opacity\":1"));
        let back: StackEvaluation = serde_json::from_str(&json).unwrap();
        assert_eq!(evaluated.opacity, back.opacity);
        assert_eq!(evaluated.effects.len(), back.effects.len());
    }
}
