use videocut_model::{effect_ids, Clip, MediaRef, Track, Transition, TransitionAlign};

use std::path::Path;

use videocut_model::ClipKind;

use crate::error::MediaError;
use crate::probe::is_still_extension;
use crate::time::scale_ticks;

/// One non-overlapping piece of a single track after cuts and dissolves.
#[derive(Debug, Clone)]
pub(crate) struct Span {
    pub timeline_in: i64,
    pub timeline_out: i64,
    pub body: SpanBody,
}

#[derive(Debug, Clone)]
pub(crate) enum SpanBody {
    Gap,
    Clip(ClipSlice),
    Dissolve { left: ClipSlice, right: ClipSlice },
}

#[derive(Debug, Clone)]
pub(crate) struct ClipSlice {
    pub input_index: usize,
    pub source_in: i64,
    pub source_out: i64,
    pub speed: f64,
    pub reversed: bool,
    pub still: bool,
    pub has_picture: bool,
    pub has_sound: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedClip {
    order: usize,
    timeline_in: i64,
    timeline_out: i64,
    kind: PreparedKind,
}

#[derive(Debug, Clone)]
enum PreparedKind {
    Media(MediaClip),
    Matte,
}

#[derive(Debug, Clone)]
struct MediaClip {
    input_index: usize,
    speed: f64,
    reversed: bool,
    still: bool,
    has_picture: bool,
    has_sound: bool,
    clip_timeline_in: i64,
    clip_source_in: i64,
    clip_source_out: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct RegisteredInput {
    pub path: String,
    pub still: bool,
    pub still_duration_ticks: i64,
}

#[derive(Debug, Default)]
pub(crate) struct InputTable {
    pub inputs: Vec<RegisteredInput>,
}

impl InputTable {
    pub fn index_for(&mut self, media: &MediaRef) -> usize {
        if let Some(index) = self
            .inputs
            .iter()
            .position(|input| input.path == media.path)
        {
            return index;
        }
        let still = is_still_extension(Path::new(&media.path))
            || (media.video.is_some() && media.audio.is_none() && media.duration_ticks == 0);
        self.inputs.push(RegisteredInput {
            path: media.path.clone(),
            still,
            still_duration_ticks: 0,
        });
        self.inputs.len() - 1
    }
}

pub(crate) fn prepare_video_track(
    track: &Track,
    media_of: impl Fn(&str) -> Result<MediaRef, MediaError>,
    inputs: &mut InputTable,
    warnings: &mut Vec<String>,
) -> Result<Vec<PreparedClip>, MediaError> {
    prepare_track(track, true, media_of, inputs, warnings)
}

pub(crate) fn prepare_audio_track(
    track: &Track,
    media_of: impl Fn(&str) -> Result<MediaRef, MediaError>,
    inputs: &mut InputTable,
    warnings: &mut Vec<String>,
) -> Result<Vec<PreparedClip>, MediaError> {
    prepare_track(track, false, media_of, inputs, warnings)
}

fn prepare_track(
    track: &Track,
    video: bool,
    media_of: impl Fn(&str) -> Result<MediaRef, MediaError>,
    inputs: &mut InputTable,
    warnings: &mut Vec<String>,
) -> Result<Vec<PreparedClip>, MediaError> {
    let mut prepared = Vec::new();
    for (order, clip) in track.clips.iter().enumerate() {
        if !clip.enabled || clip.timeline_out <= clip.timeline_in {
            continue;
        }
        match clip.kind {
            ClipKind::Media => {
                let Some(media_id) = clip.media_id.as_deref() else {
                    return Err(MediaError::Invalid(format!(
                        "clip '{}' has no media id",
                        clip.name
                    )));
                };
                let media = media_of(media_id)?;
                prepared.push(prepare_media(order, clip, &media, inputs));
            }
            ClipKind::ColorMatte if video => {
                prepared.push(PreparedClip {
                    order,
                    timeline_in: clip.timeline_in,
                    timeline_out: clip.timeline_out,
                    kind: PreparedKind::Matte,
                });
            }
            ClipKind::ColorMatte
            | ClipKind::Nested
            | ClipKind::Adjustment
            | ClipKind::Title
            | ClipKind::Caption => {
                warnings.push(format!(
                    "clip '{}' ({}) is not burned into the export",
                    clip.name,
                    clip_kind_name(clip.kind)
                ));
            }
        }
    }
    prepared.sort_by(|left, right| {
        left.timeline_in
            .cmp(&right.timeline_in)
            .then(left.timeline_out.cmp(&right.timeline_out))
            .then(left.order.cmp(&right.order))
    });
    Ok(prepared)
}

fn prepare_media(
    order: usize,
    clip: &Clip,
    media: &MediaRef,
    inputs: &mut InputTable,
) -> PreparedClip {
    let mut speed = if clip.speed.is_finite() {
        clip.speed
    } else {
        1.0
    };
    let mut reversed = clip.reversed;
    if speed < 0.0 {
        reversed = !reversed;
        speed = speed.abs();
    }
    if speed < 1e-6 {
        speed = 1.0;
    }
    let still = is_still_extension(Path::new(&media.path))
        || (media.video.is_some() && media.audio.is_none() && media.duration_ticks == 0);
    PreparedClip {
        order,
        timeline_in: clip.timeline_in,
        timeline_out: clip.timeline_out,
        kind: PreparedKind::Media(MediaClip {
            input_index: inputs.index_for(media),
            speed,
            reversed,
            still,
            has_picture: media.video.is_some(),
            has_sound: media.audio.is_some() && !still,
            clip_timeline_in: clip.timeline_in,
            clip_source_in: clip.source_in,
            clip_source_out: clip.source_out,
        }),
    }
}

pub(crate) fn build_spans(
    clips: &[PreparedClip],
    transitions: &[Transition],
    range_in: i64,
    range_out: i64,
    warnings: &mut Vec<String>,
) -> Result<Vec<Span>, MediaError> {
    if range_out <= range_in {
        return Err(MediaError::EmptyRange);
    }
    let mut spans = Vec::new();
    let mut cursor = range_in;
    let guard_max = clips.len().saturating_mul(8).saturating_add(4);
    let mut guard = 0_usize;
    while cursor < range_out {
        guard += 1;
        if guard > guard_max {
            return Err(MediaError::Invalid("timeline walk did not advance".into()));
        }
        let before = cursor;
        let Some(top) = top_clip(clips, cursor) else {
            let end = next_boundary(clips, cursor, range_out);
            spans.push(Span::gap(cursor, end));
            cursor = end;
            continue;
        };
        if let Some(incoming) = incoming_clip(clips, top, cursor, range_out) {
            let overlap_in = clips[incoming].timeline_in;
            let overlap_out = clips[top]
                .timeline_out
                .min(clips[incoming].timeline_out)
                .min(range_out);
            if overlap_out > overlap_in {
                if let Some(transition) = find_transition(transitions, overlap_in, overlap_out) {
                    if transition.effect_id == effect_ids::CROSS_DISSOLVE
                        && clips[top].is_media()
                        && clips[incoming].is_media()
                    {
                        let (window_in, window_out) = transition_window(transition);
                        let dissolve_in = window_in.max(overlap_in).max(cursor);
                        let dissolve_out = window_out.min(overlap_out);
                        if dissolve_out > dissolve_in {
                            if cursor < dissolve_in {
                                spans.push(clips[top].slice(cursor, dissolve_in));
                            }
                            spans.push(Span::dissolve(
                                &clips[top],
                                &clips[incoming],
                                dissolve_in,
                                dissolve_out,
                            ));
                            cursor = dissolve_out;
                            continue;
                        }
                    } else if transition.effect_id != effect_ids::CROSS_DISSOLVE {
                        warnings.push(format!(
                            "transition '{}' is exported as a cut",
                            transition.effect_id
                        ));
                    } else {
                        warnings.push(
                            "cross dissolve was exported as a cut because one side is not media"
                                .into(),
                        );
                    }
                }
            }
            if overlap_in > cursor {
                spans.push(clips[top].slice(cursor, overlap_in.min(range_out)));
                cursor = overlap_in.min(range_out);
                continue;
            }
        }
        let end = next_boundary(clips, cursor, range_out);
        spans.push(clips[top].slice(cursor, end));
        cursor = end;
        if cursor <= before {
            return Err(MediaError::Invalid("timeline walk did not advance".into()));
        }
    }
    Ok(merge_adjacent(spans))
}

impl Span {
    fn gap(timeline_in: i64, timeline_out: i64) -> Self {
        Self {
            timeline_in,
            timeline_out,
            body: SpanBody::Gap,
        }
    }

    fn dissolve(
        left: &PreparedClip,
        right: &PreparedClip,
        timeline_in: i64,
        timeline_out: i64,
    ) -> Self {
        Self {
            timeline_in,
            timeline_out,
            body: SpanBody::Dissolve {
                left: left.slice_body(timeline_in, timeline_out),
                right: right.slice_body(timeline_in, timeline_out),
            },
        }
    }
}

impl PreparedClip {
    fn is_media(&self) -> bool {
        matches!(self.kind, PreparedKind::Media(_))
    }

    fn slice(&self, timeline_in: i64, timeline_out: i64) -> Span {
        Span {
            timeline_in,
            timeline_out,
            body: match &self.kind {
                PreparedKind::Matte => SpanBody::Gap,
                PreparedKind::Media(_) => {
                    SpanBody::Clip(self.slice_body(timeline_in, timeline_out))
                }
            },
        }
    }

    fn slice_body(&self, timeline_in: i64, timeline_out: i64) -> ClipSlice {
        match &self.kind {
            PreparedKind::Matte => ClipSlice {
                input_index: 0,
                source_in: 0,
                source_out: 0,
                speed: 1.0,
                reversed: false,
                still: false,
                has_picture: false,
                has_sound: false,
            },
            PreparedKind::Media(media) => {
                let (source_in, source_out) = if media.still {
                    (0, timeline_out.saturating_sub(timeline_in))
                } else {
                    source_range(media, timeline_in, timeline_out)
                };
                ClipSlice {
                    input_index: media.input_index,
                    source_in,
                    source_out,
                    speed: media.speed,
                    reversed: media.reversed,
                    still: media.still,
                    has_picture: media.has_picture,
                    has_sound: media.has_sound,
                }
            }
        }
    }
}

fn source_range(media: &MediaClip, timeline_in: i64, timeline_out: i64) -> (i64, i64) {
    let start_off = scale_ticks(timeline_in - media.clip_timeline_in, media.speed);
    let end_off = scale_ticks(timeline_out - media.clip_timeline_in, media.speed);
    if media.reversed {
        let end = media.clip_source_out - start_off;
        let start = media.clip_source_out - end_off;
        let start = start.max(0);
        (start, end.max(start.saturating_add(1)))
    } else {
        let start = media.clip_source_in + start_off;
        let end = media.clip_source_in + end_off;
        (start, end.max(start.saturating_add(1)))
    }
}

fn top_clip(clips: &[PreparedClip], time: i64) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (index, clip) in clips.iter().enumerate() {
        if clip.timeline_in <= time && time < clip.timeline_out {
            let replace = best.is_none_or(|current| {
                (clip.timeline_in, clip.order) >= (clips[current].timeline_in, clips[current].order)
            });
            if replace {
                best = Some(index);
            }
        }
    }
    best
}

fn incoming_clip(
    clips: &[PreparedClip],
    current: usize,
    cursor: i64,
    range_out: i64,
) -> Option<usize> {
    let host = &clips[current];
    let mut best: Option<usize> = None;
    for (index, clip) in clips.iter().enumerate() {
        if index == current {
            continue;
        }
        let starts_inside = clip.timeline_in > cursor
            && clip.timeline_in < host.timeline_out
            && clip.timeline_in < range_out
            && clip.timeline_out > clip.timeline_in;
        if !starts_inside {
            continue;
        }
        let replace = best.is_none_or(|current_best| {
            (clip.timeline_in, clip.order)
                < (clips[current_best].timeline_in, clips[current_best].order)
        });
        if replace {
            best = Some(index);
        }
    }
    best
}

fn next_boundary(clips: &[PreparedClip], cursor: i64, range_out: i64) -> i64 {
    let mut next = range_out;
    for clip in clips {
        if clip.timeline_in > cursor && clip.timeline_in < next {
            next = clip.timeline_in;
        }
        if clip.timeline_out > cursor && clip.timeline_out < next {
            next = clip.timeline_out;
        }
    }
    next
}

fn find_transition(
    transitions: &[Transition],
    overlap_in: i64,
    overlap_out: i64,
) -> Option<&Transition> {
    transitions.iter().find(|transition| {
        let (start, end) = transition_window(transition);
        start < overlap_out && end > overlap_in
    })
}

pub(crate) fn transition_window(transition: &Transition) -> (i64, i64) {
    let duration = transition.duration_ticks.max(0);
    match transition.alignment {
        TransitionAlign::Start => (
            transition.center_ticks,
            transition.center_ticks.saturating_add(duration),
        ),
        TransitionAlign::Center => {
            let start = transition.center_ticks.saturating_sub(duration / 2);
            (start, start.saturating_add(duration))
        }
        TransitionAlign::End => (
            transition.center_ticks.saturating_sub(duration),
            transition.center_ticks,
        ),
    }
}

fn merge_adjacent(spans: Vec<Span>) -> Vec<Span> {
    let mut merged: Vec<Span> = Vec::new();
    for span in spans {
        if span.timeline_out <= span.timeline_in {
            continue;
        }
        if let Some(previous) = merged.last_mut() {
            if can_merge(previous, &span) {
                extend_span(previous, &span);
                continue;
            }
        }
        merged.push(span);
    }
    merged
}

fn can_merge(previous: &Span, next: &Span) -> bool {
    if previous.timeline_out != next.timeline_in {
        return false;
    }
    match (&previous.body, &next.body) {
        (SpanBody::Gap, SpanBody::Gap) => true,
        (SpanBody::Clip(left), SpanBody::Clip(right)) => slices_continue(left, right),
        _ => false,
    }
}

fn slices_continue(left: &ClipSlice, right: &ClipSlice) -> bool {
    left.input_index == right.input_index
        && left.still == right.still
        && left.reversed == right.reversed
        && left.has_picture == right.has_picture
        && left.has_sound == right.has_sound
        && (left.speed - right.speed).abs() < 1e-9
        && !left.reversed
        && left.source_out == right.source_in
}

fn extend_span(previous: &mut Span, next: &Span) {
    previous.timeline_out = next.timeline_out;
    if let (SpanBody::Clip(left), SpanBody::Clip(right)) = (&mut previous.body, &next.body) {
        left.source_out = right.source_out;
    }
}

fn clip_kind_name(kind: ClipKind) -> &'static str {
    match kind {
        ClipKind::Media => "media",
        ClipKind::Nested => "nested sequence",
        ClipKind::Adjustment => "adjustment",
        ClipKind::Title => "title",
        ClipKind::ColorMatte => "color matte",
        ClipKind::Caption => "caption",
    }
}
