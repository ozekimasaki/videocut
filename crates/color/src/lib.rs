//! Color effects for VideoCut.
//!
//! Wheels, RGB curves, HSL, and `.cube` LUTs are [`videocut_model::EffectInstance`]
//! values whose `effect_id` is one of the builtin color ids. Evaluate them with
//! [`apply_effect`] or [`apply_color_stack`]. The current parameter value is used;
//! keyframe sampling belongs to the effect engine.

#![forbid(unsafe_code)]

mod curves;
mod effect;
mod error;
mod hsl;
mod lut;
mod params;
mod rgb;
mod wheels;

pub use curves::{format_curve, parse_curve, Curve, CurvePoint, IDENTITY as CURVE_IDENTITY};
pub use effect::{
    apply_color_stack, apply_effect, color_wheels_effect, curves_effect, hsl_effect,
    is_color_effect, lut_effect,
};
pub use error::ColorError;
pub use hsl::{hsl_to_rgb, rgb_to_hsl};
pub use lut::{parse_cube, CubeLut, LutKind};
pub use rgb::Rgb;
pub use wheels::wheel_balance;
