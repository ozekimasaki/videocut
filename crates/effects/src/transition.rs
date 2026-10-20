use serde::{Deserialize, Serialize};
use videocut_model::{effect_ids, Transition, TransitionAlign};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DipColor {
    None,
    Black,
    White,
}

/// Mix of the outgoing clip, the incoming clip, and an optional dip color.
///
/// `wipe_edge` is the normalized x of a left-to-right wipe. Incoming occupies
/// `x < wipe_edge`. It is `0` for mixes that are not a wipe.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlendWeights {
    pub outgoing: f64,
    pub incoming: f64,
    pub dip: f64,
    pub dip_color: DipColor,
    pub wipe_edge: f64,
    pub recognized: bool,
}

/// Timeline window covered by a transition. `center_ticks` is the cut.
///
/// Center splits the duration around the cut (an odd duration keeps the extra
/// tick after the cut). Start runs forward from the cut. End finishes on the cut.
/// The window is half-open: `start <= tick < end`. A non-positive duration
/// covers no ticks.
pub fn transition_bounds(transition: &Transition) -> (i64, i64) {
    let duration = transition.duration_ticks.max(0);
    let cut = transition.center_ticks;
    match transition.alignment {
        TransitionAlign::Center => {
            let start = cut.saturating_sub(duration / 2);
            (start, start.saturating_add(duration))
        }
        TransitionAlign::Start => (cut, cut.saturating_add(duration)),
        TransitionAlign::End => (cut.saturating_sub(duration), cut),
    }
}

pub fn transition_progress(transition: &Transition, tick: i64) -> Option<f64> {
    let (start, end) = transition_bounds(transition);
    if tick < start || tick >= end {
        return None;
    }
    let duration = transition.duration_ticks.max(0);
    if duration == 0 {
        return None;
    }
    Some((tick - start) as f64 / duration as f64)
}

pub fn blend_weights(effect_id: &str, progress: f64) -> BlendWeights {
    let progress = progress.clamp(0.0, 1.0);
    match effect_id {
        effect_ids::CROSS_DISSOLVE => mix(progress, true),
        effect_ids::DIP_TO_BLACK => dip(progress, DipColor::Black),
        effect_ids::DIP_TO_WHITE => dip(progress, DipColor::White),
        effect_ids::WIPE => BlendWeights {
            outgoing: 1.0 - progress,
            incoming: progress,
            dip: 0.0,
            dip_color: DipColor::None,
            wipe_edge: progress,
            recognized: true,
        },
        _ => mix(progress, false),
    }
}

fn mix(progress: f64, recognized: bool) -> BlendWeights {
    BlendWeights {
        outgoing: 1.0 - progress,
        incoming: progress,
        dip: 0.0,
        dip_color: DipColor::None,
        wipe_edge: 0.0,
        recognized,
    }
}

fn dip(progress: f64, color: DipColor) -> BlendWeights {
    let outgoing = (1.0 - 2.0 * progress).max(0.0);
    let incoming = (2.0 * progress - 1.0).max(0.0);
    BlendWeights {
        outgoing,
        incoming,
        dip: (1.0 - outgoing - incoming).clamp(0.0, 1.0),
        dip_color: color,
        wipe_edge: 0.0,
        recognized: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use videocut_model::effect_ids;

    fn transition(effect_id: &str, alignment: TransitionAlign) -> Transition {
        Transition {
            id: "tr-1".into(),
            effect_id: effect_id.into(),
            center_ticks: 1000,
            duration_ticks: 200,
            alignment,
        }
    }

    fn assert_close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn alignment_windows() {
        assert_eq!(
            transition_bounds(&transition(
                effect_ids::CROSS_DISSOLVE,
                TransitionAlign::Center
            )),
            (900, 1100)
        );
        assert_eq!(
            transition_bounds(&transition(
                effect_ids::CROSS_DISSOLVE,
                TransitionAlign::Start
            )),
            (1000, 1200)
        );
        assert_eq!(
            transition_bounds(&transition(
                effect_ids::CROSS_DISSOLVE,
                TransitionAlign::End
            )),
            (800, 1000)
        );
        let mut zero = transition(effect_ids::WIPE, TransitionAlign::Center);
        zero.duration_ticks = 0;
        assert_eq!(transition_progress(&zero, 1000), None);
    }

    #[test]
    fn cross_dissolve_dip_and_wipe() {
        let cross = blend_weights(effect_ids::CROSS_DISSOLVE, 0.25);
        assert_close(cross.outgoing, 0.75);
        assert_close(cross.incoming, 0.25);
        assert_close(cross.dip, 0.0);
        assert_eq!(cross.dip_color, DipColor::None);

        let black = blend_weights(effect_ids::DIP_TO_BLACK, 0.25);
        assert_close(black.outgoing, 0.5);
        assert_close(black.incoming, 0.0);
        assert_close(black.dip, 0.5);
        assert_eq!(black.dip_color, DipColor::Black);

        let mid = blend_weights(effect_ids::DIP_TO_BLACK, 0.5);
        assert_close(mid.outgoing, 0.0);
        assert_close(mid.incoming, 0.0);
        assert_close(mid.dip, 1.0);

        let white = blend_weights(effect_ids::DIP_TO_WHITE, 0.75);
        assert_close(white.outgoing, 0.0);
        assert_close(white.incoming, 0.5);
        assert_close(white.dip, 0.5);
        assert_eq!(white.dip_color, DipColor::White);

        let wipe = blend_weights(effect_ids::WIPE, 0.4);
        assert_close(wipe.wipe_edge, 0.4);
        assert_close(wipe.incoming, 0.4);
        assert!(wipe.recognized);

        let unknown = blend_weights("custom.iris", 0.4);
        assert!(!unknown.recognized);
        assert_close(unknown.incoming, 0.4);
        assert_close(unknown.wipe_edge, 0.0);
    }

    #[test]
    fn progress_is_half_open() {
        let transition = transition(effect_ids::CROSS_DISSOLVE, TransitionAlign::Center);
        assert_eq!(transition_progress(&transition, 899), None);
        assert_close(transition_progress(&transition, 900).unwrap(), 0.0);
        assert_close(transition_progress(&transition, 950).unwrap(), 0.25);
        assert_eq!(transition_progress(&transition, 1100), None);
    }
}
