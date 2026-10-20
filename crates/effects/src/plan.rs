use std::collections::HashSet;

use videocut_model::{Clip, ClipKind, Sequence, Track, Transition};

use crate::stack::{evaluate_stack, local_ticks, StackEvaluation};
use crate::transition::{blend_weights, transition_bounds, transition_progress, BlendWeights};

/// Bottom-to-top composite at one timeline tick.
///
/// An [`RenderItem::Adjustment`] applies to the buffer built so far, which is
/// every visible layer on lower tracks. Later items sit above that adjustment.
/// A transition replaces its outgoing and incoming clips for the ticks it covers.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderPlan<'a> {
    pub items: Vec<RenderItem<'a>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderItem<'a> {
    Clip {
        track_index: u32,
        track_id: &'a str,
        clip: &'a Clip,
        evaluation: StackEvaluation,
    },
    Transition {
        track_index: u32,
        track_id: &'a str,
        transition: &'a Transition,
        outgoing: &'a Clip,
        incoming: &'a Clip,
        outgoing_evaluation: StackEvaluation,
        incoming_evaluation: StackEvaluation,
        progress: f64,
        blend: BlendWeights,
    },
    Adjustment {
        track_index: u32,
        track_id: &'a str,
        clip: &'a Clip,
        evaluation: StackEvaluation,
    },
}

pub fn render_plan(sequence: &Sequence, timeline_ticks: i64) -> RenderPlan<'_> {
    let mut tracks: Vec<&Track> = sequence
        .video_tracks
        .iter()
        .filter(|track| !track.hidden)
        .collect();
    tracks.sort_by_key(|track| track.index);
    let mut items = Vec::new();
    for track in tracks {
        items.extend(track_items(track, timeline_ticks));
    }
    RenderPlan { items }
}

fn track_items<'a>(track: &'a Track, tick: i64) -> Vec<RenderItem<'a>> {
    let mut items = Vec::new();
    let mut consumed: HashSet<&str> = HashSet::new();

    if let Some(transition) = active_transition(track, tick) {
        if let Some((outgoing, incoming)) = pair_for_transition(track, transition) {
            consumed.insert(outgoing.id.as_str());
            consumed.insert(incoming.id.as_str());
            let progress = transition_progress(transition, tick).unwrap_or(0.0);
            items.push(RenderItem::Transition {
                track_index: track.index,
                track_id: track.id.as_str(),
                transition,
                outgoing,
                incoming,
                outgoing_evaluation: evaluate_stack(&outgoing.effects, local_ticks(outgoing, tick)),
                incoming_evaluation: evaluate_stack(&incoming.effects, local_ticks(incoming, tick)),
                progress,
                blend: blend_weights(&transition.effect_id, progress),
            });
        }
    }

    let mut clips: Vec<&Clip> = track
        .clips
        .iter()
        .filter(|clip| clip.enabled && contains(clip, tick))
        .collect();
    clips.sort_by_key(|clip| clip.timeline_in);
    for clip in clips {
        if clip.kind == ClipKind::Adjustment {
            items.push(RenderItem::Adjustment {
                track_index: track.index,
                track_id: track.id.as_str(),
                clip,
                evaluation: evaluate_stack(&clip.effects, local_ticks(clip, tick)),
            });
            continue;
        }
        if consumed.contains(clip.id.as_str()) {
            continue;
        }
        items.push(RenderItem::Clip {
            track_index: track.index,
            track_id: track.id.as_str(),
            clip,
            evaluation: evaluate_stack(&clip.effects, local_ticks(clip, tick)),
        });
    }
    items
}

fn active_transition(track: &Track, tick: i64) -> Option<&Transition> {
    track.transitions.iter().find(|transition| {
        let (start, end) = transition_bounds(transition);
        start <= tick && tick < end
    })
}

fn pair_for_transition<'a>(
    track: &'a Track,
    transition: &Transition,
) -> Option<(&'a Clip, &'a Clip)> {
    let (start, end) = transition_bounds(transition);
    let cut = transition.center_ticks;
    let mut clips: Vec<&Clip> = track
        .clips
        .iter()
        .filter(|clip| clip.enabled && clip.kind != ClipKind::Adjustment)
        .collect();
    clips.sort_by_key(|clip| clip.timeline_in);

    let mut best: Option<(&Clip, &Clip, i64)> = None;
    for pair in clips.windows(2) {
        let (left, right) = (pair[0], pair[1]);
        if left.timeline_out <= start || right.timeline_in >= end {
            continue;
        }
        if left.timeline_in >= cut || right.timeline_out <= cut {
            continue;
        }
        let distance = (meeting_tick(left, right) - cut).abs();
        let replace = best.is_none_or(|(_, _, closest)| distance < closest);
        if replace {
            best = Some((left, right, distance));
        }
    }
    best.map(|(left, right, _)| (left, right))
}

/// Where two clips meet: the midpoint of their overlap, or the midpoint of a gap.
fn meeting_tick(left: &Clip, right: &Clip) -> i64 {
    if right.timeline_in >= left.timeline_out {
        let gap = right.timeline_in.saturating_sub(left.timeline_out);
        left.timeline_out.saturating_add(gap / 2)
    } else {
        let overlap = left.timeline_out.saturating_sub(right.timeline_in);
        right.timeline_in.saturating_add(overlap / 2)
    }
}

fn contains(clip: &Clip, tick: i64) -> bool {
    clip.timeline_in <= tick && tick < clip.timeline_out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{adjustment_clip, opacity_effect};
    use videocut_model::{
        effect_ids, Clip, Interp, Keyframe, ParamValue, Sequence, SequenceSettings, Track,
        TrackKind, Transition, TransitionAlign, TICKS_PER_SECOND,
    };

    fn sec(seconds: i64) -> i64 {
        seconds * TICKS_PER_SECOND
    }

    fn video_clip(name: &str, start: i64, end: i64) -> Clip {
        let mut clip = Clip::media(name, "media-1");
        clip.timeline_in = start;
        clip.timeline_out = end;
        clip.source_out = end - start;
        clip.effects = vec![opacity_effect()];
        clip
    }

    fn track_with(index: u32, clips: Vec<Clip>, transitions: Vec<Transition>) -> Track {
        let mut track = Track::new(index, TrackKind::Video);
        track.clips = clips;
        track.transitions = transitions;
        track
    }

    #[test]
    fn adjustment_sits_above_lower_tracks_only() {
        let mut adjustment = adjustment_clip("adj", sec(5), sec(15));
        adjustment.effects[0].params[0].value = ParamValue::Float(0.5);
        let mut sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
        sequence.video_tracks.push(track_with(
            2,
            vec![video_clip("top", 0, sec(20))],
            Vec::new(),
        ));
        sequence
            .video_tracks
            .push(track_with(0, vec![video_clip("a", 0, sec(20))], Vec::new()));
        sequence
            .video_tracks
            .push(track_with(1, vec![adjustment], Vec::new()));

        let plan = render_plan(&sequence, sec(10));
        assert_eq!(plan.items.len(), 3);
        assert!(matches!(plan.items[0], RenderItem::Clip { clip, .. } if clip.name == "a"));
        match &plan.items[1] {
            RenderItem::Adjustment { evaluation, .. } => {
                assert!((evaluation.opacity - 0.5).abs() < 1e-9);
            }
            other => panic!("expected adjustment, got {other:?}"),
        }
        assert!(matches!(plan.items[2], RenderItem::Clip { clip, .. } if clip.name == "top"));

        let early = render_plan(&sequence, sec(1));
        assert!(early
            .items
            .iter()
            .all(|item| !matches!(item, RenderItem::Adjustment { .. })));
    }

    #[test]
    fn transition_replaces_the_pair_and_uses_local_keys() {
        let mut outgoing = video_clip("out", sec(0), sec(11));
        outgoing.effects[0].params[0].keyframes = vec![
            Keyframe {
                time_ticks: 0,
                value: ParamValue::Float(0.0),
                interp: Interp::Linear,
            },
            Keyframe {
                time_ticks: sec(20),
                value: ParamValue::Float(1.0),
                interp: Interp::Linear,
            },
        ];
        let incoming = video_clip("in", sec(9), sec(20));
        let transition = Transition {
            id: "tr".into(),
            effect_id: effect_ids::CROSS_DISSOLVE.into(),
            center_ticks: sec(10),
            duration_ticks: sec(2),
            alignment: TransitionAlign::Center,
        };
        let mut sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
        sequence
            .video_tracks
            .push(track_with(0, vec![outgoing, incoming], vec![transition]));

        let plan = render_plan(&sequence, sec(10));
        assert_eq!(plan.items.len(), 1);
        match &plan.items[0] {
            RenderItem::Transition {
                outgoing,
                incoming,
                progress,
                blend,
                outgoing_evaluation,
                ..
            } => {
                assert_eq!(outgoing.name, "out");
                assert_eq!(incoming.name, "in");
                assert!((progress - 0.5).abs() < 1e-9);
                assert!((blend.outgoing - 0.5).abs() < 1e-9);
                assert!((blend.incoming - 0.5).abs() < 1e-9);
                assert!((outgoing_evaluation.opacity - 0.5).abs() < 1e-9);
            }
            other => panic!("expected transition, got {other:?}"),
        }
    }

    #[test]
    fn abutted_clips_pair_across_the_cut() {
        let transition = Transition {
            id: "tr".into(),
            effect_id: effect_ids::WIPE.into(),
            center_ticks: sec(10),
            duration_ticks: sec(2),
            alignment: TransitionAlign::Center,
        };
        let mut sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
        sequence.video_tracks.push(track_with(
            0,
            vec![
                video_clip("out", sec(0), sec(10)),
                video_clip("in", sec(10), sec(20)),
            ],
            vec![transition],
        ));
        let plan = render_plan(&sequence, sec(10));
        match &plan.items[0] {
            RenderItem::Transition {
                outgoing,
                incoming,
                blend,
                ..
            } => {
                assert_eq!(outgoing.name, "out");
                assert_eq!(incoming.name, "in");
                assert!((blend.wipe_edge - 0.5).abs() < 1e-9);
            }
            other => panic!("expected transition, got {other:?}"),
        }
    }

    #[test]
    fn hidden_and_disabled_layers_drop_out() {
        let mut hidden = track_with(0, vec![video_clip("hidden", 0, sec(5))], Vec::new());
        hidden.hidden = true;
        let mut disabled = video_clip("off", 0, sec(5));
        disabled.enabled = false;
        let mut sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
        sequence.video_tracks.push(hidden);
        sequence.video_tracks.push(track_with(
            1,
            vec![disabled, video_clip("on", 0, sec(5))],
            Vec::new(),
        ));
        let plan = render_plan(&sequence, sec(1));
        assert_eq!(plan.items.len(), 1);
        assert!(matches!(plan.items[0], RenderItem::Clip { clip, .. } if clip.name == "on"));
    }
}
