//! Canonical editorial view used by every exporter.
//!
//! Clips on a track do not overlap. A transition sits on the cut where the
//! outgoing clip's `timeline_out` meets the incoming clip's `timeline_in`.
//! `in_offset` reaches back into the outgoing clip and `out_offset` reaches
//! forward into the incoming clip. Formats that store a dissolve as overlapping
//! record times (FCPXML, CMX) derive that overlap from these offsets and
//! restore this shape on import.

use videocut_model::{
    effect_ids, AudioStreamInfo, Clip, ClipKind, Marker, Project, Sequence, SequenceSettings,
    Track, TrackKind, Transition, TransitionAlign, VideoStreamInfo,
};

use crate::timeutil::scale_ticks;

pub struct PreparedProject {
    pub name: String,
    pub sequences: Vec<PreparedSequence>,
    pub warnings: Vec<String>,
}

pub struct PreparedSequence {
    pub name: String,
    pub settings: SequenceSettings,
    pub video_tracks: Vec<PreparedTrack>,
    pub audio_tracks: Vec<PreparedTrack>,
    pub markers: Vec<Marker>,
}

pub struct PreparedTrack {
    pub kind: TrackKind,
    pub index: u32,
    pub muted: bool,
    pub hidden: bool,
    pub locked: bool,
    pub sync_lock: bool,
    pub targeted: bool,
    pub clips: Vec<PreparedClip>,
    pub joins: Vec<PreparedJoin>,
}

#[derive(Clone)]
pub struct PreparedClip {
    pub name: String,
    pub media_name: String,
    pub path: String,
    pub media_duration: i64,
    pub video: Option<VideoStreamInfo>,
    pub audio: Option<AudioStreamInfo>,
    pub timeline_in: i64,
    pub timeline_out: i64,
    pub source_in: i64,
    pub source_out: i64,
    pub speed: f64,
    pub reversed: bool,
    pub enabled: bool,
    pub kind: ClipKind,
    pub label_color: u8,
}

#[derive(Clone)]
pub struct PreparedJoin {
    pub after: usize,
    pub effect_id: String,
    pub alignment: TransitionAlign,
    pub duration_ticks: i64,
    pub in_offset: i64,
    pub out_offset: i64,
}

pub fn prepare(project: &Project) -> PreparedProject {
    let mut warnings = Vec::new();
    let sequences = project
        .sequences
        .iter()
        .map(|sequence| prepare_sequence(project, sequence, &mut warnings))
        .collect();
    PreparedProject {
        name: project.name.clone(),
        sequences,
        warnings,
    }
}

fn prepare_sequence(
    project: &Project,
    sequence: &Sequence,
    warnings: &mut Vec<String>,
) -> PreparedSequence {
    let video_tracks = sequence
        .video_tracks
        .iter()
        .map(|track| prepare_track(project, track, warnings))
        .collect();
    let audio_tracks = sequence
        .audio_tracks
        .iter()
        .map(|track| prepare_track(project, track, warnings))
        .collect();
    PreparedSequence {
        name: sequence.name.clone(),
        settings: sequence.settings.clone(),
        video_tracks,
        audio_tracks,
        markers: sequence.markers.clone(),
    }
}

fn prepare_track(project: &Project, track: &Track, warnings: &mut Vec<String>) -> PreparedTrack {
    let mut clips: Vec<Clip> = track.clips.clone();
    clips.sort_by(|a, b| {
        a.timeline_in
            .cmp(&b.timeline_in)
            .then(a.timeline_out.cmp(&b.timeline_out))
    });
    clips.retain(|clip| {
        if clip.timeline_out <= clip.timeline_in {
            warnings.push(format!(
                "skipped '{}' because its timeline range is empty",
                clip.name
            ));
            false
        } else {
            true
        }
    });

    let mut transitions = track.transitions.clone();
    let mut joins: Vec<PreparedJoin> = Vec::new();
    let mut index = 0;
    while index + 1 < clips.len() {
        if clips[index + 1].timeline_in < clips[index].timeline_out {
            let overlap_start = clips[index + 1].timeline_in;
            let overlap_end = clips[index].timeline_out.min(clips[index + 1].timeline_out);
            let matched = take_match(&mut transitions, &clips[index], &clips[index + 1]);
            let (cut, join) = if let Some(transition) = matched {
                let cut = edit_cut(&transition, overlap_start, overlap_end);
                let (in_offset, out_offset) = offsets_for(&transition, cut);
                (cut, Some((transition, in_offset, out_offset)))
            } else {
                let overlap = (overlap_end - overlap_start).max(0);
                let cut = overlap_start + overlap / 2;
                warnings.push(format!(
                    "overlapping clips '{}' and '{}' were exported as a cross dissolve",
                    clips[index].name,
                    clips[index + 1].name
                ));
                let transition = Transition {
                    id: String::new(),
                    effect_id: effect_ids::CROSS_DISSOLVE.to_string(),
                    center_ticks: cut,
                    duration_ticks: overlap,
                    alignment: TransitionAlign::Center,
                };
                let (in_offset, out_offset) = offsets_for(&transition, cut);
                (cut, Some((transition, in_offset, out_offset)))
            };
            trim_end(&mut clips[index], cut);
            trim_start(&mut clips[index + 1], cut);
            if let Some((transition, in_offset, out_offset)) = join {
                push_join(&mut joins, index, &transition, in_offset, out_offset);
            }
        }
        index += 1;
    }

    for index in 0..clips.len().saturating_sub(1) {
        if clips[index].timeline_out != clips[index + 1].timeline_in {
            continue;
        }
        if joins.iter().any(|join| join.after == index) {
            continue;
        }
        let cut = clips[index].timeline_out;
        let Some(position) = best_match(&transitions, &clips[index], &clips[index + 1]) else {
            continue;
        };
        let transition = transitions.remove(position);
        let (in_offset, out_offset) = offsets_for(&transition, cut);
        push_join(&mut joins, index, &transition, in_offset, out_offset);
    }
    for transition in &transitions {
        warnings.push(format!(
            "transition '{}' is not on a cut and was omitted",
            transition.effect_id
        ));
    }

    let clips = clips
        .into_iter()
        .map(|clip| prepared_clip(project, clip))
        .collect();
    PreparedTrack {
        kind: track.kind,
        index: track.index,
        muted: track.muted,
        hidden: track.hidden,
        locked: track.locked,
        sync_lock: track.sync_lock,
        targeted: track.targeted,
        clips,
        joins,
    }
}

fn push_join(
    joins: &mut Vec<PreparedJoin>,
    after: usize,
    transition: &Transition,
    in_offset: i64,
    out_offset: i64,
) {
    let in_offset = in_offset.max(0);
    let out_offset = out_offset.max(0);
    let duration = in_offset + out_offset;
    if duration <= 0 {
        return;
    }
    joins.push(PreparedJoin {
        after,
        effect_id: transition.effect_id.clone(),
        alignment: transition.alignment,
        duration_ticks: duration,
        in_offset,
        out_offset,
    });
}

fn prepared_clip(project: &Project, clip: Clip) -> PreparedClip {
    let media = clip.media_id.as_deref().and_then(|id| project.media(id));
    PreparedClip {
        name: clip.name,
        media_name: media.map(|media| media.name.clone()).unwrap_or_default(),
        path: media.map(|media| media.path.clone()).unwrap_or_default(),
        media_duration: media.map(|media| media.duration_ticks).unwrap_or(0),
        video: media.and_then(|media| media.video.clone()),
        audio: media.and_then(|media| media.audio.clone()),
        timeline_in: clip.timeline_in,
        timeline_out: clip.timeline_out,
        source_in: clip.source_in,
        source_out: clip.source_out,
        speed: clip.speed,
        reversed: clip.reversed,
        enabled: clip.enabled,
        kind: clip.kind,
        label_color: clip.label_color,
    }
}

pub fn head_tail(track: &PreparedTrack, index: usize) -> (i64, i64) {
    let head = track
        .joins
        .iter()
        .find(|join| join.after + 1 == index)
        .map(|join| join.in_offset)
        .unwrap_or(0);
    let tail = track
        .joins
        .iter()
        .find(|join| join.after == index)
        .map(|join| join.out_offset)
        .unwrap_or(0);
    (head, tail)
}

pub fn join_after<'a>(track: &'a PreparedTrack, index: usize) -> Option<&'a PreparedJoin> {
    track.joins.iter().find(|join| join.after == index)
}

pub fn transition_bounds(transition: &Transition) -> (i64, i64) {
    let start = transition.center_ticks - transition.duration_ticks / 2;
    let end = start + transition.duration_ticks.max(0);
    (start, end)
}

fn edit_cut(transition: &Transition, overlap_start: i64, overlap_end: i64) -> i64 {
    let (start, end) = transition_bounds(transition);
    let cut = match transition.alignment {
        TransitionAlign::Start => start,
        TransitionAlign::End => end,
        TransitionAlign::Center => transition.center_ticks,
    };
    cut.clamp(overlap_start, overlap_end.max(overlap_start))
}

fn offsets_for(transition: &Transition, cut: i64) -> (i64, i64) {
    let (start, end) = transition_bounds(transition);
    match transition.alignment {
        TransitionAlign::Start => (0, transition.duration_ticks.max(0)),
        TransitionAlign::End => (transition.duration_ticks.max(0), 0),
        TransitionAlign::Center => ((cut - start).max(0), (end - cut).max(0)),
    }
}

fn covers(transition: &Transition, outgoing: &Clip, incoming: &Clip) -> bool {
    let (start, end) = transition_bounds(transition);
    if end <= start {
        return false;
    }
    if incoming.timeline_in < outgoing.timeline_out {
        let overlap_start = incoming.timeline_in;
        let overlap_end = outgoing.timeline_out.min(incoming.timeline_out);
        return start < overlap_end && end > overlap_start;
    }
    if outgoing.timeline_out == incoming.timeline_in {
        let cut = outgoing.timeline_out;
        return start <= cut && end >= cut;
    }
    false
}

fn best_match(transitions: &[Transition], outgoing: &Clip, incoming: &Clip) -> Option<usize> {
    let point = if incoming.timeline_in == outgoing.timeline_out {
        outgoing.timeline_out
    } else if incoming.timeline_in < outgoing.timeline_out {
        let overlap_start = incoming.timeline_in;
        let overlap_end = outgoing.timeline_out.min(incoming.timeline_out);
        overlap_start + (overlap_end - overlap_start) / 2
    } else {
        return None;
    };
    let mut best: Option<(usize, i64)> = None;
    for (index, transition) in transitions.iter().enumerate() {
        if !covers(transition, outgoing, incoming) {
            continue;
        }
        let distance = (transition.center_ticks - point).abs();
        if best.map(|(_, current)| distance < current).unwrap_or(true) {
            best = Some((index, distance));
        }
    }
    best.map(|(index, _)| index)
}

fn take_match(
    transitions: &mut Vec<Transition>,
    outgoing: &Clip,
    incoming: &Clip,
) -> Option<Transition> {
    let index = best_match(transitions, outgoing, incoming)?;
    Some(transitions.remove(index))
}

fn trim_end(clip: &mut Clip, new_out: i64) {
    if new_out >= clip.timeline_out {
        return;
    }
    let delta = clip.timeline_out - new_out;
    let source_delta = scale_ticks(delta, clip.speed.abs());
    clip.timeline_out = new_out;
    if clip.reversed {
        clip.source_in += source_delta;
    } else {
        clip.source_out -= source_delta;
    }
}

fn trim_start(clip: &mut Clip, new_in: i64) {
    if new_in <= clip.timeline_in {
        return;
    }
    let delta = new_in - clip.timeline_in;
    let source_delta = scale_ticks(delta, clip.speed.abs());
    clip.timeline_in = new_in;
    if clip.reversed {
        clip.source_out -= source_delta;
    } else {
        clip.source_in += source_delta;
    }
}
