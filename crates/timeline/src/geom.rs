//! Clip geometry: splits, ripples, links, and source mapping.

use std::collections::HashSet;

use videocut_model::{new_id, Clip, Sequence, Track};

use crate::error::EditError;

/// Source time that plays at `timeline_ticks`.
///
/// Forward clips advance from `source_in`. Reversed clips start at `source_out`
/// and walk backward. A speed of zero holds `source_in`.
pub fn source_at(clip: &Clip, timeline_ticks: i64) -> i64 {
    let elapsed = (timeline_ticks - clip.timeline_in).max(0);
    let advanced = scale_by_speed(elapsed, clip.speed);
    if clip.reversed {
        clip.source_out.saturating_sub(advanced)
    } else {
        clip.source_in.saturating_add(advanced)
    }
}

pub(crate) fn scale_by_speed(ticks: i64, speed: f64) -> i64 {
    if ticks == 0 || speed == 0.0 {
        return 0;
    }
    (ticks as f64 * speed).round() as i64
}

pub(crate) fn tracks(seq: &Sequence) -> impl Iterator<Item = &Track> {
    seq.video_tracks.iter().chain(seq.audio_tracks.iter())
}

pub(crate) fn track_ref<'a>(seq: &'a Sequence, id: &str) -> Result<&'a Track, EditError> {
    tracks(seq)
        .find(|track| track.id == id)
        .ok_or_else(|| EditError::TrackNotFound(id.to_string()))
}

pub(crate) fn track_mut<'a>(seq: &'a mut Sequence, id: &str) -> Result<&'a mut Track, EditError> {
    seq.video_tracks
        .iter_mut()
        .chain(seq.audio_tracks.iter_mut())
        .find(|track| track.id == id)
        .ok_or_else(|| EditError::TrackNotFound(id.to_string()))
}

pub(crate) fn clip_ref<'a>(seq: &'a Sequence, id: &str) -> Result<&'a Clip, EditError> {
    tracks(seq)
        .find_map(|track| track.clips.iter().find(|clip| clip.id == id))
        .ok_or_else(|| EditError::ClipNotFound(id.to_string()))
}

pub(crate) fn clip_mut<'a>(seq: &'a mut Sequence, id: &str) -> Result<&'a mut Clip, EditError> {
    seq.video_tracks
        .iter_mut()
        .chain(seq.audio_tracks.iter_mut())
        .find_map(|track| track.clips.iter_mut().find(|clip| clip.id == id))
        .ok_or_else(|| EditError::ClipNotFound(id.to_string()))
}

pub(crate) fn track_id_of(seq: &Sequence, clip_id: &str) -> Result<String, EditError> {
    tracks(seq)
        .find(|track| track.clips.iter().any(|clip| clip.id == clip_id))
        .map(|track| track.id.clone())
        .ok_or_else(|| EditError::ClipNotFound(clip_id.to_string()))
}

pub(crate) fn clip_exists(seq: &Sequence, id: &str) -> bool {
    tracks(seq).any(|track| track.clips.iter().any(|clip| clip.id == id))
}

pub(crate) fn commit<T>(
    seq: &mut Sequence,
    edit: impl FnOnce(&mut Sequence) -> Result<T, EditError>,
) -> Result<T, EditError> {
    let mut draft = seq.clone();
    let value = edit(&mut draft)?;
    ensure_sequence(&draft)?;
    *seq = draft;
    Ok(value)
}

pub(crate) fn sort_clips(track: &mut Track) {
    track.clips.sort_by(|a, b| {
        a.timeline_in
            .cmp(&b.timeline_in)
            .then(a.timeline_out.cmp(&b.timeline_out))
            .then(a.id.cmp(&b.id))
    });
}

pub(crate) fn ensure_no_overlap(track: &Track) -> Result<(), EditError> {
    let mut clips: Vec<&Clip> = track.clips.iter().collect();
    clips.sort_by_key(|clip| (clip.timeline_in, clip.timeline_out));
    for pair in clips.windows(2) {
        if pair[0].timeline_out > pair[1].timeline_in {
            return Err(EditError::Overlap);
        }
    }
    Ok(())
}

pub(crate) fn ensure_sequence(seq: &Sequence) -> Result<(), EditError> {
    for track in tracks(seq) {
        ensure_no_overlap(track)?;
        for clip in &track.clips {
            if clip.timeline_in < 0 {
                return Err(EditError::InvalidTime);
            }
            if clip.timeline_out <= clip.timeline_in {
                return Err(EditError::ZeroDuration);
            }
        }
    }
    Ok(())
}

pub(crate) fn shift_clip(clip: &mut Clip, delta: i64) -> Result<(), EditError> {
    if delta == 0 {
        return Ok(());
    }
    let new_in = clip
        .timeline_in
        .checked_add(delta)
        .ok_or(EditError::InvalidTime)?;
    let new_out = clip
        .timeline_out
        .checked_add(delta)
        .ok_or(EditError::InvalidTime)?;
    if new_in < 0 || new_out <= new_in {
        return Err(EditError::InvalidTime);
    }
    clip.timeline_in = new_in;
    clip.timeline_out = new_out;
    Ok(())
}

/// Split `clip` at a timeline time strictly inside it.
/// The left piece keeps the id. The right piece is a new clip with no links.
pub(crate) fn split_owned(clip: Clip, at: i64) -> Result<(Clip, Clip), EditError> {
    if at <= clip.timeline_in || at >= clip.timeline_out {
        return Err(EditError::NotUnderPlayhead);
    }
    let mut left = clip.clone();
    let mut right = clip;
    right.id = new_id("clip");
    right.linked_ids.clear();
    left.timeline_out = at;
    right.timeline_in = at;
    let cut = source_at(&left, at);
    if left.reversed {
        left.source_in = cut;
        right.source_out = cut;
    } else {
        left.source_out = cut;
        right.source_in = cut;
    }
    if left.speed != 0.0 && left.source_out <= left.source_in {
        return Err(EditError::InsufficientSource);
    }
    if right.speed != 0.0 && right.source_out <= right.source_in {
        return Err(EditError::InsufficientSource);
    }
    Ok((left, right))
}

pub(crate) struct RangeCut {
    pub removed: Vec<Clip>,
    pub split_pairs: Vec<(String, String)>,
}

/// Remove material in `[start, end)`, keeping a gap.
///
/// A clip that spans the whole range is split. Its left piece keeps the id and
/// the right piece is returned in `split_pairs` so linked picture and sound
/// can be joined again.
pub(crate) fn clear_range(
    track: &mut Track,
    start: i64,
    end: i64,
    ignore: &[String],
) -> Result<RangeCut, EditError> {
    if end <= start {
        return Err(EditError::InvalidRange);
    }
    let original = std::mem::take(&mut track.clips);
    let mut kept = Vec::with_capacity(original.len());
    let mut removed = Vec::new();
    let mut split_pairs = Vec::new();
    for clip in original {
        if ignore.iter().any(|id| id == &clip.id) {
            kept.push(clip);
            continue;
        }
        if clip.timeline_out <= start || clip.timeline_in >= end {
            kept.push(clip);
            continue;
        }
        if clip.timeline_in < start {
            let (left, rest) = split_owned(clip, start)?;
            let left_id = left.id.clone();
            kept.push(left);
            if rest.timeline_out <= end {
                removed.push(rest);
            } else {
                let (mid, right) = split_owned(rest, end)?;
                split_pairs.push((left_id, right.id.clone()));
                removed.push(mid);
                kept.push(right);
            }
        } else if clip.timeline_out <= end {
            removed.push(clip);
        } else {
            let (mut mid, mut right) = split_owned(clip, end)?;
            std::mem::swap(&mut mid.id, &mut right.id);
            std::mem::swap(&mut mid.linked_ids, &mut right.linked_ids);
            removed.push(mid);
            kept.push(right);
        }
    }
    kept.sort_by_key(|clip| clip.timeline_in);
    track.clips = kept;
    Ok(RangeCut {
        removed,
        split_pairs,
    })
}

pub(crate) fn shift_starting_at(
    track: &mut Track,
    point: i64,
    delta: i64,
    ignore: &[String],
) -> Result<(), EditError> {
    if delta == 0 {
        return Ok(());
    }
    for clip in &mut track.clips {
        if clip.timeline_in >= point && !ignore.iter().any(|id| id == &clip.id) {
            shift_clip(clip, delta)?;
        }
    }
    sort_clips(track);
    Ok(())
}

/// Open `delta` ticks at `at` by splitting a straddling clip and shifting the tail.
pub(crate) fn insert_time(
    track: &mut Track,
    at: i64,
    delta: i64,
) -> Result<Vec<(String, String)>, EditError> {
    if delta <= 0 {
        return Err(EditError::InvalidTime);
    }
    let original = std::mem::take(&mut track.clips);
    let mut next = Vec::with_capacity(original.len() + 1);
    let mut pairs = Vec::new();
    for clip in original {
        if clip.timeline_in < at && clip.timeline_out > at {
            let left_id = clip.id.clone();
            let (left, mut right) = split_owned(clip, at)?;
            let right_id = right.id.clone();
            shift_clip(&mut right, delta)?;
            pairs.push((left_id, right_id));
            next.push(left);
            next.push(right);
        } else if clip.timeline_in >= at {
            let mut moved = clip;
            shift_clip(&mut moved, delta)?;
            next.push(moved);
        } else {
            next.push(clip);
        }
    }
    next.sort_by_key(|clip| clip.timeline_in);
    track.clips = next;
    Ok(pairs)
}

pub(crate) fn remove_time(track: &mut Track, start: i64, end: i64) -> Result<RangeCut, EditError> {
    let cut = clear_range(track, start, end, &[])?;
    let delta = start.checked_sub(end).ok_or(EditError::InvalidTime)?;
    shift_starting_at(track, end, delta, &[])?;
    Ok(cut)
}

/// Shift tails at `point`, or close `[point + delta, point)` when `delta` is negative.
pub(crate) fn ripple_from(
    track: &mut Track,
    point: i64,
    delta: i64,
    ignore: &[String],
) -> Result<Vec<(String, String)>, EditError> {
    if delta == 0 {
        return Ok(Vec::new());
    }
    if delta > 0 {
        let pairs = split_containing(track, point, ignore)?;
        shift_starting_at(track, point, delta, ignore)?;
        return Ok(pairs);
    }
    let start = point.checked_add(delta).ok_or(EditError::InvalidTime)?;
    if start < 0 {
        return Err(EditError::InvalidTime);
    }
    let cut = clear_range(track, start, point, ignore)?;
    shift_starting_at(track, point, delta, ignore)?;
    Ok(cut.split_pairs)
}

fn split_containing(
    track: &mut Track,
    at: i64,
    ignore: &[String],
) -> Result<Vec<(String, String)>, EditError> {
    let original = std::mem::take(&mut track.clips);
    let mut next = Vec::with_capacity(original.len() + 1);
    let mut pairs = Vec::new();
    for clip in original {
        let ignored = ignore.iter().any(|id| id == &clip.id);
        if !ignored && clip.timeline_in < at && clip.timeline_out > at {
            let left_id = clip.id.clone();
            let (left, right) = split_owned(clip, at)?;
            pairs.push((left_id, right.id.clone()));
            next.push(left);
            next.push(right);
        } else {
            next.push(clip);
        }
    }
    track.clips = next;
    Ok(pairs)
}

pub(crate) fn split_clip_on_track(
    track: &mut Track,
    clip_id: &str,
    at: i64,
) -> Result<(String, String), EditError> {
    let index = track
        .clips
        .iter()
        .position(|clip| clip.id == clip_id)
        .ok_or_else(|| EditError::ClipNotFound(clip_id.to_string()))?;
    let clip = track.clips.remove(index);
    let (left, right) = split_owned(clip, at)?;
    let pair = (left.id.clone(), right.id.clone());
    track.clips.push(left);
    track.clips.push(right);
    sort_clips(track);
    Ok(pair)
}

fn signed_source(delta_timeline: i64, speed: f64) -> i64 {
    let magnitude = scale_by_speed(delta_timeline.abs(), speed);
    if delta_timeline >= 0 {
        magnitude
    } else {
        -magnitude
    }
}

/// Change the source consumed by the out edge. `delta_timeline` is the duration change.
pub(crate) fn apply_out_delta(clip: &mut Clip, delta_timeline: i64) -> Result<(), EditError> {
    if delta_timeline == 0 || clip.speed == 0.0 {
        return Ok(());
    }
    let signed = signed_source(delta_timeline, clip.speed);
    if clip.reversed {
        let next = clip
            .source_in
            .checked_sub(signed)
            .ok_or(EditError::InsufficientSource)?;
        if next < 0 || next >= clip.source_out {
            return Err(EditError::InsufficientSource);
        }
        clip.source_in = next;
    } else {
        let next = clip
            .source_out
            .checked_add(signed)
            .ok_or(EditError::InsufficientSource)?;
        if next <= clip.source_in {
            return Err(EditError::InsufficientSource);
        }
        clip.source_out = next;
    }
    Ok(())
}

/// Change the source consumed by the in edge. `delta_timeline` is the duration change.
pub(crate) fn apply_in_delta(clip: &mut Clip, delta_timeline: i64) -> Result<(), EditError> {
    if delta_timeline == 0 || clip.speed == 0.0 {
        return Ok(());
    }
    let signed = signed_source(delta_timeline, clip.speed);
    if clip.reversed {
        let next = clip
            .source_out
            .checked_add(signed)
            .ok_or(EditError::InsufficientSource)?;
        if next <= clip.source_in {
            return Err(EditError::InsufficientSource);
        }
        clip.source_out = next;
    } else {
        let next = clip
            .source_in
            .checked_sub(signed)
            .ok_or(EditError::InsufficientSource)?;
        if next < 0 || next >= clip.source_out {
            return Err(EditError::InsufficientSource);
        }
        clip.source_in = next;
    }
    Ok(())
}

pub(crate) fn timeline_duration_of(clip: &Clip) -> Result<i64, EditError> {
    let existing = clip.duration_ticks();
    if existing > 0 {
        return Ok(existing);
    }
    if clip.speed == 0.0 {
        return Err(EditError::ZeroDuration);
    }
    let source = clip.source_out.saturating_sub(clip.source_in);
    if source <= 0 {
        return Err(EditError::ZeroDuration);
    }
    let duration = ((source as f64) / clip.speed).round() as i64;
    if duration <= 0 {
        Err(EditError::ZeroDuration)
    } else {
        Ok(duration)
    }
}

pub(crate) fn place_clip(clip: &mut Clip, at: i64, seq: &Sequence) -> Result<i64, EditError> {
    if at < 0 {
        return Err(EditError::InvalidTime);
    }
    if clip.id.is_empty() || clip_exists(seq, &clip.id) {
        clip.id = new_id("clip");
    }
    let duration = timeline_duration_of(clip)?;
    clip.timeline_in = at;
    clip.timeline_out = at.checked_add(duration).ok_or(EditError::InvalidTime)?;
    Ok(duration)
}

pub(crate) fn expand_links(seq: &Sequence, seeds: &[String]) -> Result<Vec<String>, EditError> {
    if seeds.is_empty() {
        return Err(EditError::EmptyEdit);
    }
    let mut out = Vec::new();
    let mut stack = seeds.to_vec();
    while let Some(id) = stack.pop() {
        if out.iter().any(|seen| seen == &id) {
            continue;
        }
        let links = clip_ref(seq, &id)?.linked_ids.clone();
        out.push(id);
        stack.extend(links);
    }
    Ok(out)
}

pub(crate) fn assign_links(seq: &mut Sequence, ids: &[String]) -> Result<(), EditError> {
    for id in ids {
        let others: Vec<String> = ids.iter().filter(|other| *other != id).cloned().collect();
        clip_mut(seq, id)?.linked_ids = others;
    }
    Ok(())
}

pub(crate) fn cleanup_links(seq: &mut Sequence) {
    let existing: HashSet<String> = tracks(seq)
        .flat_map(|track| track.clips.iter().map(|clip| clip.id.clone()))
        .collect();
    for track in seq
        .video_tracks
        .iter_mut()
        .chain(seq.audio_tracks.iter_mut())
    {
        for clip in &mut track.clips {
            clip.linked_ids
                .retain(|id| id != &clip.id && existing.contains(id));
            clip.linked_ids.sort();
            clip.linked_ids.dedup();
        }
    }
}

/// Right pieces of one linked split become their own link group.
pub(crate) fn relink_split_rights(seq: &mut Sequence, pairs: &[(String, String)]) {
    if pairs.len() < 2 {
        return;
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut groups: Vec<Vec<String>> = Vec::new();
    for (left, right) in pairs {
        if !seen.insert(left.clone()) {
            continue;
        }
        let mut rights = vec![right.clone()];
        let mut stack = vec![left.clone()];
        let mut visited = HashSet::new();
        visited.insert(left.clone());
        while let Some(id) = stack.pop() {
            let links = clip_ref(seq, &id)
                .map(|clip| clip.linked_ids.clone())
                .unwrap_or_default();
            for link in links {
                if !visited.insert(link.clone()) {
                    continue;
                }
                if let Some((_, partner_right)) = pairs.iter().find(|(partner, _)| partner == &link)
                {
                    rights.push(partner_right.clone());
                    stack.push(link);
                }
            }
        }
        rights.sort();
        rights.dedup();
        if rights.len() >= 2 {
            groups.push(rights);
        }
    }
    for group in groups {
        let _ = assign_links(seq, &group);
    }
}

pub(crate) fn adjacent_index(track: &Track, timeline_in: i64, skip: &str) -> Option<usize> {
    track
        .clips
        .iter()
        .position(|clip| clip.id != skip && clip.timeline_in == timeline_in)
}

pub(crate) fn previous_index(track: &Track, timeline_in: i64, skip: &str) -> Option<usize> {
    track
        .clips
        .iter()
        .position(|clip| clip.id != skip && clip.timeline_out == timeline_in)
}
