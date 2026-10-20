//! Editorial operations on a sequence.
//!
//! Picture and sound that share `linked_ids` stay together. A targeted track
//! receives lift, extract, insert, and overwrite. Sync lock ripples every other
//! unlocked track so later clips keep their alignment. Insert and overwrite
//! still edit a destination track when its own sync lock is off.

use std::collections::HashSet;

use videocut_model::{
    new_id, Clip, ClipKind, Marker, MarkerKind, MulticamAngle, Sequence, Track, TrackKind,
};

use crate::error::EditError;
use crate::geom::{
    adjacent_index, apply_in_delta, apply_out_delta, assign_links, cleanup_links, clip_mut,
    clip_ref, commit, expand_links, insert_time, place_clip, previous_index, relink_split_rights,
    remove_time, ripple_from, shift_starting_at, sort_clips, source_at, split_clip_on_track,
    track_id_of, track_mut, track_ref, tracks,
};

/// Which edge a trim moves. `delta_ticks` is the change in clip duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimSide {
    In,
    Out,
}

/// A clip dropped onto one track by insert.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub track_id: String,
    pub clip: Clip,
}

/// Fields for a new sequence marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerDraft {
    pub time_ticks: i64,
    pub duration_ticks: i64,
    pub name: String,
    pub comment: String,
    pub color: u8,
    pub kind: MarkerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapKind {
    SequenceStart,
    ClipIn,
    ClipOut,
    Marker,
    Playhead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapHit {
    pub ticks: i64,
    pub kind: SnapKind,
    pub distance: i64,
}

pub fn set_track_targeted(
    seq: &mut Sequence,
    track_id: &str,
    targeted: bool,
) -> Result<(), EditError> {
    track_mut(seq, track_id)?.targeted = targeted;
    Ok(())
}

pub fn set_sync_lock(seq: &mut Sequence, track_id: &str, enabled: bool) -> Result<(), EditError> {
    track_mut(seq, track_id)?.sync_lock = enabled;
    Ok(())
}

pub fn set_track_locked(seq: &mut Sequence, track_id: &str, locked: bool) -> Result<(), EditError> {
    track_mut(seq, track_id)?.locked = locked;
    Ok(())
}

fn require_targeted_open(seq: &Sequence, track_id: &str) -> Result<(), EditError> {
    let track = track_ref(seq, track_id)?;
    if track.locked {
        return Err(EditError::TrackLocked(track.id.clone()));
    }
    if !track.targeted {
        return Err(EditError::TrackNotTargeted(track.id.clone()));
    }
    Ok(())
}

/// Insert clips at `at` and shift later clips on destination and sync-locked tracks.
pub fn insert(seq: &mut Sequence, at: i64, placements: Vec<Placement>) -> Result<(), EditError> {
    if at < 0 {
        return Err(EditError::InvalidTime);
    }
    if placements.is_empty() {
        return Err(EditError::EmptyEdit);
    }
    commit(seq, |seq| {
        let mut seen = HashSet::new();
        let mut prepared = Vec::with_capacity(placements.len());
        let mut ripple = 0_i64;
        for mut placement in placements {
            if !seen.insert(placement.track_id.clone()) {
                return Err(EditError::DuplicateTrack(placement.track_id));
            }
            require_targeted_open(seq, &placement.track_id)?;
            let duration = place_clip(&mut placement.clip, at, seq)?;
            ripple = ripple.max(duration);
            prepared.push(placement);
        }
        let destinations: Vec<String> = prepared.iter().map(|item| item.track_id.clone()).collect();
        let ripple_tracks = ripple_track_ids(seq, &destinations)?;
        let mut pairs = Vec::new();
        for track_id in &ripple_tracks {
            pairs.extend(insert_time(track_mut(seq, track_id)?, at, ripple)?);
        }
        relink_split_rights(seq, &pairs);
        for placement in prepared {
            let track = track_mut(seq, &placement.track_id)?;
            track.clips.push(placement.clip);
            sort_clips(track);
        }
        Ok(())
    })
}

/// Replace the range under `clip` on one track. Later clips stay where they are.
pub fn overwrite(
    seq: &mut Sequence,
    track_id: &str,
    mut clip: Clip,
    at: i64,
) -> Result<(), EditError> {
    if at < 0 {
        return Err(EditError::InvalidTime);
    }
    commit(seq, |seq| {
        require_targeted_open(seq, track_id)?;
        let duration = place_clip(&mut clip, at, seq)?;
        let end = at.checked_add(duration).ok_or(EditError::InvalidTime)?;
        let track = track_mut(seq, track_id)?;
        let cut = crate::geom::clear_range(track, at, end, &[])?;
        track.clips.push(clip);
        sort_clips(track);
        cleanup_links(seq);
        relink_split_rights(seq, &cut.split_pairs);
        Ok(())
    })
}

/// Remove `[start, end)` on targeted tracks and leave the gap.
pub fn lift(seq: &mut Sequence, start: i64, end: i64) -> Result<Vec<Clip>, EditError> {
    if end <= start {
        return Err(EditError::InvalidRange);
    }
    commit(seq, |seq| {
        let targeted = targeted_ids(seq)?;
        let mut removed = Vec::new();
        let mut pairs = Vec::new();
        for track_id in targeted {
            let cut = crate::geom::clear_range(track_mut(seq, &track_id)?, start, end, &[])?;
            pairs.extend(cut.split_pairs);
            removed.extend(cut.removed);
        }
        cleanup_links(seq);
        relink_split_rights(seq, &pairs);
        Ok(removed)
    })
}

/// Remove `[start, end)` on targeted tracks and close the gap.
/// Sync-locked tracks that were not targeted lose the same time range.
pub fn extract(seq: &mut Sequence, start: i64, end: i64) -> Result<Vec<Clip>, EditError> {
    if end <= start {
        return Err(EditError::InvalidRange);
    }
    commit(seq, |seq| {
        let targeted = targeted_ids(seq)?;
        let targeted_set: HashSet<String> = targeted.iter().cloned().collect();
        let mut affect = targeted;
        for track in tracks(seq) {
            if track.sync_lock && !targeted_set.contains(&track.id) {
                if track.locked {
                    return Err(EditError::TrackLocked(track.id.clone()));
                }
                affect.push(track.id.clone());
            }
        }
        let mut removed = Vec::new();
        let mut pairs = Vec::new();
        for track_id in affect {
            let cut = remove_time(track_mut(seq, &track_id)?, start, end)?;
            pairs.extend(cut.split_pairs);
            removed.extend(cut.removed);
        }
        cleanup_links(seq);
        relink_split_rights(seq, &pairs);
        Ok(removed)
    })
}

/// Split clips under `at`. `track_ids` limits the tracks; linked partners are included.
/// Returns `(left_id, right_id)` pairs. The left clip keeps its id.
pub fn razor(
    seq: &mut Sequence,
    at: i64,
    track_ids: Option<&[String]>,
) -> Result<Vec<(String, String)>, EditError> {
    if at < 0 {
        return Err(EditError::InvalidTime);
    }
    let filter = track_ids.map(|ids| ids.to_vec());
    commit(seq, |seq| razor_inner(seq, at, filter.as_deref()))
}

fn razor_inner(
    seq: &mut Sequence,
    at: i64,
    track_ids: Option<&[String]>,
) -> Result<Vec<(String, String)>, EditError> {
    let mut seeds = Vec::new();
    for track in tracks(seq) {
        if let Some(filter) = track_ids {
            if !filter.iter().any(|id| id == &track.id) {
                continue;
            }
        }
        if track.locked {
            continue;
        }
        for clip in &track.clips {
            if clip.timeline_in < at && clip.timeline_out > at {
                seeds.push(clip.id.clone());
            }
        }
    }
    if seeds.is_empty() {
        return Ok(Vec::new());
    }
    let expanded = expand_links(seq, &seeds)?;
    let mut pairs = Vec::new();
    for clip_id in expanded {
        let track_id = track_id_of(seq, &clip_id)?;
        let track = track_ref(seq, &track_id)?;
        if track.locked {
            return Err(EditError::TrackLocked(track_id));
        }
        let contains = track
            .clips
            .iter()
            .any(|clip| clip.id == clip_id && clip.timeline_in < at && clip.timeline_out > at);
        if !contains {
            continue;
        }
        let track = track_mut(seq, &track_id)?;
        pairs.push(split_clip_on_track(track, &clip_id, at)?);
    }
    relink_split_rights(seq, &pairs);
    Ok(pairs)
}

/// Lengthen or shorten one edge and move every later edit by the same amount.
/// Positive `delta_ticks` makes the clip longer.
pub fn ripple_trim(
    seq: &mut Sequence,
    clip_id: &str,
    side: TrimSide,
    delta_ticks: i64,
) -> Result<(), EditError> {
    if delta_ticks == 0 {
        return Ok(());
    }
    let clip_id = clip_id.to_string();
    commit(seq, |seq| {
        let ids = expand_links(seq, std::slice::from_ref(&clip_id))?;
        let mut events = Vec::new();
        for id in &ids {
            let track_id = track_id_of(seq, id)?;
            let track = track_ref(seq, &track_id)?;
            if track.locked {
                return Err(EditError::TrackLocked(track_id));
            }
            let clip = clip_ref(seq, id)?;
            let new_duration = clip
                .duration_ticks()
                .checked_add(delta_ticks)
                .ok_or(EditError::InvalidTime)?;
            if new_duration <= 0 {
                return Err(EditError::ZeroDuration);
            }
            events.push((track_id, clip.timeline_out, id.clone(), new_duration));
        }
        for (_, _, id, new_duration) in &events {
            let clip = clip_mut(seq, id)?;
            match side {
                TrimSide::In => apply_in_delta(clip, delta_ticks)?,
                TrimSide::Out => apply_out_delta(clip, delta_ticks)?,
            }
            clip.timeline_out = clip
                .timeline_in
                .checked_add(*new_duration)
                .ok_or(EditError::InvalidTime)?;
        }
        events.sort_by_key(|event| std::cmp::Reverse(event.1));
        let mut seen = HashSet::new();
        let mut pairs = Vec::new();
        let mut handled = HashSet::new();
        for (track_id, old_out, id, _) in &events {
            handled.insert(track_id.clone());
            if !seen.insert((track_id.clone(), *old_out)) {
                continue;
            }
            let ignore: Vec<String> = events
                .iter()
                .filter(|event| &event.0 == track_id)
                .map(|event| event.2.clone())
                .collect();
            let _ = id;
            pairs.extend(ripple_from(
                track_mut(seq, track_id)?,
                *old_out,
                delta_ticks,
                &ignore,
            )?);
        }
        let primary_out = events
            .iter()
            .find(|event| event.2 == clip_id)
            .map(|event| event.1)
            .ok_or_else(|| EditError::ClipNotFound(clip_id.clone()))?;
        let sync_ids: Vec<String> = tracks(seq)
            .filter(|track| track.sync_lock && !handled.contains(&track.id))
            .map(|track| track.id.clone())
            .collect();
        for track_id in sync_ids {
            if track_ref(seq, &track_id)?.locked {
                return Err(EditError::TrackLocked(track_id));
            }
            pairs.extend(ripple_from(
                track_mut(seq, &track_id)?,
                primary_out,
                delta_ticks,
                &[],
            )?);
        }
        relink_split_rights(seq, &pairs);
        Ok(())
    })
}

/// Move the cut after `left_clip_id`. Positive `delta_ticks` gives time to the left clip.
pub fn roll_trim(
    seq: &mut Sequence,
    left_clip_id: &str,
    delta_ticks: i64,
) -> Result<(), EditError> {
    if delta_ticks == 0 {
        return Ok(());
    }
    let left_clip_id = left_clip_id.to_string();
    commit(seq, |seq| {
        let ids = expand_links(seq, &[left_clip_id])?;
        for id in ids {
            let track_id = track_id_of(seq, &id)?;
            let track = track_mut(seq, &track_id)?;
            if track.locked {
                return Err(EditError::TrackLocked(track_id));
            }
            roll_on_track(track, &id, delta_ticks)?;
        }
        Ok(())
    })
}

fn roll_on_track(track: &mut Track, left_id: &str, delta: i64) -> Result<(), EditError> {
    let left_index = track
        .clips
        .iter()
        .position(|clip| clip.id == left_id)
        .ok_or_else(|| EditError::ClipNotFound(left_id.to_string()))?;
    let left_out = track.clips[left_index].timeline_out;
    let right_index = adjacent_index(track, left_out, left_id).ok_or(EditError::NotAdjacent)?;
    let mut left = track.clips[left_index].clone();
    let mut right = track.clips[right_index].clone();
    let new_left = left
        .duration_ticks()
        .checked_add(delta)
        .ok_or(EditError::InvalidTime)?;
    let new_right = right
        .duration_ticks()
        .checked_sub(delta)
        .ok_or(EditError::InvalidTime)?;
    if new_left <= 0 || new_right <= 0 {
        return Err(EditError::ZeroDuration);
    }
    apply_out_delta(&mut left, delta)?;
    apply_in_delta(&mut right, -delta)?;
    left.timeline_out = left
        .timeline_in
        .checked_add(new_left)
        .ok_or(EditError::InvalidTime)?;
    right.timeline_in = right
        .timeline_out
        .checked_sub(new_right)
        .ok_or(EditError::InvalidTime)?;
    if right.timeline_in < 0 {
        return Err(EditError::InvalidTime);
    }
    track.clips[left_index] = left;
    track.clips[right_index] = right;
    sort_clips(track);
    Ok(())
}

/// Move the source window by `delta_source` without moving the clip on the timeline.
pub fn slip(seq: &mut Sequence, clip_id: &str, delta_source: i64) -> Result<(), EditError> {
    if delta_source == 0 {
        return Ok(());
    }
    let clip_id = clip_id.to_string();
    commit(seq, |seq| {
        let ids = expand_links(seq, &[clip_id])?;
        for id in ids {
            let clip = clip_mut(seq, &id)?;
            let new_in = clip
                .source_in
                .checked_add(delta_source)
                .ok_or(EditError::InsufficientSource)?;
            let new_out = clip
                .source_out
                .checked_add(delta_source)
                .ok_or(EditError::InsufficientSource)?;
            if new_in < 0 || new_out < new_in {
                return Err(EditError::InsufficientSource);
            }
            if clip.speed != 0.0 && new_out <= new_in {
                return Err(EditError::InsufficientSource);
            }
            clip.source_in = new_in;
            clip.source_out = new_out;
        }
        Ok(())
    })
}

/// Slide a clip between its neighbors. The sequence duration does not change.
pub fn slide(seq: &mut Sequence, clip_id: &str, delta_ticks: i64) -> Result<(), EditError> {
    if delta_ticks == 0 {
        return Ok(());
    }
    let clip_id = clip_id.to_string();
    commit(seq, |seq| {
        let ids = expand_links(seq, &[clip_id])?;
        for id in ids {
            let track_id = track_id_of(seq, &id)?;
            let track = track_mut(seq, &track_id)?;
            if track.locked {
                return Err(EditError::TrackLocked(track_id));
            }
            slide_on_track(track, &id, delta_ticks)?;
        }
        Ok(())
    })
}

fn slide_on_track(track: &mut Track, clip_id: &str, delta: i64) -> Result<(), EditError> {
    let index = track
        .clips
        .iter()
        .position(|clip| clip.id == clip_id)
        .ok_or_else(|| EditError::ClipNotFound(clip_id.to_string()))?;
    let start = track.clips[index].timeline_in;
    let end = track.clips[index].timeline_out;
    let prev_index = previous_index(track, start, clip_id).ok_or(EditError::NotAdjacent)?;
    let next_index = adjacent_index(track, end, clip_id).ok_or(EditError::NotAdjacent)?;
    let mut prev = track.clips[prev_index].clone();
    let mut clip = track.clips[index].clone();
    let mut next = track.clips[next_index].clone();
    let new_prev = prev
        .duration_ticks()
        .checked_add(delta)
        .ok_or(EditError::InvalidTime)?;
    let new_next = next
        .duration_ticks()
        .checked_sub(delta)
        .ok_or(EditError::InvalidTime)?;
    if new_prev <= 0 || new_next <= 0 {
        return Err(EditError::ZeroDuration);
    }
    apply_out_delta(&mut prev, delta)?;
    apply_in_delta(&mut next, -delta)?;
    prev.timeline_out = prev
        .timeline_in
        .checked_add(new_prev)
        .ok_or(EditError::InvalidTime)?;
    crate::geom::shift_clip(&mut clip, delta)?;
    next.timeline_in = next
        .timeline_out
        .checked_sub(new_next)
        .ok_or(EditError::InvalidTime)?;
    if next.timeline_in < 0 {
        return Err(EditError::InvalidTime);
    }
    track.clips[prev_index] = prev;
    track.clips[index] = clip;
    track.clips[next_index] = next;
    sort_clips(track);
    Ok(())
}

/// Delete clips, including linked partners, and close the gap they occupied.
pub fn ripple_delete(seq: &mut Sequence, clip_ids: &[String]) -> Result<(), EditError> {
    if clip_ids.is_empty() {
        return Err(EditError::EmptyEdit);
    }
    let clip_ids = clip_ids.to_vec();
    commit(seq, |seq| {
        let ids = expand_links(seq, &clip_ids)?;
        let mut spans = Vec::new();
        for id in &ids {
            let track_id = track_id_of(seq, id)?;
            let track = track_ref(seq, &track_id)?;
            if track.locked {
                return Err(EditError::TrackLocked(track_id));
            }
            let clip = clip_ref(seq, id)?;
            spans.push((track_id, clip.timeline_in, clip.timeline_out));
        }
        for id in &ids {
            let track_id = track_id_of(seq, id)?;
            track_mut(seq, &track_id)?
                .clips
                .retain(|clip| &clip.id != id);
        }
        cleanup_links(seq);
        spans.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)));
        let mut seen_on_track = HashSet::new();
        let mut synced = HashSet::new();
        let mut pairs = Vec::new();
        for (track_id, start, end) in &spans {
            if !seen_on_track.insert((track_id.clone(), *start, *end)) {
                continue;
            }
            let delta = start.checked_sub(*end).ok_or(EditError::InvalidTime)?;
            shift_starting_at(track_mut(seq, track_id)?, *end, delta, &[])?;
            if !synced.insert((*start, *end)) {
                continue;
            }
            let deleted_tracks: HashSet<String> = spans
                .iter()
                .filter(|span| span.1 == *start && span.2 == *end)
                .map(|span| span.0.clone())
                .collect();
            let sync_ids: Vec<String> = tracks(seq)
                .filter(|track| track.sync_lock && !deleted_tracks.contains(&track.id))
                .map(|track| track.id.clone())
                .collect();
            for sync_id in sync_ids {
                if track_ref(seq, &sync_id)?.locked {
                    return Err(EditError::TrackLocked(sync_id));
                }
                let cut = remove_time(track_mut(seq, &sync_id)?, *start, *end)?;
                pairs.extend(cut.split_pairs);
            }
        }
        relink_split_rights(seq, &pairs);
        Ok(())
    })
}

pub fn link_clips(seq: &mut Sequence, clip_ids: &[String]) -> Result<(), EditError> {
    if clip_ids.len() < 2 {
        return Err(EditError::EmptyEdit);
    }
    let clip_ids = clip_ids.to_vec();
    commit(seq, |seq| {
        let mut group = Vec::new();
        for id in &clip_ids {
            for linked in expand_links(seq, std::slice::from_ref(id))? {
                if !group.contains(&linked) {
                    group.push(linked);
                }
            }
        }
        assign_links(seq, &group)
    })
}

pub fn unlink_clip(seq: &mut Sequence, clip_id: &str) -> Result<(), EditError> {
    let clip_id = clip_id.to_string();
    commit(seq, |seq| {
        let partners = clip_ref(seq, &clip_id)?.linked_ids.clone();
        clip_mut(seq, &clip_id)?.linked_ids.clear();
        for partner in partners {
            if let Ok(clip) = clip_mut(seq, &partner) {
                clip.linked_ids.retain(|id| id != &clip_id);
            }
        }
        Ok(())
    })
}

/// Keep the source range and change timeline length to match `speed`.
pub fn set_speed(
    seq: &mut Sequence,
    clip_id: &str,
    speed: f64,
    ripple: bool,
) -> Result<(), EditError> {
    if !speed.is_finite() || speed <= 0.0 {
        return Err(EditError::InvalidSpeed);
    }
    let clip_id = clip_id.to_string();
    commit(seq, |seq| {
        let ids = expand_links(seq, std::slice::from_ref(&clip_id))?;
        let mut changes = Vec::new();
        for id in &ids {
            let track_id = track_id_of(seq, id)?;
            if track_ref(seq, &track_id)?.locked {
                return Err(EditError::TrackLocked(track_id));
            }
            let clip = clip_ref(seq, id)?;
            let source = clip.source_out.saturating_sub(clip.source_in);
            if source <= 0 {
                return Err(EditError::InsufficientSource);
            }
            let new_duration = ((source as f64) / speed).round() as i64;
            if new_duration <= 0 {
                return Err(EditError::ZeroDuration);
            }
            changes.push((
                track_id,
                clip.timeline_out,
                id.clone(),
                new_duration,
                Some(speed),
            ));
        }
        apply_retiming(seq, &clip_id, changes, ripple)
    })
}

/// Keep the source range and change speed so the clip lasts `duration_ticks`.
pub fn set_duration(
    seq: &mut Sequence,
    clip_id: &str,
    duration_ticks: i64,
    ripple: bool,
) -> Result<(), EditError> {
    if duration_ticks <= 0 {
        return Err(EditError::ZeroDuration);
    }
    let clip_id = clip_id.to_string();
    commit(seq, |seq| {
        let ids = expand_links(seq, std::slice::from_ref(&clip_id))?;
        let mut changes = Vec::new();
        for id in &ids {
            let track_id = track_id_of(seq, id)?;
            if track_ref(seq, &track_id)?.locked {
                return Err(EditError::TrackLocked(track_id));
            }
            let clip = clip_ref(seq, id)?;
            let source = clip.source_out.saturating_sub(clip.source_in);
            let new_speed = if source > 0 {
                Some(source as f64 / duration_ticks as f64)
            } else if clip.speed == 0.0 {
                None
            } else {
                return Err(EditError::InsufficientSource);
            };
            changes.push((
                track_id,
                clip.timeline_out,
                id.clone(),
                duration_ticks,
                new_speed,
            ));
        }
        apply_retiming(seq, &clip_id, changes, ripple)
    })
}

fn apply_retiming(
    seq: &mut Sequence,
    primary_id: &str,
    changes: Vec<(String, i64, String, i64, Option<f64>)>,
    ripple: bool,
) -> Result<(), EditError> {
    let primary = changes
        .iter()
        .find(|change| change.2 == primary_id)
        .cloned()
        .ok_or_else(|| EditError::ClipNotFound(primary_id.to_string()))?;
    for (_, _, id, new_duration, new_speed) in &changes {
        let clip = clip_mut(seq, id)?;
        if let Some(speed) = new_speed {
            clip.speed = *speed;
        }
        clip.timeline_out = clip
            .timeline_in
            .checked_add(*new_duration)
            .ok_or(EditError::InvalidTime)?;
    }
    if !ripple {
        return Ok(());
    }
    let mut ordered = changes.clone();
    ordered.sort_by_key(|change| std::cmp::Reverse(change.1));
    let mut seen = HashSet::new();
    let mut handled = HashSet::new();
    let mut pairs = Vec::new();
    for (track_id, old_out, id, new_duration, _) in &ordered {
        handled.insert(track_id.clone());
        let clip_in = clip_ref(seq, id)?.timeline_in;
        let delta = clip_in
            .checked_add(*new_duration)
            .ok_or(EditError::InvalidTime)?
            .checked_sub(*old_out)
            .ok_or(EditError::InvalidTime)?;
        if delta == 0 || !seen.insert((track_id.clone(), *old_out)) {
            continue;
        }
        let ignore: Vec<String> = ordered
            .iter()
            .filter(|change| &change.0 == track_id)
            .map(|change| change.2.clone())
            .collect();
        pairs.extend(ripple_from(
            track_mut(seq, track_id)?,
            *old_out,
            delta,
            &ignore,
        )?);
    }
    let primary_delta = {
        let clip_in = clip_ref(seq, primary_id)?.timeline_in;
        clip_in
            .checked_add(primary.3)
            .ok_or(EditError::InvalidTime)?
            .checked_sub(primary.1)
            .ok_or(EditError::InvalidTime)?
    };
    if primary_delta != 0 {
        let sync_ids: Vec<String> = tracks(seq)
            .filter(|track| track.sync_lock && !handled.contains(&track.id))
            .map(|track| track.id.clone())
            .collect();
        for track_id in sync_ids {
            if track_ref(seq, &track_id)?.locked {
                return Err(EditError::TrackLocked(track_id.clone()));
            }
            pairs.extend(ripple_from(
                track_mut(seq, &track_id)?,
                primary.1,
                primary_delta,
                &[],
            )?);
        }
    }
    relink_split_rights(seq, &pairs);
    Ok(())
}

pub fn set_reversed(seq: &mut Sequence, clip_id: &str, reversed: bool) -> Result<(), EditError> {
    let clip_id = clip_id.to_string();
    commit(seq, |seq| {
        for id in expand_links(seq, &[clip_id])? {
            clip_mut(seq, &id)?.reversed = reversed;
        }
        Ok(())
    })
}

pub fn add_marker(seq: &mut Sequence, draft: MarkerDraft) -> Result<String, EditError> {
    if draft.time_ticks < 0 || draft.duration_ticks < 0 {
        return Err(EditError::InvalidTime);
    }
    let marker = Marker {
        id: new_id("marker"),
        time_ticks: draft.time_ticks,
        duration_ticks: draft.duration_ticks,
        name: draft.name,
        comment: draft.comment,
        color: draft.color,
        kind: draft.kind,
    };
    let id = marker.id.clone();
    seq.markers.push(marker);
    seq.markers.sort_by_key(|marker| marker.time_ticks);
    Ok(id)
}

pub fn move_marker(seq: &mut Sequence, marker_id: &str, time_ticks: i64) -> Result<(), EditError> {
    if time_ticks < 0 {
        return Err(EditError::InvalidTime);
    }
    let marker = seq
        .markers
        .iter_mut()
        .find(|marker| marker.id == marker_id)
        .ok_or_else(|| EditError::MarkerNotFound(marker_id.to_string()))?;
    marker.time_ticks = time_ticks;
    seq.markers.sort_by_key(|marker| marker.time_ticks);
    Ok(())
}

pub fn remove_marker(seq: &mut Sequence, marker_id: &str) -> Result<Marker, EditError> {
    let index = seq
        .markers
        .iter()
        .position(|marker| marker.id == marker_id)
        .ok_or_else(|| EditError::MarkerNotFound(marker_id.to_string()))?;
    Ok(seq.markers.remove(index))
}

pub fn collect_snap_points(
    seq: &Sequence,
    playhead: Option<i64>,
    ignore_clip_ids: &[String],
) -> Vec<(i64, SnapKind)> {
    let mut points = vec![(0, SnapKind::SequenceStart)];
    if let Some(playhead) = playhead {
        points.push((playhead, SnapKind::Playhead));
    }
    for track in tracks(seq) {
        for clip in &track.clips {
            if ignore_clip_ids.iter().any(|id| id == &clip.id) {
                continue;
            }
            points.push((clip.timeline_in, SnapKind::ClipIn));
            points.push((clip.timeline_out, SnapKind::ClipOut));
        }
    }
    for marker in &seq.markers {
        points.push((marker.time_ticks, SnapKind::Marker));
    }
    points
}

pub fn snap_hit(ticks: i64, points: &[(i64, SnapKind)], threshold: i64) -> Option<SnapHit> {
    let threshold = threshold.max(0);
    let mut best: Option<SnapHit> = None;
    for (point, kind) in points {
        let distance = (point - ticks).saturating_abs();
        if distance > threshold {
            continue;
        }
        let replace = match best {
            None => true,
            Some(current) => {
                distance < current.distance
                    || (distance == current.distance && *point < current.ticks)
            }
        };
        if replace {
            best = Some(SnapHit {
                ticks: *point,
                kind: *kind,
                distance,
            });
        }
    }
    best
}

pub fn snap_ticks(
    seq: &Sequence,
    ticks: i64,
    threshold: i64,
    playhead: Option<i64>,
    ignore_clip_ids: &[String],
) -> i64 {
    let points = collect_snap_points(seq, playhead, ignore_clip_ids);
    snap_hit(ticks, &points, threshold)
        .map(|hit| hit.ticks)
        .unwrap_or(ticks)
}

/// Replace the selected clips with one nested clip. The returned sequence holds
/// the moved clips, starting at time zero, and must be stored on the project.
pub fn nest(
    seq: &mut Sequence,
    clip_ids: &[String],
    name: impl Into<String>,
) -> Result<Sequence, EditError> {
    let name = name.into();
    let clip_ids = clip_ids.to_vec();
    commit(seq, |seq| {
        let ids = expand_links(seq, &clip_ids)?;
        let mut moved = Vec::new();
        for id in &ids {
            let track_id = track_id_of(seq, id)?;
            if track_ref(seq, &track_id)?.locked {
                return Err(EditError::TrackLocked(track_id));
            }
            moved.push((track_id, clip_ref(seq, id)?.clone()));
        }
        let min_in = moved
            .iter()
            .map(|(_, clip)| clip.timeline_in)
            .min()
            .ok_or(EditError::EmptyEdit)?;
        let max_out = moved
            .iter()
            .map(|(_, clip)| clip.timeline_out)
            .max()
            .ok_or(EditError::EmptyEdit)?;
        if max_out <= min_in {
            return Err(EditError::ZeroDuration);
        }
        let dest = nest_destination(seq, &moved)?;
        let mut child = Sequence::new(name, seq.settings.clone());
        let parent_tracks: Vec<Track> = tracks(seq).cloned().collect();
        for parent in &parent_tracks {
            let mut clips: Vec<Clip> = moved
                .iter()
                .filter(|(track_id, _)| track_id == &parent.id)
                .map(|(_, clip)| clip.clone())
                .collect();
            if clips.is_empty() {
                continue;
            }
            for clip in &mut clips {
                clip.timeline_in -= min_in;
                clip.timeline_out -= min_in;
            }
            sort_owned(&mut clips);
            let mut track = Track::new(parent.index, parent.kind);
            track.targeted = parent.targeted;
            track.sync_lock = parent.sync_lock;
            track.clips = clips;
            match parent.kind {
                TrackKind::Video => child.video_tracks.push(track),
                TrackKind::Audio => child.audio_tracks.push(track),
            }
        }
        for id in &ids {
            let track_id = track_id_of(seq, id)?;
            track_mut(seq, &track_id)?
                .clips
                .retain(|clip| &clip.id != id);
        }
        cleanup_links(seq);
        let track = track_mut(seq, &dest)?;
        if track
            .clips
            .iter()
            .any(|clip| clip.timeline_in < max_out && clip.timeline_out > min_in)
        {
            return Err(EditError::Overlap);
        }
        let mut nested = Clip::media(child.name.clone(), "");
        nested.media_id = None;
        nested.kind = ClipKind::Nested;
        nested.nested_sequence_id = Some(child.id.clone());
        nested.timeline_in = min_in;
        nested.timeline_out = max_out;
        nested.source_in = 0;
        nested.source_out = max_out - min_in;
        nested.speed = 1.0;
        track.clips.push(nested);
        sort_clips(track);
        Ok(child)
    })
}

fn sort_owned(clips: &mut [Clip]) {
    clips.sort_by_key(|clip| clip.timeline_in);
}

fn nest_destination(seq: &Sequence, moved: &[(String, Clip)]) -> Result<String, EditError> {
    let mut video: Vec<(u32, String)> = Vec::new();
    let mut audio: Vec<(u32, String)> = Vec::new();
    for (track_id, _) in moved {
        let track = track_ref(seq, track_id)?;
        match track.kind {
            TrackKind::Video => video.push((track.index, track.id.clone())),
            TrackKind::Audio => audio.push((track.index, track.id.clone())),
        }
    }
    video.sort_by_key(|item| item.0);
    audio.sort_by_key(|item| item.0);
    video
        .into_iter()
        .chain(audio)
        .map(|(_, id)| id)
        .next()
        .ok_or(EditError::EmptyEdit)
}

pub fn add_multicam_angle(
    seq: &mut Sequence,
    name: impl Into<String>,
    media_id: impl Into<String>,
    sync_ticks: i64,
) -> String {
    let angle = MulticamAngle {
        id: new_id("angle"),
        name: name.into(),
        media_id: media_id.into(),
        sync_ticks,
    };
    let id = angle.id.clone();
    seq.multicam_angles.push(angle);
    id
}

/// Cut `clip_id` at `at` when needed and point the right piece at `angle_id`.
/// Angle `sync_ticks` keeps the frame at the cut aligned.
pub fn switch_multicam_angle(
    seq: &mut Sequence,
    clip_id: &str,
    at: i64,
    angle_id: &str,
) -> Result<String, EditError> {
    let clip_id = clip_id.to_string();
    let angle_id = angle_id.to_string();
    commit(seq, |seq| {
        let angle = seq
            .multicam_angles
            .iter()
            .find(|angle| angle.id == angle_id)
            .cloned()
            .ok_or(EditError::AngleNotFound(angle_id))?;
        let clip = clip_ref(seq, &clip_id)?.clone();
        if at < clip.timeline_in || at >= clip.timeline_out {
            return Err(EditError::NotUnderPlayhead);
        }
        let old_sync = seq
            .multicam_angles
            .iter()
            .find(|item| clip.media_id.as_ref() == Some(&item.media_id))
            .map(|item| item.sync_ticks)
            .unwrap_or(0);
        let delta_sync = angle
            .sync_ticks
            .checked_sub(old_sync)
            .ok_or(EditError::InvalidTime)?;
        let track_id = track_id_of(seq, &clip_id)?;
        let target = if at == clip.timeline_in {
            clip_id
        } else {
            let pairs = razor_inner(seq, at, Some(&[track_id]))?;
            pairs
                .into_iter()
                .find(|(left, _)| left == &clip_id)
                .map(|(_, right)| right)
                .ok_or(EditError::NotUnderPlayhead)?
        };
        let clip = clip_mut(seq, &target)?;
        let new_in = clip
            .source_in
            .checked_add(delta_sync)
            .ok_or(EditError::InsufficientSource)?;
        let new_out = clip
            .source_out
            .checked_add(delta_sync)
            .ok_or(EditError::InsufficientSource)?;
        if new_in < 0 || (clip.speed != 0.0 && new_out <= new_in) {
            return Err(EditError::InsufficientSource);
        }
        clip.source_in = new_in;
        clip.source_out = new_out;
        clip.media_id = Some(angle.media_id);
        Ok(target)
    })
}

/// Insert a still of the frame under `at` and push the tail by `hold_ticks`.
pub fn freeze_frame(
    seq: &mut Sequence,
    clip_id: &str,
    at: i64,
    hold_ticks: i64,
) -> Result<String, EditError> {
    if hold_ticks <= 0 {
        return Err(EditError::ZeroDuration);
    }
    let clip_id = clip_id.to_string();
    commit(seq, |seq| {
        let track_id = track_id_of(seq, &clip_id)?;
        let clip = clip_ref(seq, &clip_id)?.clone();
        if at < clip.timeline_in || at >= clip.timeline_out {
            return Err(EditError::NotUnderPlayhead);
        }
        let frame = source_at(&clip, at);
        if at > clip.timeline_in {
            razor_inner(seq, at, Some(std::slice::from_ref(&track_id)))?;
        }
        let mut freeze = Clip::media(format!("{} Freeze", clip.name), "");
        freeze.media_id = clip.media_id.clone();
        freeze.kind = clip.kind;
        freeze.nested_sequence_id = clip.nested_sequence_id.clone();
        freeze.speed = 0.0;
        freeze.reversed = false;
        freeze.enabled = clip.enabled;
        freeze.label_color = clip.label_color;
        freeze.source_in = frame;
        freeze.source_out = frame;
        freeze.timeline_in = at;
        freeze.timeline_out = at.checked_add(hold_ticks).ok_or(EditError::InvalidTime)?;
        let freeze_id = freeze.id.clone();
        insert(
            seq,
            at,
            vec![Placement {
                track_id,
                clip: freeze,
            }],
        )?;
        Ok(freeze_id)
    })
}

fn targeted_ids(seq: &Sequence) -> Result<Vec<String>, EditError> {
    let mut ids = Vec::new();
    for track in tracks(seq) {
        if !track.targeted {
            continue;
        }
        if track.locked {
            return Err(EditError::TrackLocked(track.id.clone()));
        }
        ids.push(track.id.clone());
    }
    if ids.is_empty() {
        return Err(EditError::EmptyEdit);
    }
    Ok(ids)
}

fn ripple_track_ids(seq: &Sequence, destinations: &[String]) -> Result<Vec<String>, EditError> {
    let mut ids = Vec::new();
    for track in tracks(seq) {
        let destination = destinations.iter().any(|id| id == &track.id);
        if destination || track.sync_lock {
            if track.locked {
                return Err(EditError::TrackLocked(track.id.clone()));
            }
            ids.push(track.id.clone());
        }
    }
    Ok(ids)
}
