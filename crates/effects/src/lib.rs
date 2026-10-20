//! Effect stack, keyframe evaluation, transitions, and adjustment layers.
//!
//! Keyframe `time_ticks` are clip-local: `0` is the clip's `timeline_in`.
//! Convert a timeline position with [`local_ticks`] before evaluating a clip.
//! Transition timing stays on the timeline.
//!
//! Bezier segments use a unit cubic with control points `(0,0), (0,0), (1,1), (1,1)`
//! (flat tangents). The model stores no handles, so this ease is the whole curve.
//! Opacity is `0..=1`, scale is a factor (`1` = native size), and crop edges are
//! fractions of the frame. Position and anchor are normalized, with `(0.5, 0.5)`
//! at the frame center.

mod affine;
mod catalog;
mod eval;
mod plan;
mod stack;
mod transition;

pub use affine::Affine;
pub use catalog::{
    adjustment_clip, crop_effect, default_video_stack, definition, motion_effect, opacity_effect,
    specs, transform_effect, EffectKind, EffectSpec, ParamSpec,
};
pub use eval::{add_keyframe, bezier_ease, evaluate_param, remove_keyframe};
pub use plan::{render_plan, RenderItem, RenderPlan};
pub use stack::{
    evaluate_stack, insert_effect, local_ticks, move_effect, remove_effect, set_enabled,
    set_param_value, Crop, EffectError, EvaluatedEffect, EvaluatedParam, StackEvaluation,
};
pub use transition::{
    blend_weights, transition_bounds, transition_progress, BlendWeights, DipColor,
};

pub mod param_ids {
    pub const ANCHOR: &str = "anchor";
    pub const POSITION: &str = "position";
    pub const SCALE: &str = "scale";
    pub const ROTATION: &str = "rotation";
    pub const OPACITY: &str = "opacity";
    pub const LEFT: &str = "left";
    pub const TOP: &str = "top";
    pub const RIGHT: &str = "right";
    pub const BOTTOM: &str = "bottom";
    pub const SKEW: &str = "skew";
    pub const SKEW_AXIS: &str = "skewAxis";
}
