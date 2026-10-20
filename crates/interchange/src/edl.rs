//! CMX 3600 edit decision lists.
//!
//! A dissolve is written on the incoming event. Record times overlap by the
//! blend length. Import trims that overlap back to an abutting cut.

use videocut_model::{effect_ids, Rational, SequenceSettings, TrackKind, TransitionAlign};

use crate::assemble::{
    DraftClip, DraftMarker, DraftProject, DraftSequence, DraftTrack, DraftTransition,
};
use crate::edit::{head_tail, join_after, prepare, PreparedClip, PreparedSequence, PreparedTrack};
use crate::error::Error;
use crate::names::{
    align_name, clip_kind_name, parse_align, parse_clip_kind, parse_marker_kind, single_line,
    track_kind_name,
};
use crate::timeutil::{
    format_timecode, frames_to_ticks, is_drop_frame, parse_fps_token, parse_timecode, scale_ticks,
    ticks_to_frames, timecode_to_frames, Timecode,
};

pub fn export(project: &videocut_model::Project) -> Result<(String, Vec<String>), Error> {
    let prepared = prepare(project);
    let sequence = prepared.sequences.first().ok_or(Error::NoSequence)?;
    let mut warnings = prepared.warnings;
    if prepared.sequences.len() > 1 {
        warnings.push("CMX 3600 holds one sequence; only the first sequence was exported".into());
    }
    Ok((write_sequence(&prepared.name, sequence), warnings))
}

pub fn export_sequence(
    project: &videocut_model::Project,
    sequence_id: &str,
) -> Result<(String, Vec<String>), Error> {
    let prepared = prepare(project);
    let Some(position) = project
        .sequences
        .iter()
        .position(|sequence| sequence.id == sequence_id)
    else {
        return Err(Error::SequenceNotFound(sequence_id.to_string()));
    };
    let Some(sequence) = prepared.sequences.get(position) else {
        return Err(Error::SequenceNotFound(sequence_id.to_string()));
    };
    Ok((write_sequence(&prepared.name, sequence), prepared.warnings))
}

pub fn import(text: &str) -> Result<(DraftProject, Vec<String>), Error> {
    if text.trim().is_empty() {
        return Err(Error::Edl("the list is empty".into()));
    }
    let mut warnings = Vec::new();
    let mut title: Option<String> = None;
    let mut project_name: Option<String> = None;
    let mut drop_frame = false;
    let mut fps: Option<Rational> = None;
    let mut width = 1920_u32;
    let mut height = 1080_u32;
    let mut sample_rate = 48_000_u32;
    let mut channels = 2_u16;
    let mut start_tc: Option<Timecode> = None;
    let mut events: Vec<RawEvent> = Vec::new();
    let mut markers: Vec<RawMarker> = Vec::new();
    let mut sticky_track: Option<(TrackKind, u32)> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = strip_prefix_ignore_ascii(line, "TITLE:") {
            title = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = strip_prefix_ignore_ascii(line, "FCM:") {
            let upper = rest.to_ascii_uppercase();
            drop_frame = upper.contains("DROP FRAME") && !upper.contains("NON-DROP");
            continue;
        }
        if let Some(rest) = line.strip_prefix('*') {
            apply_comment(
                rest.trim(),
                &mut events,
                &mut markers,
                &mut project_name,
                &mut fps,
                &mut width,
                &mut height,
                &mut sample_rate,
                &mut channels,
                &mut start_tc,
                &mut sticky_track,
                &mut warnings,
            );
            continue;
        }
        if line.starts_with("M2") || line.starts_with("M0") {
            warnings.push("motion reshape lines are not imported".into());
            continue;
        }
        if let Some(mut event) = parse_event(line) {
            if let Some((kind, index)) = sticky_track {
                event.kind = kind;
                event.track_index = index;
            }
            events.push(event);
            continue;
        }
        warnings.push(format!("ignored EDL line '{line}'"));
    }

    let fps = fps.unwrap_or_else(|| {
        if drop_frame {
            Rational {
                num: 30000,
                den: 1001,
            }
        } else {
            Rational { num: 30, den: 1 }
        }
    });
    let start_ticks = start_tc
        .map(|tc| frames_to_ticks(timecode_to_frames(tc, fps, drop_frame), fps))
        .unwrap_or(0);

    let mut groups: Vec<((TrackKind, u32), Vec<RawEvent>)> = Vec::new();
    for event in events {
        let key = (event.kind, event.track_index);
        if let Some((_, bucket)) = groups.iter_mut().find(|(candidate, _)| *candidate == key) {
            bucket.push(event);
        } else {
            groups.push((key, vec![event]));
        }
    }

    let mut tracks = Vec::new();
    for ((kind, _), mut bucket) in groups {
        bucket.sort_by_key(|event| {
            frames_to_ticks(timecode_to_frames(event.rec_in, fps, drop_frame), fps)
        });
        tracks.push(track_from_events(
            kind,
            bucket,
            fps,
            drop_frame,
            start_ticks,
            &mut warnings,
        ));
    }

    let markers = markers
        .into_iter()
        .map(|marker| {
            let time = frames_to_ticks(timecode_to_frames(marker.time, fps, drop_frame), fps)
                - start_ticks;
            DraftMarker {
                time_ticks: time,
                duration_ticks: marker.duration,
                name: marker.name,
                comment: marker.comment,
                color: marker.color,
                kind: marker.kind,
            }
        })
        .collect();

    let sequence_name = title.unwrap_or_else(|| "Sequence".into());
    Ok((
        DraftProject {
            name: project_name.unwrap_or_else(|| sequence_name.clone()),
            sequences: vec![DraftSequence {
                name: sequence_name,
                settings: SequenceSettings {
                    width,
                    height,
                    fps,
                    sample_rate,
                    audio_channels: channels,
                    start_timecode_ticks: start_ticks,
                },
                tracks,
                markers,
            }],
            warnings: Vec::new(),
        },
        warnings,
    ))
}

struct RawEvent {
    kind: TrackKind,
    track_index: u32,
    reel: String,
    transition: char,
    duration_frames: i64,
    src_in: Timecode,
    src_out: Timecode,
    rec_in: Timecode,
    rec_out: Timecode,
    clip_name: Option<String>,
    path: Option<String>,
    effect_id: Option<String>,
    alignment: Option<TransitionAlign>,
    speed: f64,
    reversed: bool,
    clip_kind: videocut_model::ClipKind,
}

struct RawMarker {
    time: Timecode,
    duration: i64,
    name: String,
    comment: String,
    color: u8,
    kind: videocut_model::MarkerKind,
}

struct Placed {
    event: RawEvent,
    timeline_in: i64,
    timeline_out: i64,
    source_in: i64,
    source_out: i64,
}

fn track_from_events(
    kind: TrackKind,
    events: Vec<RawEvent>,
    fps: Rational,
    drop_frame: bool,
    start_ticks: i64,
    warnings: &mut Vec<String>,
) -> DraftTrack {
    let mut placed: Vec<Placed> = events
        .into_iter()
        .map(|event| {
            let timeline_in =
                frames_to_ticks(timecode_to_frames(event.rec_in, fps, drop_frame), fps)
                    - start_ticks;
            let timeline_out =
                frames_to_ticks(timecode_to_frames(event.rec_out, fps, drop_frame), fps)
                    - start_ticks;
            let source_in = frames_to_ticks(timecode_to_frames(event.src_in, fps, drop_frame), fps);
            let source_out =
                frames_to_ticks(timecode_to_frames(event.src_out, fps, drop_frame), fps);
            Placed {
                event,
                timeline_in,
                timeline_out,
                source_in,
                source_out,
            }
        })
        .collect();

    let mut transitions = Vec::new();
    for index in 0..placed.len() {
        let transition = placed[index].event.transition.to_ascii_uppercase();
        if index == 0 {
            if matches!(transition, 'D' | 'W' | 'K') {
                warnings.push("a dissolve on the first event was kept as a cut".into());
            }
            continue;
        }
        if !matches!(transition, 'D' | 'W' | 'K') {
            continue;
        }
        let previous_out = placed[index - 1].timeline_out;
        let incoming_in = placed[index].timeline_in;
        let overlap = previous_out - incoming_in;
        let stated = if placed[index].event.duration_frames > 0 {
            frames_to_ticks(placed[index].event.duration_frames, fps)
        } else {
            overlap.max(0)
        };
        let alignment = placed[index]
            .event
            .alignment
            .unwrap_or(TransitionAlign::Center);
        let effect_id = placed[index].event.effect_id.clone().unwrap_or_else(|| {
            if transition == 'W' {
                effect_ids::WIPE.to_string()
            } else {
                effect_ids::CROSS_DISSOLVE.to_string()
            }
        });
        if overlap > 0 {
            let start = incoming_in;
            let end = previous_out;
            let cut = match alignment {
                TransitionAlign::Start => start,
                TransitionAlign::End => end,
                TransitionAlign::Center => start + (end - start) / 2,
            };
            let tail = (previous_out - cut).max(0);
            let head = (cut - incoming_in).max(0);
            trim_end(&mut placed[index - 1], tail);
            trim_start(&mut placed[index], head);
            transitions.push(DraftTransition {
                effect_id,
                center_ticks: cut,
                duration_ticks: (end - start).max(stated.max(0)),
                alignment,
            });
        } else if stated > 0 {
            let cut = previous_out;
            let center = match alignment {
                TransitionAlign::Start => cut + stated / 2,
                TransitionAlign::End => cut - stated / 2,
                TransitionAlign::Center => cut,
            };
            transitions.push(DraftTransition {
                effect_id,
                center_ticks: center,
                duration_ticks: stated,
                alignment,
            });
        }
    }

    let clips = placed
        .into_iter()
        .filter(|clip| clip.timeline_out > clip.timeline_in)
        .map(|clip| {
            let name = clip
                .event
                .clip_name
                .clone()
                .unwrap_or_else(|| clip.event.reel.clone());
            DraftClip {
                media_name: name.clone(),
                name,
                path: clip.event.path.unwrap_or_default(),
                media_duration: clip.source_out.max(0),
                video: None,
                audio: None,
                timeline_in: clip.timeline_in,
                timeline_out: clip.timeline_out,
                source_in: clip.source_in,
                source_out: clip.source_out,
                speed: clip.event.speed,
                reversed: clip.event.reversed,
                enabled: true,
                kind: clip.event.clip_kind,
                label_color: 0,
            }
        })
        .collect();
    DraftTrack {
        kind,
        clips,
        transitions,
    }
}

fn trim_end(clip: &mut Placed, tail: i64) {
    if tail <= 0 {
        return;
    }
    let speed = clip.event.speed.abs();
    clip.timeline_out -= tail;
    if clip.event.reversed {
        clip.source_in += scale_ticks(tail, speed);
    } else {
        clip.source_out -= scale_ticks(tail, speed);
    }
}

fn trim_start(clip: &mut Placed, head: i64) {
    if head <= 0 {
        return;
    }
    let speed = clip.event.speed.abs();
    clip.timeline_in += head;
    if clip.event.reversed {
        clip.source_out -= scale_ticks(head, speed);
    } else {
        clip.source_in += scale_ticks(head, speed);
    }
}

fn write_sequence(project_name: &str, sequence: &PreparedSequence) -> String {
    let fps = sequence.settings.fps;
    let drop = is_drop_frame(fps);
    let mut lines = Vec::new();
    lines.push(format!("TITLE: {}", single_line(&sequence.name)));
    lines.push(format!(
        "FCM: {}",
        if drop { "DROP FRAME" } else { "NON-DROP FRAME" }
    ));
    lines.push(format!("* VIDEOCUT_PROJECT: {}", single_line(project_name)));
    lines.push(format!("* VIDEOCUT_FPS: {}/{}", fps.num, fps.den));
    lines.push(format!(
        "* VIDEOCUT_SIZE: {}x{}",
        sequence.settings.width, sequence.settings.height
    ));
    lines.push(format!(
        "* VIDEOCUT_AUDIO: {} {}",
        sequence.settings.sample_rate, sequence.settings.audio_channels
    ));
    lines.push(format!(
        "* VIDEOCUT_START: {}",
        format_timecode(
            ticks_to_frames(sequence.settings.start_timecode_ticks, fps),
            fps,
            drop
        )
    ));
    lines.push(String::new());

    let mut number = 1_u32;
    for track in sequence
        .video_tracks
        .iter()
        .chain(sequence.audio_tracks.iter())
    {
        lines.push(format!(
            "* VIDEOCUT_TRACK: {} {}",
            track_kind_name(track.kind),
            track.index
        ));
        number = write_track(&mut lines, track, sequence, number, drop);
    }
    for marker in &sequence.markers {
        lines.push(format!(
            "* MARKER: {} | {} | {} | {} | {} | {}",
            format_timecode(
                ticks_to_frames(
                    marker.time_ticks + sequence.settings.start_timecode_ticks,
                    fps
                ),
                fps,
                drop
            ),
            single_line(&marker.name),
            single_line(&marker.comment),
            marker.duration_ticks,
            marker.color,
            crate::names::marker_kind_name(marker.kind)
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn write_track(
    lines: &mut Vec<String>,
    track: &PreparedTrack,
    sequence: &PreparedSequence,
    mut number: u32,
    drop: bool,
) -> u32 {
    let fps = sequence.settings.fps;
    let start = sequence.settings.start_timecode_ticks;
    let code = track_code(track.kind, track.index);
    for (index, clip) in track.clips.iter().enumerate() {
        let (head, tail) = head_tail(track, index);
        let rec_in = clip.timeline_in - head + start;
        let rec_out = clip.timeline_out + tail + start;
        let src_in = source_edge(clip, head, true);
        let src_out = source_edge(clip, tail, false);
        let join_before = index
            .checked_sub(1)
            .and_then(|previous| join_after(track, previous));
        let (trans, frames) = if let Some(join) = join_before {
            let letter = if join.effect_id == effect_ids::WIPE {
                'W'
            } else {
                'D'
            };
            (letter, ticks_to_frames(join.duration_ticks, fps))
        } else {
            ('C', 0)
        };
        lines.push(format_event(
            number,
            &reel_of(clip),
            &code,
            trans,
            frames,
            [
                format_timecode(ticks_to_frames(src_in, fps), fps, drop),
                format_timecode(ticks_to_frames(src_out, fps), fps, drop),
                format_timecode(ticks_to_frames(rec_in, fps), fps, drop),
                format_timecode(ticks_to_frames(rec_out, fps), fps, drop),
            ],
        ));
        lines.push(format!("* FROM CLIP NAME: {}", single_line(&clip.name)));
        if !clip.path.is_empty() {
            lines.push(format!("* SOURCE FILE: {}", single_line(&clip.path)));
        }
        if let Some(join) = join_before {
            lines.push(format!("* VIDEOCUT_EFFECT: {}", join.effect_id));
            lines.push(format!("* VIDEOCUT_ALIGN: {}", align_name(join.alignment)));
        }
        if (clip.speed - 1.0).abs() > 1e-9 {
            lines.push(format!("* VIDEOCUT_SPEED: {}", clip.speed));
        }
        if clip.reversed {
            lines.push("* VIDEOCUT_REVERSED: 1".into());
        }
        if clip.kind != videocut_model::ClipKind::Media {
            lines.push(format!("* VIDEOCUT_KIND: {}", clip_kind_name(clip.kind)));
        }
        number += 1;
    }
    number
}

fn source_edge(clip: &PreparedClip, extra: i64, head: bool) -> i64 {
    let delta = scale_ticks(extra, clip.speed.abs());
    if head {
        if clip.reversed {
            clip.source_out + delta
        } else {
            clip.source_in - delta
        }
    } else if clip.reversed {
        clip.source_in - delta
    } else {
        clip.source_out + delta
    }
}

fn format_event(
    number: u32,
    reel: &str,
    track: &str,
    transition: char,
    frames: i64,
    timecodes: [String; 4],
) -> String {
    let trans = if transition == 'C' {
        "C".to_string()
    } else {
        format!("{transition}    {frames:03}")
    };
    format!(
        "{number:03}  {reel:<8} {track:<4} {trans:<8} {} {} {} {}",
        timecodes[0], timecodes[1], timecodes[2], timecodes[3]
    )
}

fn reel_of(clip: &PreparedClip) -> String {
    let source = if clip.media_name.is_empty() {
        clip.name.as_str()
    } else {
        clip.media_name.as_str()
    };
    let mut reel: String = source
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_uppercase())
        .take(8)
        .collect();
    if reel.is_empty() {
        reel = "AX".into();
    }
    reel
}

fn track_code(kind: TrackKind, index: u32) -> String {
    match kind {
        TrackKind::Video => {
            if index == 0 {
                "V".into()
            } else {
                format!("V{}", index + 1)
            }
        }
        TrackKind::Audio => {
            if index == 0 {
                "A".into()
            } else {
                format!("A{}", index + 1)
            }
        }
    }
}

fn parse_event(line: &str) -> Option<RawEvent> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 7 {
        return None;
    }
    let _number: u32 = parts[0].parse().ok()?;
    let reel = parts[1].to_string();
    let (kind, track_index) = parse_track_code(parts[2])?;
    let trans_tok = parts[3];
    let transition = trans_tok.chars().next()?.to_ascii_uppercase();
    if !matches!(transition, 'C' | 'D' | 'W' | 'K') {
        return None;
    }
    let mut index = 4;
    let mut duration_frames = 0_i64;
    if parts.get(index).is_some_and(|token| {
        token.chars().all(|ch| ch.is_ascii_digit())
            && parts
                .get(index + 1)
                .is_some_and(|next| next.contains(':') || next.contains(';'))
    }) {
        duration_frames = parts[index].parse().ok()?;
        index += 1;
    }
    if parts.len() < index + 4 {
        return None;
    }
    Some(RawEvent {
        kind,
        track_index,
        reel,
        transition,
        duration_frames,
        src_in: parse_timecode(parts[index])?,
        src_out: parse_timecode(parts[index + 1])?,
        rec_in: parse_timecode(parts[index + 2])?,
        rec_out: parse_timecode(parts[index + 3])?,
        clip_name: None,
        path: None,
        effect_id: None,
        alignment: None,
        speed: 1.0,
        reversed: false,
        clip_kind: videocut_model::ClipKind::Media,
    })
}

fn parse_track_code(token: &str) -> Option<(TrackKind, u32)> {
    let upper = token.to_ascii_uppercase();
    if upper == "V" || upper == "B" || upper == "V1" {
        return Some((TrackKind::Video, 0));
    }
    if let Some(rest) = upper.strip_prefix('V') {
        let index: u32 = rest.parse().ok()?;
        return Some((TrackKind::Video, index.saturating_sub(1)));
    }
    if upper == "A" || upper == "A1" || upper == "AA" || upper == "AUD" {
        return Some((TrackKind::Audio, 0));
    }
    if let Some(rest) = upper.strip_prefix('A') {
        let index: u32 = rest.parse().ok()?;
        return Some((TrackKind::Audio, index.saturating_sub(1)));
    }
    None
}

fn apply_comment(
    body: &str,
    events: &mut [RawEvent],
    markers: &mut Vec<RawMarker>,
    project_name: &mut Option<String>,
    fps: &mut Option<Rational>,
    width: &mut u32,
    height: &mut u32,
    sample_rate: &mut u32,
    channels: &mut u16,
    start_tc: &mut Option<Timecode>,
    sticky_track: &mut Option<(TrackKind, u32)>,
    warnings: &mut Vec<String>,
) {
    let (key, value) = body.split_once(':').unwrap_or((body, ""));
    let key = key.trim().to_ascii_uppercase();
    let value = value.trim();
    match key.as_str() {
        "FROM CLIP NAME" => {
            if let Some(event) = events.last_mut() {
                event.clip_name = Some(value.to_string());
            }
        }
        "SOURCE FILE" => {
            if let Some(event) = events.last_mut() {
                event.path = Some(value.to_string());
            }
        }
        "VIDEOCUT_EFFECT" => {
            if let Some(event) = events.last_mut() {
                event.effect_id = Some(value.to_string());
            }
        }
        "VIDEOCUT_ALIGN" => {
            if let Some(event) = events.last_mut() {
                event.alignment = Some(parse_align(value));
            }
        }
        "VIDEOCUT_SPEED" => {
            if let Some(event) = events.last_mut() {
                if let Ok(speed) = value.parse::<f64>() {
                    event.speed = speed;
                }
            }
        }
        "VIDEOCUT_REVERSED" => {
            if let Some(event) = events.last_mut() {
                event.reversed = value != "0" && !value.eq_ignore_ascii_case("false");
            }
        }
        "VIDEOCUT_KIND" => {
            if let Some(event) = events.last_mut() {
                event.clip_kind = parse_clip_kind(value);
            }
        }
        "VIDEOCUT_PROJECT" => *project_name = Some(value.to_string()),
        "VIDEOCUT_FPS" => {
            if let Some(parsed) = parse_fps_token(value) {
                *fps = Some(parsed);
            } else {
                warnings.push(format!("ignored frame rate '{value}'"));
            }
        }
        "VIDEOCUT_SIZE" => {
            if let Some((w, h)) = value.split_once('x') {
                if let (Ok(w), Ok(h)) = (w.parse(), h.parse()) {
                    *width = w;
                    *height = h;
                }
            }
        }
        "VIDEOCUT_AUDIO" => {
            let mut parts = value.split_whitespace();
            if let Some(rate) = parts.next().and_then(|text| text.parse().ok()) {
                *sample_rate = rate;
            }
            if let Some(count) = parts.next().and_then(|text| text.parse().ok()) {
                *channels = count;
            }
        }
        "VIDEOCUT_START" => {
            if let Some(tc) = parse_timecode(value) {
                *start_tc = Some(tc);
            }
        }
        "VIDEOCUT_TRACK" => {
            let mut parts = value.split_whitespace();
            let kind = match parts.next().unwrap_or("").to_ascii_lowercase().as_str() {
                "audio" => TrackKind::Audio,
                _ => TrackKind::Video,
            };
            let index = parts.next().and_then(|text| text.parse().ok()).unwrap_or(0);
            *sticky_track = Some((kind, index));
        }
        "MARKER" => {
            if let Some(marker) = parse_marker(value) {
                markers.push(marker);
            } else {
                warnings.push(format!("ignored marker '{value}'"));
            }
        }
        "LOC" => {
            if let Some(marker) = parse_loc(value) {
                markers.push(marker);
            }
        }
        _ => {}
    }
}

fn parse_marker(value: &str) -> Option<RawMarker> {
    let mut parts = value.split('|');
    let time = parse_timecode(parts.next()?.trim())?;
    let name = parts.next().unwrap_or("").trim().to_string();
    let comment = parts.next().unwrap_or("").trim().to_string();
    let duration = parts
        .next()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0);
    let color = parts
        .next()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0);
    let kind = parts
        .next()
        .map(|text| parse_marker_kind(text.trim()))
        .unwrap_or(videocut_model::MarkerKind::Comment);
    Some(RawMarker {
        time,
        duration,
        name,
        comment,
        color,
        kind,
    })
}

fn parse_loc(value: &str) -> Option<RawMarker> {
    let mut parts = value.split_whitespace();
    let time = parse_timecode(parts.next()?)?;
    let _color = parts.next();
    let name = parts.collect::<Vec<_>>().join(" ");
    Some(RawMarker {
        time,
        duration: 0,
        name,
        comment: String::new(),
        color: 0,
        kind: videocut_model::MarkerKind::Comment,
    })
}

fn strip_prefix_ignore_ascii<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    if text.len() >= prefix.len() && text[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&text[prefix.len()..])
    } else {
        None
    }
}
