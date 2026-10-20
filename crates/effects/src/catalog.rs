use videocut_model::{
    effect_ids, new_id, Clip, ClipKind, EffectInstance, Param, ParamValue, Point2,
};

use crate::param_ids;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKind {
    Filter,
    Transition,
}

#[derive(Clone, Copy)]
pub struct ParamSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub default_value: fn() -> ParamValue,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

pub struct EffectSpec {
    pub effect_id: &'static str,
    pub label: &'static str,
    pub kind: EffectKind,
    pub params: &'static [ParamSpec],
}

fn point(x: f64, y: f64) -> ParamValue {
    ParamValue::Point(Point2 { x, y })
}

fn float_value(value: f64) -> ParamValue {
    ParamValue::Float(value)
}

fn anchor_default() -> ParamValue {
    point(0.5, 0.5)
}

fn position_default() -> ParamValue {
    point(0.5, 0.5)
}

fn scale_default() -> ParamValue {
    point(1.0, 1.0)
}

fn zero() -> ParamValue {
    float_value(0.0)
}

fn one() -> ParamValue {
    float_value(1.0)
}

const PLACEMENT_PARAMS: [ParamSpec; 4] = [
    ParamSpec {
        id: param_ids::ANCHOR,
        label: "Anchor",
        default_value: anchor_default,
        min: Some(0.0),
        max: Some(1.0),
    },
    ParamSpec {
        id: param_ids::POSITION,
        label: "Position",
        default_value: position_default,
        min: None,
        max: None,
    },
    ParamSpec {
        id: param_ids::SCALE,
        label: "Scale",
        default_value: scale_default,
        min: None,
        max: None,
    },
    ParamSpec {
        id: param_ids::ROTATION,
        label: "Rotation",
        default_value: zero,
        min: None,
        max: None,
    },
];

const TRANSFORM_PARAMS: [ParamSpec; 6] = [
    PLACEMENT_PARAMS[0],
    PLACEMENT_PARAMS[1],
    PLACEMENT_PARAMS[2],
    PLACEMENT_PARAMS[3],
    ParamSpec {
        id: param_ids::SKEW,
        label: "Skew",
        default_value: zero,
        min: None,
        max: None,
    },
    ParamSpec {
        id: param_ids::SKEW_AXIS,
        label: "Skew Axis",
        default_value: zero,
        min: None,
        max: None,
    },
];

const OPACITY_PARAMS: [ParamSpec; 1] = [ParamSpec {
    id: param_ids::OPACITY,
    label: "Opacity",
    default_value: one,
    min: Some(0.0),
    max: Some(1.0),
}];

const CROP_PARAMS: [ParamSpec; 4] = [
    ParamSpec {
        id: param_ids::LEFT,
        label: "Left",
        default_value: zero,
        min: Some(0.0),
        max: Some(1.0),
    },
    ParamSpec {
        id: param_ids::TOP,
        label: "Top",
        default_value: zero,
        min: Some(0.0),
        max: Some(1.0),
    },
    ParamSpec {
        id: param_ids::RIGHT,
        label: "Right",
        default_value: zero,
        min: Some(0.0),
        max: Some(1.0),
    },
    ParamSpec {
        id: param_ids::BOTTOM,
        label: "Bottom",
        default_value: zero,
        min: Some(0.0),
        max: Some(1.0),
    },
];

const SPECS: [EffectSpec; 8] = [
    EffectSpec {
        effect_id: effect_ids::MOTION,
        label: "Motion",
        kind: EffectKind::Filter,
        params: &PLACEMENT_PARAMS,
    },
    EffectSpec {
        effect_id: effect_ids::OPACITY,
        label: "Opacity",
        kind: EffectKind::Filter,
        params: &OPACITY_PARAMS,
    },
    EffectSpec {
        effect_id: effect_ids::CROP,
        label: "Crop",
        kind: EffectKind::Filter,
        params: &CROP_PARAMS,
    },
    EffectSpec {
        effect_id: effect_ids::TRANSFORM,
        label: "Transform",
        kind: EffectKind::Filter,
        params: &TRANSFORM_PARAMS,
    },
    EffectSpec {
        effect_id: effect_ids::CROSS_DISSOLVE,
        label: "Cross Dissolve",
        kind: EffectKind::Transition,
        params: &[],
    },
    EffectSpec {
        effect_id: effect_ids::DIP_TO_BLACK,
        label: "Dip to Black",
        kind: EffectKind::Transition,
        params: &[],
    },
    EffectSpec {
        effect_id: effect_ids::DIP_TO_WHITE,
        label: "Dip to White",
        kind: EffectKind::Transition,
        params: &[],
    },
    EffectSpec {
        effect_id: effect_ids::WIPE,
        label: "Wipe",
        kind: EffectKind::Transition,
        params: &[],
    },
];

pub fn specs() -> &'static [EffectSpec] {
    &SPECS
}

pub fn definition(effect_id: &str) -> Option<&'static EffectSpec> {
    specs().iter().find(|spec| spec.effect_id == effect_id)
}

pub fn motion_effect() -> EffectInstance {
    instance(effect_ids::MOTION)
}

pub fn opacity_effect() -> EffectInstance {
    instance(effect_ids::OPACITY)
}

pub fn crop_effect() -> EffectInstance {
    instance(effect_ids::CROP)
}

pub fn transform_effect() -> EffectInstance {
    instance(effect_ids::TRANSFORM)
}

/// Motion followed by opacity. Crop and transform are added by the user.
pub fn default_video_stack() -> Vec<EffectInstance> {
    vec![motion_effect(), opacity_effect()]
}

pub fn adjustment_clip(name: impl Into<String>, timeline_in: i64, timeline_out: i64) -> Clip {
    Clip {
        id: new_id("clip"),
        name: name.into(),
        media_id: None,
        nested_sequence_id: None,
        kind: ClipKind::Adjustment,
        timeline_in,
        timeline_out,
        source_in: 0,
        source_out: 0,
        speed: 1.0,
        reversed: false,
        enabled: true,
        linked_ids: Vec::new(),
        effects: vec![opacity_effect()],
        label_color: 0,
    }
}

fn instance(effect_id: &str) -> EffectInstance {
    let spec = definition(effect_id).expect("builtin effect id");
    EffectInstance {
        id: new_id("fx"),
        effect_id: effect_id.to_string(),
        enabled: true,
        params: spec
            .params
            .iter()
            .map(|param| Param {
                id: param.id.to_string(),
                value: (param.default_value)(),
                keyframes: Vec::new(),
            })
            .collect(),
    }
}
