//! OpenTimelineIO JSON. Clips on a track abut; a transition between them
//! overlaps `in_offset` of the previous item and `out_offset` of the next.

use serde_json::{json, Map, Value};
use videocut_model::{
    AudioStreamInfo, Rational, SequenceSettings, TrackKind, TransitionAlign, VideoStreamInfo,
    TICKS_PER_SECOND,
};

use crate::assemble::{
    DraftClip, DraftMarker, DraftProject, DraftSequence, DraftTrack, DraftTransition,
};
use crate::edit::{
    join_after, prepare, PreparedClip, PreparedJoin, PreparedSequence, PreparedTrack,
};
use crate::error::Error;
use crate::names::{
    align_name, clip_kind_name, effect_from_otio, file_url, marker_color_index, marker_color_name,
    marker_kind_name, otio_transition_type, parse_align, parse_clip_kind, parse_marker_kind,
    path_from_url, track_kind_name,
};
use crate::timeutil::{f64_to_ticks, rational_to_ticks, scale_ticks, ticks_to_rational};

pub fn export(project: &videocut_model::Project) -> Result<(String, Vec<String>), Error> {
    let prepared = prepare(project);
    if prepared.sequences.is_empty() {
        return Err(Error::NoSequence);
    }
    let value = if prepared.sequences.len() == 1 {
        timeline_value(&prepared.sequences[0], &prepared.name)
    } else {
        json!({
            "OTIO_SCHEMA": "SerializableCollection.1",
            "name": prepared.name,
            "metadata": {},
            "children": prepared
                .sequences
                .iter()
                .map(|sequence| timeline_value(sequence, &prepared.name))
                .collect::<Vec<_>>()
        })
    };
    let text = serde_json::to_string_pretty(&value).map_err(|err| Error::Otio(err.to_string()))?;
    Ok((text, prepared.warnings))
}

pub fn import(text: &str) -> Result<(DraftProject, Vec<String>), Error> {
    let value: Value = serde_json::from_str(text).map_err(|err| Error::Otio(err.to_string()))?;
    let mut warnings = Vec::new();
    let schema = schema_of(&value);
    let (name, timelines) = if schema_is(&value, "SerializableCollection") {
        let name = string_field(&value, "name").unwrap_or_else(|| "Untitled".into());
        let children = value
            .get("children")
            .and_then(|child| child.as_array())
            .cloned()
            .unwrap_or_default();
        let mut timelines = Vec::new();
        for child in children {
            if schema_is(&child, "Timeline") {
                timelines.push(child);
            } else {
                warnings.push(format!("skipped collection child {}", schema_of(&child)));
            }
        }
        (name, timelines)
    } else if schema_is(&value, "Timeline") {
        let name = string_field(&value, "name").unwrap_or_else(|| "Untitled".into());
        (name, vec![value])
    } else {
        return Err(Error::Otio(format!(
            "expected a Timeline or SerializableCollection, found {schema}"
        )));
    };
    if timelines.is_empty() {
        return Err(Error::Otio("the document has no timeline".into()));
    }
    let mut sequences = Vec::new();
    for timeline in timelines {
        sequences.push(import_timeline(&timeline, &mut warnings)?);
    }
    let project_name = sequences
        .first()
        .and_then(|sequence| sequence.project_name.clone())
        .unwrap_or(name);
    let sequences = sequences
        .into_iter()
        .map(|loaded| loaded.sequence)
        .collect();
    Ok((
        DraftProject {
            name: project_name,
            sequences,
            warnings: Vec::new(),
        },
        warnings,
    ))
}

struct LoadedTimeline {
    project_name: Option<String>,
    sequence: DraftSequence,
}

fn import_timeline(timeline: &Value, warnings: &mut Vec<String>) -> Result<LoadedTimeline, Error> {
    let meta = videocut_meta(timeline);
    let name = string_field(timeline, "name").unwrap_or_else(|| "Sequence".into());
    let project_name = meta
        .and_then(|meta| string_field(meta, "projectName"))
        .filter(|text| !text.is_empty());
    let fps = fps_from_meta(meta).or_else(|| infer_fps(timeline));
    let fps = fps.unwrap_or(Rational { num: 30, den: 1 });
    if fps_from_meta(meta).is_none() {
        warnings.push(format!(
            "sequence '{name}' frame rate was inferred as {}/{}",
            fps.num, fps.den
        ));
    }
    let width = meta
        .and_then(|meta| u32_field(meta, "width"))
        .unwrap_or(1920);
    let height = meta
        .and_then(|meta| u32_field(meta, "height"))
        .unwrap_or(1080);
    let sample_rate = meta
        .and_then(|meta| u32_field(meta, "sampleRate"))
        .unwrap_or(48_000);
    let audio_channels = meta
        .and_then(|meta| u32_field(meta, "audioChannels"))
        .unwrap_or(2) as u16;
    let start = match timeline.get("global_start_time") {
        Some(time) if !time.is_null() => read_time(time, warnings)?,
        _ => 0,
    };
    let stack = timeline
        .get("tracks")
        .ok_or_else(|| Error::Otio(format!("timeline '{name}' is missing its track stack")))?;
    let children = stack
        .get("children")
        .and_then(|child| child.as_array())
        .ok_or_else(|| Error::Otio(format!("timeline '{name}' has no tracks")))?;
    let mut tracks = Vec::new();
    let mut markers = Vec::new();
    for (index, child) in children.iter().enumerate() {
        if !schema_is(child, "Track") {
            warnings.push(format!("skipped stack child {}", schema_of(child)));
            continue;
        }
        let kind = match string_field(child, "kind").as_deref() {
            Some("Audio") => TrackKind::Audio,
            _ => TrackKind::Video,
        };
        let (track, track_markers) = import_track(child, kind, warnings)?;
        if index == 0 || markers.is_empty() {
            markers.extend(track_markers);
        } else {
            markers.extend(track_markers);
        }
        tracks.push(track);
    }
    if let Some(meta_markers) = meta
        .and_then(|meta| meta.get("markers"))
        .and_then(|v| v.as_array())
    {
        markers = meta_markers
            .iter()
            .filter_map(|marker| draft_marker_from_meta(marker))
            .collect();
    }
    Ok(LoadedTimeline {
        project_name,
        sequence: DraftSequence {
            name,
            settings: SequenceSettings {
                width,
                height,
                fps,
                sample_rate,
                audio_channels,
                start_timecode_ticks: start,
            },
            tracks,
            markers,
        },
    })
}

fn import_track(
    track: &Value,
    kind: TrackKind,
    warnings: &mut Vec<String>,
) -> Result<(DraftTrack, Vec<DraftMarker>), Error> {
    let children = track
        .get("children")
        .and_then(|child| child.as_array())
        .cloned()
        .unwrap_or_default();
    let mut clips = Vec::new();
    let mut transitions = Vec::new();
    let mut cursor = 0_i64;
    let mut pending: Option<PendingTransition> = None;
    for child in children {
        if schema_is(&child, "Transition") {
            if pending.is_some() {
                warnings.push("adjacent transitions were collapsed to the latest one".into());
            }
            let in_offset = child
                .get("in_offset")
                .map(|time| read_time(time, warnings))
                .transpose()?
                .unwrap_or(0)
                .max(0);
            let out_offset = child
                .get("out_offset")
                .map(|time| read_time(time, warnings))
                .transpose()?
                .unwrap_or(0)
                .max(0);
            let effect = effect_from_otio(
                string_field(&child, "transition_type")
                    .as_deref()
                    .unwrap_or(""),
                videocut_meta(&child)
                    .and_then(|meta| string_field(meta, "effectId"))
                    .as_deref(),
            );
            let alignment = videocut_meta(&child)
                .and_then(|meta| string_field(meta, "alignment"))
                .map(|text| parse_align(&text))
                .unwrap_or_else(|| alignment_from_offsets(in_offset, out_offset));
            pending = Some(PendingTransition {
                in_offset,
                out_offset,
                effect_id: effect,
                alignment,
            });
            continue;
        }
        let is_gap = schema_is(&child, "Gap");
        let (source_in, duration) = item_range(&child, warnings)?;
        let timeline_in = cursor;
        let timeline_out = cursor + duration.max(0);
        if let Some(transition) = pending.take() {
            let cut = timeline_in;
            let duration_ticks = transition.in_offset + transition.out_offset;
            let center = cut - transition.in_offset + duration_ticks / 2;
            if duration_ticks > 0 {
                transitions.push(DraftTransition {
                    effect_id: transition.effect_id,
                    center_ticks: center,
                    duration_ticks,
                    alignment: transition.alignment,
                });
            }
        }
        cursor = timeline_out;
        if is_gap || duration <= 0 {
            continue;
        }
        clips.push(import_clip(
            &child,
            source_in,
            timeline_in,
            timeline_out,
            warnings,
        )?);
    }
    if pending.is_some() {
        warnings.push("a transition at the end of a track was omitted".into());
    }
    let markers = track
        .get("markers")
        .and_then(|markers| markers.as_array())
        .map(|markers| {
            markers
                .iter()
                .filter_map(|marker| import_marker(marker, warnings))
                .collect()
        })
        .unwrap_or_default();
    Ok((
        DraftTrack {
            kind,
            clips,
            transitions,
        },
        markers,
    ))
}

struct PendingTransition {
    in_offset: i64,
    out_offset: i64,
    effect_id: String,
    alignment: TransitionAlign,
}

fn alignment_from_offsets(in_offset: i64, out_offset: i64) -> TransitionAlign {
    if in_offset == 0 && out_offset > 0 {
        TransitionAlign::Start
    } else if out_offset == 0 && in_offset > 0 {
        TransitionAlign::End
    } else {
        TransitionAlign::Center
    }
}

fn import_clip(
    clip: &Value,
    source_in: i64,
    timeline_in: i64,
    timeline_out: i64,
    warnings: &mut Vec<String>,
) -> Result<DraftClip, Error> {
    let meta = videocut_meta(clip);
    let (speed, reversed) = speed_of(clip);
    let media = media_reference(clip);
    let target = media
        .and_then(|media| string_field(media, "target_url"))
        .unwrap_or_default();
    let path = if target.is_empty() {
        String::new()
    } else {
        path_from_url(&target)
    };
    let media_name = media
        .and_then(|media| string_field(media, "name"))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| string_field(clip, "name").unwrap_or_else(|| "Clip".into()));
    let available = media
        .and_then(|media| media.get("available_range"))
        .map(|range| range_duration(range, warnings))
        .transpose()?
        .unwrap_or(0);
    let video = media.and_then(video_from_meta);
    let audio = media.and_then(audio_from_meta);
    let timeline_duration = timeline_out - timeline_in;
    let source_delta = scale_ticks(timeline_duration, speed);
    Ok(DraftClip {
        name: string_field(clip, "name").unwrap_or_else(|| media_name.clone()),
        media_name,
        path,
        media_duration: available.max(source_in + source_delta),
        video,
        audio,
        timeline_in,
        timeline_out,
        source_in,
        source_out: source_in + source_delta,
        speed,
        reversed,
        enabled: clip
            .get("enabled")
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
        kind: meta
            .and_then(|meta| string_field(meta, "kind"))
            .map(|kind| parse_clip_kind(&kind))
            .unwrap_or(videocut_model::ClipKind::Media),
        label_color: meta
            .and_then(|meta| u32_field(meta, "labelColor"))
            .unwrap_or(0) as u8,
    })
}

fn speed_of(clip: &Value) -> (f64, bool) {
    let Some(effects) = clip.get("effects").and_then(|value| value.as_array()) else {
        return (1.0, false);
    };
    for effect in effects {
        let schema = schema_of(effect);
        if schema.starts_with("LinearTimeWarp") || schema.starts_with("FreezeFrame") {
            let scalar = effect
                .get("time_scalar")
                .and_then(|value| value.as_f64())
                .unwrap_or(1.0);
            if schema.starts_with("FreezeFrame") {
                return (0.0, false);
            }
            if scalar < 0.0 {
                return (-scalar, true);
            }
            return (scalar, false);
        }
    }
    (1.0, false)
}

fn media_reference(clip: &Value) -> Option<&Value> {
    if let Some(refs) = clip
        .get("media_references")
        .and_then(|value| value.as_object())
    {
        if let Some(key) = clip
            .get("active_media_reference_key")
            .and_then(|value| value.as_str())
        {
            if let Some(reference) = refs.get(key) {
                return Some(reference);
            }
        }
        return refs.values().next();
    }
    clip.get("media_reference")
}

fn item_range(item: &Value, warnings: &mut Vec<String>) -> Result<(i64, i64), Error> {
    if let Some(range) = item.get("source_range") {
        if !range.is_null() {
            let start = range
                .get("start_time")
                .map(|time| read_time(time, warnings))
                .transpose()?
                .unwrap_or(0);
            let duration = range
                .get("duration")
                .map(|time| read_time(time, warnings))
                .transpose()?
                .unwrap_or(0);
            return Ok((start, duration));
        }
    }
    if let Some(media) = media_reference(item) {
        if let Some(range) = media.get("available_range") {
            if !range.is_null() {
                let start = range
                    .get("start_time")
                    .map(|time| read_time(time, warnings))
                    .transpose()?
                    .unwrap_or(0);
                let duration = range_duration(range, warnings)?;
                return Ok((start, duration));
            }
        }
    }
    warnings.push(format!(
        "item '{}' has no source range and was given zero length",
        string_field(item, "name").unwrap_or_default()
    ));
    Ok((0, 0))
}

fn range_duration(range: &Value, warnings: &mut Vec<String>) -> Result<i64, Error> {
    range
        .get("duration")
        .map(|time| read_time(time, warnings))
        .transpose()
        .map(|duration| duration.unwrap_or(0))
}

fn import_marker(marker: &Value, warnings: &mut Vec<String>) -> Option<DraftMarker> {
    let range = marker.get("marked_range")?;
    let time = range
        .get("start_time")
        .map(|time| read_time(time, warnings))
        .transpose()
        .ok()??;
    let duration = range
        .get("duration")
        .map(|time| read_time(time, warnings))
        .transpose()
        .ok()
        .flatten()
        .unwrap_or(0);
    let meta = videocut_meta(marker);
    Some(draft_marker(
        time,
        duration,
        string_field(marker, "name").unwrap_or_default(),
        string_field(marker, "comment").unwrap_or_default(),
        meta.and_then(|meta| u32_field(meta, "color"))
            .map(|color| color as u8)
            .unwrap_or_else(|| {
                marker_color_index(string_field(marker, "color").as_deref().unwrap_or(""))
            }),
        meta.and_then(|meta| string_field(meta, "kind"))
            .map(|kind| parse_marker_kind(&kind))
            .unwrap_or(videocut_model::MarkerKind::Comment),
    ))
}

fn draft_marker(
    time_ticks: i64,
    duration_ticks: i64,
    name: String,
    comment: String,
    color: u8,
    kind: videocut_model::MarkerKind,
) -> DraftMarker {
    DraftMarker {
        time_ticks,
        duration_ticks,
        name,
        comment,
        color,
        kind,
    }
}

fn draft_marker_from_meta(marker: &Value) -> Option<DraftMarker> {
    Some(DraftMarker {
        time_ticks: i64_field(marker, "timeTicks")?,
        duration_ticks: i64_field(marker, "durationTicks").unwrap_or(0),
        name: string_field(marker, "name").unwrap_or_default(),
        comment: string_field(marker, "comment").unwrap_or_default(),
        color: u32_field(marker, "color").unwrap_or(0) as u8,
        kind: string_field(marker, "kind")
            .map(|kind| parse_marker_kind(&kind))
            .unwrap_or(videocut_model::MarkerKind::Comment),
    })
}

fn timeline_value(sequence: &PreparedSequence, project_name: &str) -> Value {
    let fps = sequence.settings.fps;
    let mut children = Vec::new();
    for (index, track) in sequence
        .video_tracks
        .iter()
        .chain(sequence.audio_tracks.iter())
        .enumerate()
    {
        children.push(track_value(track, sequence, fps, index == 0));
    }
    json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": sequence.name,
        "global_start_time": rational_time(sequence.settings.start_timecode_ticks, fps),
        "metadata": {
            "VideoCut": {
                "projectName": project_name,
                "width": sequence.settings.width,
                "height": sequence.settings.height,
                "fpsNum": fps.num,
                "fpsDen": fps.den,
                "sampleRate": sequence.settings.sample_rate,
                "audioChannels": sequence.settings.audio_channels,
                "markers": sequence.markers.iter().map(|marker| {
                    json!({
                        "timeTicks": marker.time_ticks,
                        "durationTicks": marker.duration_ticks,
                        "name": marker.name,
                        "comment": marker.comment,
                        "color": marker.color,
                        "kind": marker_kind_name(marker.kind)
                    })
                }).collect::<Vec<_>>()
            }
        },
        "tracks": {
            "OTIO_SCHEMA": "Stack.1",
            "name": "tracks",
            "source_range": null,
            "effects": [],
            "markers": [],
            "enabled": true,
            "metadata": {},
            "children": children
        }
    })
}

fn track_value(
    track: &PreparedTrack,
    sequence: &PreparedSequence,
    fps: Rational,
    with_markers: bool,
) -> Value {
    let kind = match track.kind {
        TrackKind::Video => "Video",
        TrackKind::Audio => "Audio",
    };
    let mut children = Vec::new();
    let mut cursor = 0_i64;
    for (index, clip) in track.clips.iter().enumerate() {
        if clip.timeline_in > cursor {
            children.push(gap_value(clip.timeline_in - cursor, fps));
        }
        children.push(clip_value(clip, fps));
        cursor = clip.timeline_out;
        if let Some(join) = join_after(track, index) {
            children.push(transition_value(join, fps));
        }
    }
    let markers = if with_markers {
        sequence
            .markers
            .iter()
            .map(|marker| {
                marker_value(
                    marker.time_ticks,
                    marker.duration_ticks,
                    &marker.name,
                    &marker.comment,
                    marker.color,
                    marker.kind,
                    fps,
                )
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    json!({
        "OTIO_SCHEMA": "Track.1",
        "name": format!("{}{}", kind.chars().next().unwrap_or('V'), track.index + 1),
        "kind": kind,
        "source_range": null,
        "effects": [],
        "markers": markers,
        "enabled": !track.hidden && !track.muted,
        "metadata": {
            "VideoCut": {
                "kind": track_kind_name(track.kind),
                "locked": track.locked,
                "syncLock": track.sync_lock,
                "targeted": track.targeted
            }
        },
        "children": children
    })
}

fn gap_value(duration: i64, fps: Rational) -> Value {
    json!({
        "OTIO_SCHEMA": "Gap.1",
        "name": "Gap",
        "source_range": time_range(0, duration, fps),
        "effects": [],
        "markers": [],
        "enabled": true,
        "metadata": {}
    })
}

fn clip_value(clip: &PreparedClip, fps: Rational) -> Value {
    let mut effects = Vec::new();
    if clip.reversed || (clip.speed - 1.0).abs() > 1e-9 {
        let scalar = if clip.reversed {
            -clip.speed
        } else {
            clip.speed
        };
        effects.push(json!({
            "OTIO_SCHEMA": "LinearTimeWarp.1",
            "name": "",
            "effect_name": "",
            "time_scalar": scalar,
            "metadata": {}
        }));
    }
    let duration = clip.timeline_out - clip.timeline_in;
    let mut reference = json!({
        "OTIO_SCHEMA": "ExternalReference.1",
        "name": if clip.media_name.is_empty() { &clip.name } else { &clip.media_name },
        "target_url": file_url(&clip.path),
        "available_range": if clip.media_duration > 0 {
            time_range(0, clip.media_duration, fps)
        } else {
            Value::Null
        },
        "metadata": media_meta(clip)
    });
    if reference
        .get("available_range")
        .map(|value| value.is_null())
        .unwrap_or(false)
    {
        if let Some(object) = reference.as_object_mut() {
            object.remove("available_range");
        }
    }
    json!({
        "OTIO_SCHEMA": "Clip.2",
        "name": clip.name,
        "source_range": time_range(clip.source_in, duration, fps),
        "media_references": {
            "DEFAULT_MEDIA": reference
        },
        "active_media_reference_key": "DEFAULT_MEDIA",
        "effects": effects,
        "markers": [],
        "enabled": clip.enabled,
        "metadata": {
            "VideoCut": {
                "kind": clip_kind_name(clip.kind),
                "labelColor": clip.label_color
            }
        }
    })
}

fn media_meta(clip: &PreparedClip) -> Value {
    let mut meta = Map::new();
    let mut video_cut = Map::new();
    if let Some(video) = &clip.video {
        video_cut.insert("videoWidth".into(), json!(video.width));
        video_cut.insert("videoHeight".into(), json!(video.height));
        video_cut.insert("videoFpsNum".into(), json!(video.fps.num));
        video_cut.insert("videoFpsDen".into(), json!(video.fps.den));
        video_cut.insert("videoCodec".into(), json!(video.codec));
    }
    if let Some(audio) = &clip.audio {
        video_cut.insert("sampleRate".into(), json!(audio.sample_rate));
        video_cut.insert("channels".into(), json!(audio.channels));
        video_cut.insert("audioCodec".into(), json!(audio.codec));
    }
    if !video_cut.is_empty() {
        meta.insert("VideoCut".into(), Value::Object(video_cut));
    }
    Value::Object(meta)
}

fn transition_value(join: &PreparedJoin, fps: Rational) -> Value {
    json!({
        "OTIO_SCHEMA": "Transition.1",
        "name": crate::names::effect_label(&join.effect_id),
        "transition_type": otio_transition_type(&join.effect_id),
        "in_offset": rational_time(join.in_offset, fps),
        "out_offset": rational_time(join.out_offset, fps),
        "metadata": {
            "VideoCut": {
                "effectId": join.effect_id,
                "alignment": align_name(join.alignment)
            }
        }
    })
}

fn marker_value(
    time: i64,
    duration: i64,
    name: &str,
    comment: &str,
    color: u8,
    kind: videocut_model::MarkerKind,
    fps: Rational,
) -> Value {
    json!({
        "OTIO_SCHEMA": "Marker.2",
        "name": name,
        "color": marker_color_name(color),
        "comment": comment,
        "marked_range": time_range(time, duration, fps),
        "metadata": {
            "VideoCut": {
                "color": color,
                "kind": marker_kind_name(kind)
            }
        }
    })
}

fn time_range(start: i64, duration: i64, fps: Rational) -> Value {
    json!({
        "OTIO_SCHEMA": "TimeRange.1",
        "start_time": rational_time(start, fps),
        "duration": rational_time(duration, fps)
    })
}

fn rational_time(ticks: i64, fps: Rational) -> Value {
    let (value, rate) = ticks_to_rational(ticks, fps);
    json!({
        "OTIO_SCHEMA": "RationalTime.1",
        "value": value,
        "rate": rate
    })
}

fn read_time(value: &Value, _warnings: &mut Vec<String>) -> Result<i64, Error> {
    let Some(value_field) = value.get("value") else {
        return Err(Error::Otio("rational time is missing value".into()));
    };
    let Some(rate_field) = value.get("rate") else {
        return Err(Error::Otio("rational time is missing rate".into()));
    };
    if let (Some(numerator), Some(rate)) = (as_int(value_field), as_int(rate_field)) {
        return Ok(rational_to_ticks(numerator, rate));
    }
    let numerator = value_field
        .as_f64()
        .ok_or_else(|| Error::Otio("rational time value is not a number".into()))?;
    let rate = rate_field
        .as_f64()
        .ok_or_else(|| Error::Otio("rational time rate is not a number".into()))?;
    Ok(f64_to_ticks(numerator, rate))
}

fn as_int(value: &Value) -> Option<i128> {
    if let Some(number) = value.as_i64() {
        return Some(i128::from(number));
    }
    if let Some(number) = value.as_u64() {
        return Some(i128::from(number));
    }
    let number = value.as_f64()?;
    if number.is_finite() && number.fract() == 0.0 {
        return Some(number as i128);
    }
    None
}

fn schema_of(value: &Value) -> String {
    string_field(value, "OTIO_SCHEMA").unwrap_or_else(|| "unknown".into())
}

fn schema_is(value: &Value, name: &str) -> bool {
    schema_of(value)
        .split('.')
        .next()
        .map(|schema| schema == name)
        .unwrap_or(false)
}

fn videocut_meta(value: &Value) -> Option<&Value> {
    value.get("metadata")?.get("VideoCut")
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(|text| text.to_string())
}

fn u32_field(value: &Value, key: &str) -> Option<u32> {
    value
        .get(key)?
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .or_else(|| {
            value
                .get(key)?
                .as_i64()
                .and_then(|number| u32::try_from(number).ok())
        })
}

fn i64_field(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|number| {
        number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|n| i64::try_from(n).ok()))
    })
}

fn fps_from_meta(meta: Option<&Value>) -> Option<Rational> {
    let meta = meta?;
    let num = i64_field(meta, "fpsNum")? as i32;
    let den = i64_field(meta, "fpsDen")? as i32;
    Rational::fps(num, den).ok()
}

fn infer_fps(timeline: &Value) -> Option<Rational> {
    let rate = timeline
        .pointer("/global_start_time/rate")
        .or_else(|| timeline.pointer("/tracks/children/0/children/0/source_range/duration/rate"))?;
    let rate = rate.as_f64().or_else(|| rate.as_i64().map(|n| n as f64))?;
    if (rate - TICKS_PER_SECOND as f64).abs() < 1.0 {
        return Some(Rational { num: 30, den: 1 });
    }
    crate::timeutil::parse_fps_token(&format!("{rate}")).or_else(|| snap_fps(rate))
}

fn snap_fps(rate: f64) -> Option<Rational> {
    const KNOWN: &[(f64, i32, i32)] = &[
        (23.976, 24000, 1001),
        (23.98, 24000, 1001),
        (24.0, 24, 1),
        (25.0, 25, 1),
        (29.97, 30000, 1001),
        (30.0, 30, 1),
        (50.0, 50, 1),
        (59.94, 60000, 1001),
        (60.0, 60, 1),
    ];
    KNOWN
        .iter()
        .find(|(approx, _, _)| (rate - approx).abs() < 0.02)
        .map(|(_, num, den)| Rational {
            num: *num,
            den: *den,
        })
}

fn video_from_meta(media: &Value) -> Option<VideoStreamInfo> {
    let meta = videocut_meta(media)?;
    let width = u32_field(meta, "videoWidth")?;
    let height = u32_field(meta, "videoHeight")?;
    let num = i64_field(meta, "videoFpsNum")? as i32;
    let den = i64_field(meta, "videoFpsDen")? as i32;
    Some(VideoStreamInfo {
        width,
        height,
        fps: Rational::fps(num, den).ok()?,
        codec: string_field(meta, "videoCodec").unwrap_or_else(|| "unknown".into()),
    })
}

fn audio_from_meta(media: &Value) -> Option<AudioStreamInfo> {
    let meta = videocut_meta(media)?;
    Some(AudioStreamInfo {
        sample_rate: u32_field(meta, "sampleRate")?,
        channels: u32_field(meta, "channels")? as u16,
        codec: string_field(meta, "audioCodec").unwrap_or_else(|| "unknown".into()),
    })
}
