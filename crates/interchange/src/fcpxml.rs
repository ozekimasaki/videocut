//! FCPXML 1.9. A transition overlaps the tail of the outgoing clip and the
//! head of the incoming clip, so the spine clips overlap by the blend length.
//! Import trims those handles back to the abutting cut used by VideoCut.

use std::collections::HashMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use videocut_model::{
    AudioStreamInfo, ClipKind, MarkerKind, Rational, SequenceSettings, TrackKind, TransitionAlign,
    VideoStreamInfo,
};

use crate::assemble::{
    DraftClip, DraftMarker, DraftProject, DraftSequence, DraftTrack, DraftTransition,
};
use crate::edit::{head_tail, join_after, prepare, PreparedClip, PreparedSequence, PreparedTrack};
use crate::error::Error;
use crate::names::{
    align_name, clip_kind_name, effect_from_label, effect_label, file_url, parse_align,
    parse_clip_kind, parse_marker_kind, path_from_url, xml_escape,
};
use crate::timeutil::{
    format_fcpx_time, frames_to_ticks, is_drop_frame, parse_fcpx_time, scale_ticks,
};

pub fn export(project: &videocut_model::Project) -> Result<(String, Vec<String>), Error> {
    let prepared = prepare(project);
    if prepared.sequences.is_empty() {
        return Err(Error::NoSequence);
    }
    let mut ids = IdGen::default();
    let mut formats = Vec::new();
    let mut assets = Vec::new();
    let mut effects = Vec::new();
    let mut asset_ids: HashMap<String, String> = HashMap::new();
    let mut effect_ids: HashMap<String, String> = HashMap::new();
    let mut projects = Vec::new();
    for sequence in &prepared.sequences {
        let format_id = ids.next();
        formats.push(format_element(sequence, &format_id));
        let project_xml = sequence_project(
            sequence,
            &prepared.name,
            &format_id,
            &mut ids,
            &mut assets,
            &mut asset_ids,
            &mut effects,
            &mut effect_ids,
        );
        projects.push(project_xml);
    }
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE fcpxml>\n");
    xml.push_str("<fcpxml version=\"1.9\">\n  <resources>\n");
    for format in formats {
        xml.push_str("    ");
        xml.push_str(&format);
        xml.push('\n');
    }
    for asset in assets {
        xml.push_str("    ");
        xml.push_str(&asset);
        xml.push('\n');
    }
    for effect in effects {
        xml.push_str("    ");
        xml.push_str(&effect);
        xml.push('\n');
    }
    xml.push_str("  </resources>\n  <library>\n    <event name=\"");
    xml.push_str(&xml_escape(&prepared.name));
    xml.push_str("\">\n");
    for project in projects {
        xml.push_str(&project);
    }
    xml.push_str("    </event>\n  </library>\n</fcpxml>\n");
    Ok((xml, prepared.warnings))
}

pub fn import(text: &str) -> Result<(DraftProject, Vec<String>), Error> {
    let root = parse_tree(text)?;
    if root.name != "fcpxml" {
        return Err(Error::Fcpxml(format!(
            "expected fcpxml root, found '{}'",
            root.name
        )));
    }
    let mut warnings = Vec::new();
    match root.attr("version") {
        Some(version) if version_at_least(version, 1, 9) => {}
        Some(version) => warnings.push(format!(
            "FCPXML version {version} is older than 1.9 and may omit objects"
        )),
        None => warnings.push("FCPXML document has no version".into()),
    }
    let mut formats = HashMap::new();
    let mut assets = HashMap::new();
    let mut effects = HashMap::new();
    if let Some(resources) = root.child("resources") {
        for child in &resources.children {
            match child.name.as_str() {
                "format" => {
                    if let Some(id) = child.attr("id") {
                        formats.insert(id.to_string(), child.clone());
                    }
                }
                "asset" => {
                    if let Some(id) = child.attr("id") {
                        assets.insert(id.to_string(), child.clone());
                    }
                }
                "effect" => {
                    if let Some(id) = child.attr("id") {
                        effects.insert(id.to_string(), child.clone());
                    }
                }
                _ => {}
            }
        }
    }
    let mut found = Vec::new();
    collect_sequences(&root, None, None, &mut found);
    if found.is_empty() {
        return Err(Error::Fcpxml("the document has no sequence".into()));
    }
    let project_name = found
        .iter()
        .find_map(|item| item.event_name.clone())
        .unwrap_or_else(|| "Untitled".into());
    let mut sequences = Vec::new();
    for item in found {
        sequences.push(import_sequence(
            &item,
            &formats,
            &assets,
            &effects,
            &mut warnings,
        )?);
    }
    Ok((
        DraftProject {
            name: project_name,
            sequences,
            warnings: Vec::new(),
        },
        warnings,
    ))
}

struct FoundSequence<'a> {
    event_name: Option<String>,
    project_name: Option<String>,
    node: &'a XmlNode,
}

fn collect_sequences<'a>(
    node: &'a XmlNode,
    event_name: Option<String>,
    project_name: Option<String>,
    out: &mut Vec<FoundSequence<'a>>,
) {
    let event_name = if node.name == "event" {
        node.attr("name").map(|name| name.to_string())
    } else {
        event_name
    };
    let project_name = if node.name == "project" {
        node.attr("name").map(|name| name.to_string())
    } else {
        project_name
    };
    if node.name == "sequence" {
        out.push(FoundSequence {
            event_name,
            project_name,
            node,
        });
        return;
    }
    for child in &node.children {
        collect_sequences(child, event_name.clone(), project_name.clone(), out);
    }
}

fn import_sequence(
    found: &FoundSequence<'_>,
    formats: &HashMap<String, XmlNode>,
    assets: &HashMap<String, XmlNode>,
    effects: &HashMap<String, XmlNode>,
    warnings: &mut Vec<String>,
) -> Result<DraftSequence, Error> {
    let node = found.node;
    let format = node.attr("format").and_then(|id| formats.get(id));
    let (width, height, fps) = format_info(format);
    let mut fps = fps;
    if node
        .attr("tcFormat")
        .map(|value| value.eq_ignore_ascii_case("DF"))
        .unwrap_or(false)
        && fps.den == 1
        && (fps.num == 30 || fps.num == 60)
    {
        fps = Rational {
            num: fps.num * 1000,
            den: 1001,
        };
    }
    let sample_rate = node
        .attr("audioRate")
        .map(parse_audio_rate)
        .unwrap_or(48_000);
    let audio_channels = match node.attr("audioLayout") {
        Some("mono") => 1,
        Some("surround") => 6,
        _ => 2,
    };
    let start = node
        .attr("tcStart")
        .map(parse_fcpx_time)
        .transpose()?
        .unwrap_or(0);
    let spine = node
        .child("spine")
        .ok_or_else(|| Error::Fcpxml("sequence is missing a spine".into()))?;
    let mut spine_items = Vec::new();
    let mut anchored = Vec::new();
    for child in &spine.children {
        absorb_item(
            child,
            false,
            &mut spine_items,
            &mut anchored,
            assets,
            effects,
            warnings,
        )?;
    }
    assign_spine_offsets(&mut spine_items);
    let spine_is_audio = spine_items.iter().any(|item| item.element == "audio")
        && spine_items
            .iter()
            .all(|item| matches!(item.element.as_str(), "audio" | "transition" | "gap"));
    let mut grouped: HashMap<(u8, i32), Vec<TimedItem>> = HashMap::new();
    for item in spine_items {
        let key = spine_key(&item, spine_is_audio);
        grouped.entry(key).or_default().push(item);
    }
    for item in anchored {
        let key = anchored_key(&item);
        grouped.entry(key).or_default().push(item);
    }
    let mut keys: Vec<_> = grouped.keys().copied().collect();
    keys.sort();
    let mut tracks = Vec::new();
    let mut markers = Vec::new();
    for key in keys {
        let items = grouped.remove(&key).unwrap_or_default();
        let (clips, transitions, track_markers) = canonicalize_lane(items, warnings);
        markers.extend(track_markers);
        tracks.push(DraftTrack {
            kind: if key.0 == 0 {
                TrackKind::Video
            } else {
                TrackKind::Audio
            },
            clips,
            transitions,
        });
    }
    Ok(DraftSequence {
        name: found
            .project_name
            .clone()
            .unwrap_or_else(|| "Sequence".into()),
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
    })
}

fn spine_key(item: &TimedItem, spine_is_audio: bool) -> (u8, i32) {
    if item.element == "audio" || (spine_is_audio && item.element == "transition") {
        (1, 0)
    } else {
        (0, 0)
    }
}

fn anchored_key(item: &TimedItem) -> (u8, i32) {
    if item.lane > 0 {
        (0, item.lane)
    } else if item.lane < 0 {
        (1, -item.lane - 1)
    } else if item.element == "audio" {
        (1, 0)
    } else {
        (0, 0)
    }
}

#[derive(Clone)]
struct TimedItem {
    element: String,
    name: String,
    offset: i64,
    start: i64,
    duration: i64,
    lane: i32,
    path: String,
    media_name: String,
    media_duration: i64,
    video: Option<VideoStreamInfo>,
    audio: Option<AudioStreamInfo>,
    effect_id: String,
    alignment: TransitionAlign,
    markers: Vec<RawMarker>,
    speed: f64,
    reversed: bool,
    enabled: bool,
    kind: ClipKind,
}

#[derive(Clone)]
struct RawMarker {
    start: i64,
    duration: i64,
    name: String,
    comment: String,
    color: u8,
    kind: MarkerKind,
}

fn absorb_item(
    node: &XmlNode,
    anchored_child: bool,
    spine: &mut Vec<TimedItem>,
    anchored: &mut Vec<TimedItem>,
    assets: &HashMap<String, XmlNode>,
    effects: &HashMap<String, XmlNode>,
    warnings: &mut Vec<String>,
) -> Result<(), Error> {
    if !matches!(
        node.name.as_str(),
        "asset-clip" | "clip" | "audio" | "video" | "gap" | "transition" | "ref-clip" | "title"
    ) {
        return Ok(());
    }
    let item = timed_from_node(node, assets, effects, warnings)?;
    let lane = item.lane;
    let is_spine = !anchored_child && lane == 0;
    if is_spine {
        spine.push(item);
    } else {
        anchored.push(item);
    }
    if matches!(
        node.name.as_str(),
        "asset-clip" | "clip" | "audio" | "video" | "gap" | "ref-clip" | "title"
    ) {
        for child in &node.children {
            absorb_item(child, true, spine, anchored, assets, effects, warnings)?;
        }
    }
    Ok(())
}

fn timed_from_node(
    node: &XmlNode,
    assets: &HashMap<String, XmlNode>,
    effects: &HashMap<String, XmlNode>,
    warnings: &mut Vec<String>,
) -> Result<TimedItem, Error> {
    let asset = node.attr("ref").and_then(|id| assets.get(id));
    let effect = node.attr("ref").and_then(|id| effects.get(id));
    let duration = node
        .attr("duration")
        .map(parse_fcpx_time)
        .transpose()?
        .unwrap_or(0);
    let start = node
        .attr("start")
        .map(parse_fcpx_time)
        .transpose()?
        .unwrap_or(0);
    let offset = node.attr("offset").map(parse_fcpx_time).transpose()?;
    let (speed, reversed) = speed_of(node, warnings)?;
    let path = asset
        .and_then(|asset| asset.attr("src"))
        .map(path_from_url)
        .unwrap_or_default();
    let media_name = asset
        .and_then(|asset| asset.attr("name"))
        .unwrap_or_else(|| node.attr("name").unwrap_or("Clip"))
        .to_string();
    let media_duration = asset
        .and_then(|asset| asset.attr("duration"))
        .map(parse_fcpx_time)
        .transpose()?
        .unwrap_or(0);
    let markers = node
        .children
        .iter()
        .filter(|child| child.name == "marker" || child.name == "chapter-marker")
        .map(|child| raw_marker(child))
        .collect::<Result<Vec<_>, _>>()?;
    let effect_id = if node.name == "transition" {
        transition_effect(node, effect)
    } else {
        String::new()
    };
    let alignment = node
        .attr("align")
        .map(parse_align)
        .unwrap_or(TransitionAlign::Center);
    let kind = node
        .attr("videocutKind")
        .map(parse_clip_kind)
        .unwrap_or(if node.name == "title" {
            ClipKind::Title
        } else {
            ClipKind::Media
        });
    Ok(TimedItem {
        element: node.name.clone(),
        name: node
            .attr("name")
            .unwrap_or(if node.name == "gap" { "Gap" } else { "Clip" })
            .to_string(),
        offset: offset.unwrap_or(i64::MIN),
        start,
        duration,
        lane: node
            .attr("lane")
            .and_then(|lane| lane.parse().ok())
            .unwrap_or(0),
        path,
        media_name,
        media_duration,
        video: None,
        audio: None,
        effect_id,
        alignment,
        markers,
        speed,
        reversed,
        enabled: node
            .attr("enabled")
            .map(|value| value != "0" && value != "false")
            .unwrap_or(true),
        kind,
    })
}

fn raw_marker(node: &XmlNode) -> Result<RawMarker, Error> {
    Ok(RawMarker {
        start: node
            .attr("start")
            .map(parse_fcpx_time)
            .transpose()?
            .unwrap_or(0),
        duration: node
            .attr("duration")
            .map(parse_fcpx_time)
            .transpose()?
            .unwrap_or(0),
        name: node.attr("value").unwrap_or("").to_string(),
        comment: node.attr("note").unwrap_or("").to_string(),
        color: node
            .attr("videocutColor")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
        kind: if node.name == "chapter-marker" {
            MarkerKind::Chapter
        } else {
            node.attr("videocutKind")
                .map(parse_marker_kind)
                .unwrap_or(MarkerKind::Comment)
        },
    })
}

fn transition_effect(node: &XmlNode, effect: Option<&XmlNode>) -> String {
    if let Some(id) = node.attr("effectId") {
        if id.starts_with("builtin.") {
            return id.to_string();
        }
    }
    if let Some(effect) = effect {
        if let Some(uid) = effect.attr("uid") {
            if let Some(rest) = uid.strip_prefix("videocut.") {
                return rest.to_string();
            }
        }
        if let Some(name) = effect.attr("name") {
            return effect_from_label(name);
        }
    }
    effect_from_label(node.attr("name").unwrap_or("Cross Dissolve"))
}

fn speed_of(node: &XmlNode, warnings: &mut Vec<String>) -> Result<(f64, bool), Error> {
    let Some(map) = node.child("timeMap") else {
        return Ok((1.0, false));
    };
    let mut points = Vec::new();
    for child in &map.children {
        if child.name != "timept" {
            continue;
        }
        let time = child
            .attr("time")
            .map(parse_fcpx_time)
            .transpose()?
            .unwrap_or(0);
        let value = child
            .attr("value")
            .map(parse_fcpx_time)
            .transpose()?
            .unwrap_or(0);
        points.push((time, value));
    }
    if points.len() < 2 {
        warnings.push(format!(
            "clip '{}' has a time map without two points",
            node.attr("name").unwrap_or("clip")
        ));
        return Ok((1.0, false));
    }
    let (t0, v0) = points[0];
    let (t1, v1) = points[points.len() - 1];
    let dt = t1 - t0;
    if dt == 0 {
        return Ok((1.0, false));
    }
    let speed = (v1 - v0) as f64 / dt as f64;
    if speed < 0.0 {
        Ok((-speed, true))
    } else {
        Ok((speed, false))
    }
}

fn assign_spine_offsets(items: &mut [TimedItem]) {
    let mut cursor = 0_i64;
    for item in items.iter_mut() {
        if item.element == "transition" {
            let offset = if item.offset == i64::MIN {
                cursor.saturating_sub(item.duration)
            } else {
                item.offset
            };
            item.offset = offset;
            cursor = offset;
        } else {
            let offset = if item.offset == i64::MIN {
                cursor
            } else {
                item.offset
            };
            item.offset = offset;
            cursor = offset + item.duration;
        }
    }
}

fn canonicalize_lane(
    mut items: Vec<TimedItem>,
    warnings: &mut Vec<String>,
) -> (Vec<DraftClip>, Vec<DraftTransition>, Vec<DraftMarker>) {
    items.sort_by_key(|item| item.offset);
    let mut markers = Vec::new();
    for item in &items {
        if item.element == "transition" {
            continue;
        }
        for marker in &item.markers {
            markers.push(marker_at(item, marker));
        }
    }
    let transitions: Vec<TimedItem> = items
        .iter()
        .filter(|item| item.element == "transition")
        .cloned()
        .collect();
    let clips: Vec<TimedItem> = items
        .into_iter()
        .filter(|item| item.element != "transition" && item.element != "gap")
        .collect();
    let mut heads = vec![0_i64; clips.len()];
    let mut tails = vec![0_i64; clips.len()];
    let mut draft_transitions = Vec::new();
    for transition in transitions {
        let start = transition.offset;
        let end = transition.offset + transition.duration;
        let mut overlapping: Vec<usize> = clips
            .iter()
            .enumerate()
            .filter(|(_, clip)| clip.offset < end && clip.offset + clip.duration > start)
            .map(|(index, _)| index)
            .collect();
        overlapping.sort_by_key(|index| clips[*index].offset);
        if overlapping.len() < 2 {
            warnings.push(format!(
                "transition '{}' does not join two clips",
                transition.name
            ));
            continue;
        }
        let outgoing = overlapping[0];
        let incoming = overlapping[overlapping.len() - 1];
        let cut = match transition.alignment {
            TransitionAlign::Start => start,
            TransitionAlign::End => end,
            TransitionAlign::Center => start + transition.duration / 2,
        };
        let outgoing_end = clips[outgoing].offset + clips[outgoing].duration;
        tails[outgoing] = (outgoing_end - cut).max(0);
        heads[incoming] = (cut - clips[incoming].offset).max(0);
        draft_transitions.push(DraftTransition {
            effect_id: transition.effect_id,
            center_ticks: cut,
            duration_ticks: transition.duration.max(0),
            alignment: transition.alignment,
        });
    }
    let mut draft_clips = Vec::new();
    for (index, clip) in clips.iter().enumerate() {
        let head = heads[index];
        let tail = tails[index];
        let timeline_in = clip.offset + head;
        let timeline_duration = clip.duration - head - tail;
        if timeline_duration <= 0 {
            warnings.push(format!(
                "clip '{}' was fully covered by a transition",
                clip.name
            ));
            continue;
        }
        let source_in = clip.start + scale_ticks(head, clip.speed);
        let source_out = clip.start + scale_ticks(clip.duration - tail, clip.speed);
        draft_clips.push(DraftClip {
            name: clip.name.clone(),
            media_name: clip.media_name.clone(),
            path: clip.path.clone(),
            media_duration: clip.media_duration,
            video: clip.video.clone(),
            audio: clip.audio.clone(),
            timeline_in,
            timeline_out: timeline_in + timeline_duration,
            source_in,
            source_out,
            speed: clip.speed,
            reversed: clip.reversed,
            enabled: clip.enabled,
            kind: clip.kind,
            label_color: 0,
        });
    }
    (draft_clips, draft_transitions, markers)
}

fn marker_at(item: &TimedItem, marker: &RawMarker) -> DraftMarker {
    let delta = marker.start - item.start;
    let local = if item.speed.abs() < 1e-9 {
        0
    } else {
        scale_ticks(delta, 1.0 / item.speed)
    };
    DraftMarker {
        time_ticks: item.offset + local,
        duration_ticks: marker.duration,
        name: marker.name.clone(),
        comment: marker.comment.clone(),
        color: marker.color,
        kind: marker.kind,
    }
}

fn format_info(format: Option<&XmlNode>) -> (u32, u32, Rational) {
    let Some(format) = format else {
        return (1920, 1080, Rational { num: 30, den: 1 });
    };
    let width = format
        .attr("width")
        .and_then(|value| value.parse().ok())
        .unwrap_or(1920);
    let height = format
        .attr("height")
        .and_then(|value| value.parse().ok())
        .unwrap_or(1080);
    let fps = format
        .attr("frameDuration")
        .and_then(|value| parse_fcpx_time(value).ok())
        .map(fps_from_frame_ticks)
        .unwrap_or(Rational { num: 30, den: 1 });
    (width, height, fps)
}

fn fps_from_frame_ticks(ticks: i64) -> Rational {
    if ticks <= 0 {
        return Rational { num: 30, den: 1 };
    }
    let ticks_per_second = i128::from(videocut_model::TICKS_PER_SECOND);
    let g = crate::timeutil::gcd(ticks_per_second, i128::from(ticks));
    let num = ticks_per_second / g;
    let den = i128::from(ticks) / g;
    if num > 0 && den > 0 && num <= i128::from(i32::MAX) && den <= i128::from(i32::MAX) {
        Rational {
            num: num as i32,
            den: den as i32,
        }
    } else {
        Rational { num: 30, den: 1 }
    }
}

fn sequence_project(
    sequence: &PreparedSequence,
    _project_name: &str,
    format_id: &str,
    ids: &mut IdGen,
    assets: &mut Vec<String>,
    asset_ids: &mut HashMap<String, String>,
    effects: &mut Vec<String>,
    effect_ids: &mut HashMap<String, String>,
) -> String {
    let fps = sequence.settings.fps;
    let primary = sequence
        .video_tracks
        .first()
        .map(|track| (TrackKind::Video, track))
        .or_else(|| {
            sequence
                .audio_tracks
                .first()
                .map(|track| (TrackKind::Audio, track))
        });
    let mut spine = Vec::new();
    let mut end = 0_i64;
    if let Some((kind, track)) = primary {
        let mut cursor = 0_i64;
        for (index, clip) in track.clips.iter().enumerate() {
            let (head, tail) = head_tail(track, index);
            let offset = clip.timeline_in - head;
            let duration = (clip.timeline_out - clip.timeline_in) + head + tail;
            if offset > cursor {
                spine.push(gap_element(cursor, offset - cursor, fps, &[]));
            }
            let asset_id = asset_id_for(clip, kind, format_id, ids, assets, asset_ids, fps);
            let markers = markers_in(
                sequence,
                clip.timeline_in,
                clip.timeline_out,
                fps,
                clip,
                head,
            );
            spine.push(clip_element(
                kind, clip, &asset_id, offset, duration, head, &markers, None,
            ));
            cursor = offset + duration;
            end = end.max(cursor);
            if let Some(join) = join_after(track, index) {
                let effect_id = effect_id_for(&join.effect_id, ids, effects, effect_ids);
                let transition_offset = clip.timeline_out - join.in_offset;
                spine.push(format!(
                    "<transition ref=\"{effect_id}\" name=\"{}\" offset=\"{}\" duration=\"{}\" effectId=\"{}\" align=\"{}\"/>",
                    xml_escape(effect_label(&join.effect_id)),
                    format_fcpx_time(transition_offset),
                    format_fcpx_time(join.duration_ticks),
                    xml_escape(&join.effect_id),
                    align_name(join.alignment)
                ));
            }
        }
        for (track_index, extra) in sequence.video_tracks.iter().enumerate().skip(1) {
            attach_lane(
                &mut spine,
                extra,
                TrackKind::Video,
                track_index as i32,
                format_id,
                ids,
                assets,
                asset_ids,
                fps,
            );
        }
        for (track_index, extra) in sequence.audio_tracks.iter().enumerate() {
            if kind == TrackKind::Audio && track_index == 0 {
                continue;
            }
            let lane = -1 - track_index as i32;
            attach_lane(
                &mut spine,
                extra,
                TrackKind::Audio,
                lane,
                format_id,
                ids,
                assets,
                asset_ids,
                fps,
            );
        }
    }
    for marker in &sequence.markers {
        if marker.time_ticks >= end {
            end = marker.time_ticks + marker.duration_ticks.max(frames_to_ticks(1, fps));
        }
    }
    let duration = end.max(frames_to_ticks(1, fps));
    let tc_format = if is_drop_frame(fps) { "DF" } else { "NDF" };
    let mut body = format!(
        "      <project name=\"{}\">\n        <sequence format=\"{format_id}\" duration=\"{}\" tcStart=\"{}\" tcFormat=\"{tc_format}\" audioRate=\"{}\" audioLayout=\"{}\">\n          <spine>\n",
        xml_escape(&sequence.name),
        format_fcpx_time(duration),
        format_fcpx_time(sequence.settings.start_timecode_ticks),
        audio_rate_token(sequence.settings.sample_rate),
        if sequence.settings.audio_channels <= 1 { "mono" } else { "stereo" }
    );
    for child in spine {
        body.push_str("            ");
        body.push_str(&child);
        body.push('\n');
    }
    body.push_str("          </spine>\n        </sequence>\n      </project>\n");
    body
}

fn attach_lane(
    spine: &mut Vec<String>,
    track: &PreparedTrack,
    kind: TrackKind,
    lane: i32,
    format_id: &str,
    ids: &mut IdGen,
    assets: &mut Vec<String>,
    asset_ids: &mut HashMap<String, String>,
    fps: Rational,
) {
    for clip in &track.clips {
        let asset_id = asset_id_for(clip, kind, format_id, ids, assets, asset_ids, fps);
        let duration = clip.timeline_out - clip.timeline_in;
        let element = clip_element(
            kind,
            clip,
            &asset_id,
            clip.timeline_in,
            duration,
            0,
            &[],
            Some(lane),
        );
        if spine.is_empty() {
            spine.push(gap_element(0, duration.max(clip.timeline_out), fps, &[]));
        }
        let host = spine
            .iter()
            .rposition(|item| {
                item.starts_with("<asset-clip")
                    || item.starts_with("<audio")
                    || item.starts_with("<gap")
            })
            .unwrap_or(0);
        insert_child(&mut spine[host], &element);
    }
}

fn insert_child(parent: &mut String, child: &str) {
    if let Some(stripped) = parent.strip_suffix("/>") {
        *parent = format!(
            "{stripped}>\n              {child}\n            </{}>",
            open_name(parent)
        );
    } else if let Some(index) = parent.rfind("</") {
        parent.insert_str(index, &format!("  {child}\n            "));
    }
}

fn open_name(element: &str) -> &str {
    element
        .trim_start_matches('<')
        .split(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/')
        .next()
        .unwrap_or("asset-clip")
}

fn markers_in(
    sequence: &PreparedSequence,
    start: i64,
    end: i64,
    _fps: Rational,
    clip: &PreparedClip,
    head: i64,
) -> Vec<String> {
    sequence
        .markers
        .iter()
        .filter(|marker| marker.time_ticks >= start && marker.time_ticks < end)
        .map(|marker| {
            let into_clip = marker.time_ticks - clip.timeline_in;
            let source = clip.source_in + scale_ticks(into_clip, clip.speed);
            let _ = head;
            let tag = if marker.kind == MarkerKind::Chapter {
                "chapter-marker"
            } else {
                "marker"
            };
            format!(
                "<{tag} start=\"{}\" duration=\"{}\" value=\"{}\" note=\"{}\" videocutKind=\"{}\" videocutColor=\"{}\"/>",
                format_fcpx_time(source),
                format_fcpx_time(marker.duration_ticks),
                xml_escape(&marker.name),
                xml_escape(&marker.comment),
                crate::names::marker_kind_name(marker.kind),
                marker.color
            )
        })
        .collect()
}

fn clip_element(
    kind: TrackKind,
    clip: &PreparedClip,
    asset_id: &str,
    offset: i64,
    duration: i64,
    head: i64,
    markers: &[String],
    lane: Option<i32>,
) -> String {
    let name = match kind {
        TrackKind::Video => "asset-clip",
        TrackKind::Audio => "audio",
    };
    let source_in = clip.source_in - scale_ticks(head, clip.speed);
    let mut attrs = format!(
        "ref=\"{asset_id}\" name=\"{}\" offset=\"{}\" start=\"{}\" duration=\"{}\"",
        xml_escape(&clip.name),
        format_fcpx_time(offset),
        format_fcpx_time(source_in),
        format_fcpx_time(duration)
    );
    if let Some(lane) = lane {
        attrs.push_str(&format!(" lane=\"{lane}\""));
    }
    if clip.kind != ClipKind::Media {
        attrs.push_str(&format!(" videocutKind=\"{}\"", clip_kind_name(clip.kind)));
    }
    if !clip.enabled {
        attrs.push_str(" enabled=\"0\"");
    }
    let time_map = if clip.reversed || (clip.speed - 1.0).abs() > 1e-9 {
        let scalar = if clip.reversed {
            -clip.speed
        } else {
            clip.speed
        };
        let value = scale_ticks(duration, scalar);
        Some(format!(
            "<timeMap><timept time=\"0s\" value=\"0s\" interp=\"smooth2\"/><timept time=\"{}\" value=\"{}\" interp=\"smooth2\"/></timeMap>",
            format_fcpx_time(duration),
            format_fcpx_time(value)
        ))
    } else {
        None
    };
    if markers.is_empty() && time_map.is_none() {
        format!("<{name} {attrs}/>")
    } else {
        let mut body = format!("<{name} {attrs}>");
        for marker in markers {
            body.push_str(marker);
        }
        if let Some(time_map) = time_map {
            body.push_str(&time_map);
        }
        body.push_str(&format!("</{name}>"));
        body
    }
}

fn gap_element(offset: i64, duration: i64, _fps: Rational, markers: &[String]) -> String {
    if markers.is_empty() {
        format!(
            "<gap name=\"Gap\" offset=\"{}\" start=\"0s\" duration=\"{}\"/>",
            format_fcpx_time(offset),
            format_fcpx_time(duration)
        )
    } else {
        let mut body = format!(
            "<gap name=\"Gap\" offset=\"{}\" start=\"0s\" duration=\"{}\">",
            format_fcpx_time(offset),
            format_fcpx_time(duration)
        );
        for marker in markers {
            body.push_str(marker);
        }
        body.push_str("</gap>");
        body
    }
}

fn asset_id_for(
    clip: &PreparedClip,
    kind: TrackKind,
    format_id: &str,
    ids: &mut IdGen,
    assets: &mut Vec<String>,
    asset_ids: &mut HashMap<String, String>,
    fps: Rational,
) -> String {
    let key = if clip.path.is_empty() {
        format!("name:{}", clip.media_name)
    } else {
        clip.path.clone()
    };
    if let Some(id) = asset_ids.get(&key) {
        return id.clone();
    }
    let id = ids.next();
    let name = if clip.media_name.is_empty() {
        clip.name.as_str()
    } else {
        clip.media_name.as_str()
    };
    let has_video = clip.video.is_some() || kind == TrackKind::Video;
    let has_audio = clip.audio.is_some() || kind == TrackKind::Audio;
    let duration = clip
        .media_duration
        .max(clip.source_out)
        .max(frames_to_ticks(1, fps));
    assets.push(format!(
        "<asset id=\"{id}\" name=\"{}\" src=\"{}\" start=\"0s\" duration=\"{}\" hasVideo=\"{}\" hasAudio=\"{}\" format=\"{format_id}\"/>",
        xml_escape(name),
        xml_escape(&file_url(&clip.path)),
        format_fcpx_time(duration),
        if has_video { "1" } else { "0" },
        if has_audio { "1" } else { "0" }
    ));
    asset_ids.insert(key, id.clone());
    id
}

fn effect_id_for(
    effect_id: &str,
    ids: &mut IdGen,
    effects: &mut Vec<String>,
    effect_ids: &mut HashMap<String, String>,
) -> String {
    if let Some(id) = effect_ids.get(effect_id) {
        return id.clone();
    }
    let id = ids.next();
    effects.push(format!(
        "<effect id=\"{id}\" name=\"{}\" uid=\"videocut.{}\"/>",
        xml_escape(effect_label(effect_id)),
        xml_escape(effect_id)
    ));
    effect_ids.insert(effect_id.to_string(), id.clone());
    id
}

fn format_element(sequence: &PreparedSequence, id: &str) -> String {
    let fps = sequence.settings.fps;
    let frame = frames_to_ticks(1, fps);
    format!(
        "<format id=\"{id}\" name=\"{}\" frameDuration=\"{}\" width=\"{}\" height=\"{}\"/>",
        xml_escape(&format_name(sequence)),
        format_fcpx_time(frame),
        sequence.settings.width,
        sequence.settings.height
    )
}

fn format_name(sequence: &PreparedSequence) -> String {
    let fps = &sequence.settings.fps;
    let rate = if fps.den == 1 {
        fps.num.to_string()
    } else if fps.num == 30000 && fps.den == 1001 {
        "29.97".into()
    } else if fps.num == 24000 && fps.den == 1001 {
        "23.98".into()
    } else if fps.num == 60000 && fps.den == 1001 {
        "59.94".into()
    } else {
        format!("{}", fps.num)
    };
    let height = sequence.settings.height;
    if matches!(height, 720 | 1080 | 2160) {
        format!("FFVideoFormat{height}p{rate}")
    } else {
        format!("FFVideoFormat{}x{height}", sequence.settings.width)
    }
}

fn audio_rate_token(rate: u32) -> String {
    match rate {
        48_000 => "48k".into(),
        44_100 => "44.1k".into(),
        96_000 => "96k".into(),
        other => other.to_string(),
    }
}

fn parse_audio_rate(text: &str) -> u32 {
    match text {
        "48k" => 48_000,
        "44.1k" => 44_100,
        "96k" => 96_000,
        "88.2k" => 88_200,
        other => other.trim_end_matches('k').parse().unwrap_or(48_000),
    }
}

fn version_at_least(version: &str, major: u32, minor: u32) -> bool {
    let mut parts = version.split('.');
    let parsed_major = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    let parsed_minor = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    (parsed_major, parsed_minor) >= (major, minor)
}

#[derive(Default)]
struct IdGen {
    next: u32,
}

impl IdGen {
    fn next(&mut self) -> String {
        self.next += 1;
        format!("r{}", self.next)
    }
}

#[derive(Clone)]
struct XmlNode {
    name: String,
    attrs: HashMap<String, String>,
    children: Vec<XmlNode>,
}

impl XmlNode {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.get(key).map(String::as_str)
    }

    fn child(&self, name: &str) -> Option<&XmlNode> {
        self.children.iter().find(|child| child.name == name)
    }
}

fn parse_tree(text: &str) -> Result<XmlNode, Error> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut stack: Vec<XmlNode> = Vec::new();
    loop {
        let event = reader
            .read_event_into(&mut buf)
            .map_err(|err| Error::Fcpxml(err.to_string()))?;
        match event {
            Event::Start(start) => stack.push(node_from_start(&start)?),
            Event::Empty(start) => {
                let node = node_from_start(&start)?;
                push_child(&mut stack, node)?;
            }
            Event::End(_) => {
                let node = stack
                    .pop()
                    .ok_or_else(|| Error::Fcpxml("unexpected end tag".into()))?;
                if stack.is_empty() {
                    return Ok(node);
                }
                push_child(&mut stack, node)?;
            }
            Event::Eof => {
                return stack
                    .pop()
                    .ok_or_else(|| Error::Fcpxml("document has no root element".into()));
            }
            _ => {}
        }
        buf.clear();
    }
}

fn push_child(stack: &mut [XmlNode], node: XmlNode) -> Result<(), Error> {
    stack
        .last_mut()
        .ok_or_else(|| Error::Fcpxml("element is outside the root".into()))?
        .children
        .push(node);
    Ok(())
}

fn node_from_start(start: &BytesStart<'_>) -> Result<XmlNode, Error> {
    let name = local_name(start.name().as_ref());
    let mut attrs = HashMap::new();
    for attr in start.attributes() {
        let attr = attr.map_err(|err| Error::Fcpxml(err.to_string()))?;
        let key = local_name(attr.key.as_ref());
        let value = attr
            .unescape_value()
            .map_err(|err| Error::Fcpxml(err.to_string()))?
            .into_owned();
        attrs.insert(key, value);
    }
    Ok(XmlNode {
        name,
        attrs,
        children: Vec::new(),
    })
}

fn local_name(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    text.rsplit(':').next().unwrap_or(&text).to_string()
}
