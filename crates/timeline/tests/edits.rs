use videocut_model::{
    Clip, MarkerKind, Sequence, SequenceSettings, Track, TrackKind, TICKS_PER_SECOND,
};
use videocut_timeline::{
    add_marker, add_multicam_angle, extract, freeze_frame, insert, lift, link_clips, move_marker,
    nest, overwrite, razor, remove_marker, ripple_delete, ripple_trim, roll_trim, set_duration,
    set_reversed, set_speed, set_sync_lock, set_track_locked, set_track_targeted, slide, slip,
    snap_ticks, source_at, switch_multicam_angle, unlink_clip, EditError, MarkerDraft, Placement,
    SnapKind, TrimSide,
};

fn ticks(seconds: i64) -> i64 {
    seconds * TICKS_PER_SECOND
}

fn span(name: &str, start_seconds: i64, end_seconds: i64) -> Clip {
    let mut clip = Clip::media(name, "media-1");
    clip.timeline_in = ticks(start_seconds);
    clip.timeline_out = ticks(end_seconds);
    clip.source_in = 0;
    clip.source_out = ticks(end_seconds - start_seconds);
    clip
}

fn sequence(video: Vec<Clip>, audio: Vec<Clip>) -> Sequence {
    let mut seq = Sequence::new("seq", SequenceSettings::hd1080p30());
    let mut video_track = Track::new(0, TrackKind::Video);
    video_track.clips = video;
    seq.video_tracks.push(video_track);
    let mut audio_track = Track::new(0, TrackKind::Audio);
    audio_track.clips = audio;
    seq.audio_tracks.push(audio_track);
    seq
}

fn find<'a>(seq: &'a Sequence, name: &str, at_seconds: i64) -> &'a Clip {
    let at = ticks(at_seconds);
    seq.video_tracks
        .iter()
        .chain(seq.audio_tracks.iter())
        .flat_map(|track| track.clips.iter())
        .find(|clip| clip.name == name && clip.timeline_in <= at && clip.timeline_out > at)
        .unwrap_or_else(|| panic!("missing {name} at {at_seconds}s"))
}

fn names_at(seq: &Sequence, track: usize, video: bool) -> Vec<(&str, i64, i64)> {
    let tracks = if video {
        &seq.video_tracks
    } else {
        &seq.audio_tracks
    };
    tracks[track]
        .clips
        .iter()
        .map(|clip| (clip.name.as_str(), clip.timeline_in, clip.timeline_out))
        .collect()
}

#[test]
fn insert_shifts_later_clips_and_sync_locked_tracks() {
    let mut seq = sequence(vec![span("V", 0, 2)], vec![span("A", 0, 2)]);
    let video_id = seq.video_tracks[0].id.clone();
    insert(
        &mut seq,
        ticks(1),
        vec![Placement {
            track_id: video_id,
            clip: span("NEW", 0, 1),
        }],
    )
    .unwrap();

    assert_eq!(
        names_at(&seq, 0, true),
        vec![
            ("V", ticks(0), ticks(1)),
            ("NEW", ticks(1), ticks(2)),
            ("V", ticks(2), ticks(3)),
        ]
    );
    assert_eq!(
        names_at(&seq, 0, false),
        vec![("A", ticks(0), ticks(1)), ("A", ticks(2), ticks(3))]
    );
    assert_eq!(find(&seq, "V", 0).source_out, ticks(1));
    assert_eq!(find(&seq, "V", 2).source_in, ticks(1));
}

#[test]
fn insert_rejects_an_untargeted_track() {
    let mut seq = sequence(vec![], vec![]);
    let video_id = seq.video_tracks[0].id.clone();
    set_track_targeted(&mut seq, &video_id, false).unwrap();
    let error = insert(
        &mut seq,
        0,
        vec![Placement {
            track_id: video_id,
            clip: span("NEW", 0, 1),
        }],
    )
    .unwrap_err();
    assert!(matches!(error, EditError::TrackNotTargeted(_)));
    assert!(seq.video_tracks[0].clips.is_empty());
}

#[test]
fn overwrite_replaces_a_range_without_shifting() {
    let mut seq = sequence(
        vec![span("V", 0, 4), span("TAIL", 4, 6)],
        vec![span("A", 0, 4)],
    );
    let video_id = seq.video_tracks[0].id.clone();
    overwrite(&mut seq, &video_id, span("NEW", 0, 2), ticks(1)).unwrap();

    assert_eq!(
        names_at(&seq, 0, true),
        vec![
            ("V", ticks(0), ticks(1)),
            ("NEW", ticks(1), ticks(3)),
            ("V", ticks(3), ticks(4)),
            ("TAIL", ticks(4), ticks(6)),
        ]
    );
    assert_eq!(find(&seq, "V", 0).source_out, ticks(1));
    assert_eq!(find(&seq, "V", 3).source_in, ticks(3));
    assert_eq!(names_at(&seq, 0, false), vec![("A", ticks(0), ticks(4))]);
}

#[test]
fn lift_leaves_a_gap_on_targeted_tracks_only() {
    let mut seq = sequence(vec![span("V", 0, 4)], vec![span("A", 0, 4)]);
    let audio_id = seq.audio_tracks[0].id.clone();
    set_track_targeted(&mut seq, &audio_id, false).unwrap();
    let removed = lift(&mut seq, ticks(1), ticks(3)).unwrap();

    assert_eq!(removed.len(), 1);
    assert_eq!(
        names_at(&seq, 0, true),
        vec![("V", ticks(0), ticks(1)), ("V", ticks(3), ticks(4))]
    );
    assert_eq!(names_at(&seq, 0, false), vec![("A", ticks(0), ticks(4))]);
}

#[test]
fn extract_closes_the_gap_and_ripples_sync_locked_tracks() {
    let mut seq = sequence(
        vec![span("V", 0, 4), span("TAIL", 4, 6)],
        vec![span("A", 0, 6)],
    );
    let audio_id = seq.audio_tracks[0].id.clone();
    set_track_targeted(&mut seq, &audio_id, false).unwrap();
    extract(&mut seq, ticks(1), ticks(3)).unwrap();

    assert_eq!(
        names_at(&seq, 0, true),
        vec![
            ("V", ticks(0), ticks(1)),
            ("V", ticks(1), ticks(2)),
            ("TAIL", ticks(2), ticks(4))
        ]
    );
    assert_eq!(
        names_at(&seq, 0, false),
        vec![("A", ticks(0), ticks(1)), ("A", ticks(1), ticks(4))]
    );
    assert_eq!(find(&seq, "A", 1).source_in, ticks(3));
}

#[test]
fn sync_lock_off_keeps_the_other_track_still() {
    let mut seq = sequence(vec![span("V", 0, 4)], vec![span("A", 0, 4)]);
    let audio_id = seq.audio_tracks[0].id.clone();
    set_sync_lock(&mut seq, &audio_id, false).unwrap();
    set_track_targeted(&mut seq, &audio_id, false).unwrap();
    extract(&mut seq, ticks(1), ticks(2)).unwrap();

    assert_eq!(names_at(&seq, 0, false), vec![("A", ticks(0), ticks(4))]);
    assert_eq!(find(&seq, "V", 1).timeline_in, ticks(1));
}

#[test]
fn razor_splits_linked_audio_and_video() {
    let mut video = span("V", 0, 4);
    let mut audio = span("A", 0, 4);
    video.linked_ids = vec![audio.id.clone()];
    audio.linked_ids = vec![video.id.clone()];
    let video_id = video.id.clone();
    let mut seq = sequence(vec![video], vec![audio]);
    let pairs = razor(&mut seq, ticks(2), None).unwrap();

    assert_eq!(pairs.len(), 2);
    assert!(pairs.iter().any(|(left, _)| left == &video_id));
    let left = find(&seq, "V", 0);
    let right = find(&seq, "V", 2);
    assert_eq!((left.timeline_in, left.timeline_out), (ticks(0), ticks(2)));
    assert_eq!((left.source_in, left.source_out), (0, ticks(2)));
    assert_eq!(
        (right.timeline_in, right.timeline_out),
        (ticks(2), ticks(4))
    );
    assert_eq!((right.source_in, right.source_out), (ticks(2), ticks(4)));
    assert_eq!(left.linked_ids.len(), 1);
    assert_eq!(right.linked_ids.len(), 1);
    assert_ne!(left.linked_ids, right.linked_ids);
    assert_eq!(find(&seq, "A", 2).linked_ids, vec![right.id.clone()]);
}

#[test]
fn razor_maps_reversed_and_double_speed_source() {
    let mut reversed = span("REV", 0, 4);
    reversed.reversed = true;
    let mut fast = span("FAST", 0, 2);
    fast.source_out = ticks(4);
    fast.speed = 2.0;
    let mut seq = sequence(vec![reversed], vec![]);
    let mut fast_track = Track::new(1, TrackKind::Video);
    fast_track.clips = vec![fast];
    seq.video_tracks.push(fast_track);
    razor(&mut seq, ticks(1), None).unwrap();

    let left = find(&seq, "REV", 0);
    let right = find(&seq, "REV", 1);
    assert_eq!((left.source_in, left.source_out), (ticks(3), ticks(4)));
    assert_eq!((right.source_in, right.source_out), (0, ticks(3)));
    assert_eq!(source_at(left, ticks(0)), ticks(4));

    let fast_left = find(&seq, "FAST", 0);
    let fast_right = find(&seq, "FAST", 1);
    assert_eq!(fast_left.source_out, ticks(2));
    assert_eq!(fast_right.source_in, ticks(2));
}

#[test]
fn ripple_trim_out_moves_followers() {
    let mut seq = sequence(
        vec![span("A", 0, 2), span("B", 2, 4)],
        vec![span("S", 0, 4)],
    );
    let audio_id = seq.audio_tracks[0].id.clone();
    set_track_targeted(&mut seq, &audio_id, false).unwrap();
    let clip_id = find(&seq, "A", 0).id.clone();
    ripple_trim(&mut seq, &clip_id, TrimSide::Out, -ticks(1)).unwrap();

    let trimmed = find(&seq, "A", 0);
    assert_eq!(trimmed.timeline_out, ticks(1));
    assert_eq!(trimmed.source_out, ticks(1));
    assert_eq!(find(&seq, "B", 1).timeline_in, ticks(1));
    assert_eq!(find(&seq, "B", 1).timeline_out, ticks(3));
    assert_eq!(
        names_at(&seq, 0, false),
        vec![("S", ticks(0), ticks(1)), ("S", ticks(1), ticks(3))]
    );
    assert_eq!(find(&seq, "S", 1).source_in, ticks(2));
}

#[test]
fn ripple_trim_in_changes_the_source_head_and_keeps_the_cut() {
    let mut clip = span("A", 0, 4);
    clip.source_in = ticks(2);
    clip.source_out = ticks(6);
    let mut seq = sequence(vec![clip, span("B", 4, 6)], vec![]);
    let clip_id = find(&seq, "A", 0).id.clone();
    ripple_trim(&mut seq, &clip_id, TrimSide::In, ticks(1)).unwrap();

    let trimmed = find(&seq, "A", 0);
    assert_eq!(trimmed.timeline_in, ticks(0));
    assert_eq!(trimmed.timeline_out, ticks(5));
    assert_eq!(trimmed.source_in, ticks(1));
    assert_eq!(find(&seq, "B", 5).timeline_in, ticks(5));
}

#[test]
fn ripple_trim_rejects_media_that_is_not_there_without_changing_the_sequence() {
    let mut seq = sequence(vec![span("A", 0, 2)], vec![]);
    let before = seq.clone();
    let clip_id = find(&seq, "A", 0).id.clone();
    let error = ripple_trim(&mut seq, &clip_id, TrimSide::In, ticks(1)).unwrap_err();
    assert_eq!(error, EditError::InsufficientSource);
    assert_eq!(seq, before);
}

#[test]
fn roll_trim_moves_the_cut_between_adjacent_clips() {
    let mut right = span("B", 2, 4);
    right.source_in = ticks(1);
    right.source_out = ticks(3);
    let mut seq = sequence(vec![span("A", 0, 2), right], vec![]);
    let left_id = find(&seq, "A", 0).id.clone();
    roll_trim(&mut seq, &left_id, ticks(1)).unwrap();

    let left = find(&seq, "A", 0);
    let right = find(&seq, "B", 3);
    assert_eq!((left.timeline_out, left.source_out), (ticks(3), ticks(3)));
    assert_eq!(
        (right.timeline_in, right.timeline_out),
        (ticks(3), ticks(4))
    );
    assert_eq!((right.source_in, right.source_out), (ticks(2), ticks(3)));
}

#[test]
fn roll_trim_requires_adjacent_clips() {
    let mut seq = sequence(vec![span("A", 0, 2), span("B", 3, 4)], vec![]);
    let left_id = find(&seq, "A", 0).id.clone();
    assert_eq!(
        roll_trim(&mut seq, &left_id, ticks(1)).unwrap_err(),
        EditError::NotAdjacent
    );
}

#[test]
fn slip_moves_the_source_window() {
    let mut clip = span("A", 0, 2);
    clip.source_in = ticks(1);
    clip.source_out = ticks(3);
    let mut seq = sequence(vec![clip], vec![]);
    let clip_id = find(&seq, "A", 0).id.clone();
    slip(&mut seq, &clip_id, -ticks(1)).unwrap();

    let slipped = find(&seq, "A", 0);
    assert_eq!(
        (slipped.timeline_in, slipped.timeline_out),
        (ticks(0), ticks(2))
    );
    assert_eq!((slipped.source_in, slipped.source_out), (0, ticks(2)));
    assert_eq!(
        slip(&mut seq, &clip_id, -ticks(1)).unwrap_err(),
        EditError::InsufficientSource
    );
}

#[test]
fn slide_moves_a_clip_between_its_neighbors() {
    let mut tail = span("C", 4, 6);
    tail.source_in = ticks(1);
    tail.source_out = ticks(3);
    let mut seq = sequence(vec![span("A", 0, 2), span("B", 2, 4), tail], vec![]);
    let clip_id = find(&seq, "B", 2).id.clone();
    slide(&mut seq, &clip_id, ticks(1)).unwrap();

    assert_eq!(find(&seq, "A", 0).timeline_out, ticks(3));
    assert_eq!(find(&seq, "A", 0).source_out, ticks(3));
    assert_eq!(
        (
            find(&seq, "B", 3).timeline_in,
            find(&seq, "B", 3).timeline_out
        ),
        (ticks(3), ticks(5))
    );
    assert_eq!(find(&seq, "B", 3).source_in, 0);
    assert_eq!(find(&seq, "C", 5).timeline_in, ticks(5));
    assert_eq!(find(&seq, "C", 5).source_in, ticks(2));
    assert_eq!(find(&seq, "C", 5).timeline_out, ticks(6));
}

#[test]
fn ripple_delete_removes_a_linked_pair_and_closes_the_gap_once() {
    let mut video = span("B", 2, 4);
    let mut audio = span("B", 2, 4);
    video.linked_ids = vec![audio.id.clone()];
    audio.linked_ids = vec![video.id.clone()];
    let video_id = video.id.clone();
    let mut seq = sequence(
        vec![span("A", 0, 2), video, span("C", 4, 6)],
        vec![span("A", 0, 2), audio, span("C", 4, 6)],
    );
    ripple_delete(&mut seq, &[video_id]).unwrap();

    assert_eq!(
        names_at(&seq, 0, true),
        vec![("A", ticks(0), ticks(2)), ("C", ticks(2), ticks(4))]
    );
    assert_eq!(
        names_at(&seq, 0, false),
        vec![("A", ticks(0), ticks(2)), ("C", ticks(2), ticks(4))]
    );
}

#[test]
fn track_targeting_and_lock_guard_edits() {
    let mut seq = sequence(vec![span("V", 0, 2)], vec![]);
    let video_id = seq.video_tracks[0].id.clone();
    set_track_locked(&mut seq, &video_id, true).unwrap();
    assert!(matches!(
        lift(&mut seq, 0, ticks(1)).unwrap_err(),
        EditError::TrackLocked(_)
    ));
    assert_eq!(seq.video_tracks[0].clips.len(), 1);
}

#[test]
fn snapping_prefers_the_nearest_clip_edge_marker_or_playhead() {
    let mut seq = sequence(vec![span("V", 0, 2)], vec![]);
    let marker_id = add_marker(
        &mut seq,
        MarkerDraft {
            time_ticks: ticks(5),
            duration_ticks: 0,
            name: "note".into(),
            comment: "look here".into(),
            color: 2,
            kind: MarkerKind::Comment,
        },
    )
    .unwrap();
    move_marker(&mut seq, &marker_id, ticks(4)).unwrap();

    let snapped = snap_ticks(&seq, ticks(4) - 10, ticks(1), Some(ticks(3)), &[]);
    assert_eq!(snapped, ticks(4));
    let points = videocut_timeline::collect_snap_points(&seq, Some(ticks(3)), &[]);
    assert!(points
        .iter()
        .any(|(time, kind)| *time == ticks(2) && *kind == SnapKind::ClipOut));
    assert!(points
        .iter()
        .any(|(time, kind)| *time == ticks(3) && *kind == SnapKind::Playhead));
    assert!(points
        .iter()
        .any(|(time, kind)| *time == ticks(4) && *kind == SnapKind::Marker));

    let removed = remove_marker(&mut seq, &marker_id).unwrap();
    assert_eq!(removed.name, "note");
    assert!(seq.markers.is_empty());
}

#[test]
fn link_and_unlink_keep_both_sides_in_agreement() {
    let mut seq = sequence(vec![span("V", 0, 2)], vec![span("A", 0, 2)]);
    let video_id = find(&seq, "V", 0).id.clone();
    let audio_id = find(&seq, "A", 0).id.clone();
    link_clips(&mut seq, &[video_id.clone(), audio_id.clone()]).unwrap();
    assert_eq!(find(&seq, "V", 0).linked_ids, vec![audio_id.clone()]);
    assert_eq!(find(&seq, "A", 0).linked_ids, vec![video_id.clone()]);
    unlink_clip(&mut seq, &video_id).unwrap();
    assert!(find(&seq, "V", 0).linked_ids.is_empty());
    assert!(find(&seq, "A", 0).linked_ids.is_empty());
}

#[test]
fn set_speed_changes_duration_and_set_duration_changes_speed() {
    let mut seq = sequence(
        vec![span("V", 0, 2), span("TAIL", 2, 3)],
        vec![span("A", 0, 2)],
    );
    let clip_id = find(&seq, "V", 0).id.clone();
    set_speed(&mut seq, &clip_id, 2.0, true).unwrap();

    let fast = find(&seq, "V", 0);
    assert_eq!(fast.speed, 2.0);
    assert_eq!((fast.source_in, fast.source_out), (0, ticks(2)));
    assert_eq!(fast.timeline_out, ticks(1));
    assert_eq!(find(&seq, "TAIL", 1).timeline_in, ticks(1));
    assert_eq!(find(&seq, "A", 0).timeline_out, ticks(1));

    let blocked = set_duration(&mut seq, &clip_id, ticks(4), false).unwrap_err();
    assert_eq!(blocked, EditError::Overlap);
    set_duration(&mut seq, &clip_id, ticks(4), true).unwrap();
    let longer = find(&seq, "V", 0);
    assert_eq!(longer.timeline_out, ticks(4));
    assert!((longer.speed - 0.5).abs() < 1e-9);
    assert_eq!(longer.source_out, ticks(2));
    assert_eq!(find(&seq, "TAIL", 4).timeline_in, ticks(4));
}

#[test]
fn reverse_flips_playback_without_moving_the_clip() {
    let mut seq = sequence(vec![span("V", 0, 2)], vec![]);
    let clip_id = find(&seq, "V", 0).id.clone();
    set_reversed(&mut seq, &clip_id, true).unwrap();
    let clip = find(&seq, "V", 0);
    assert!(clip.reversed);
    assert_eq!((clip.timeline_in, clip.timeline_out), (ticks(0), ticks(2)));
    assert_eq!(source_at(clip, ticks(0)), ticks(2));
    assert_eq!(source_at(clip, ticks(2)), 0);
}

#[test]
fn nest_replaces_clips_with_one_nested_sequence() {
    let mut seq = sequence(vec![span("A", 0, 2), span("B", 2, 5)], vec![]);
    let ids = vec![find(&seq, "A", 0).id.clone(), find(&seq, "B", 2).id.clone()];
    let child = nest(&mut seq, &ids, "Nest").unwrap();

    assert_eq!(child.video_tracks.len(), 1);
    assert_eq!(child.video_tracks[0].clips.len(), 2);
    assert_eq!(child.video_tracks[0].clips[0].timeline_in, 0);
    assert_eq!(child.video_tracks[0].clips[1].timeline_in, ticks(2));
    assert_eq!(seq.video_tracks[0].clips.len(), 1);
    let nested = &seq.video_tracks[0].clips[0];
    assert_eq!(nested.kind, videocut_model::ClipKind::Nested);
    assert_eq!(
        nested.nested_sequence_id.as_deref(),
        Some(child.id.as_str())
    );
    assert_eq!((nested.timeline_in, nested.timeline_out), (0, ticks(5)));
    assert_eq!(nested.media_id, None);
}

#[test]
fn switch_multicam_angle_splits_and_realigns_source() {
    let mut seq = sequence(vec![span("CAM", 0, 4)], vec![]);
    let clip_id = find(&seq, "CAM", 0).id.clone();
    add_multicam_angle(&mut seq, "A", "media-1", 0);
    let angle_b = add_multicam_angle(&mut seq, "B", "media-b", ticks(10));
    let switched = switch_multicam_angle(&mut seq, &clip_id, ticks(2), &angle_b).unwrap();

    let left = find(&seq, "CAM", 0);
    let right = find(&seq, "CAM", 2);
    assert_eq!(left.media_id.as_deref(), Some("media-1"));
    assert_eq!((left.source_in, left.source_out), (0, ticks(2)));
    assert_eq!(right.id, switched);
    assert_eq!(right.media_id.as_deref(), Some("media-b"));
    assert_eq!((right.source_in, right.source_out), (ticks(12), ticks(14)));
}

#[test]
fn freeze_frame_inserts_a_hold_and_pushes_the_tail() {
    let mut seq = sequence(vec![span("V", 0, 4), span("TAIL", 4, 6)], vec![]);
    let clip_id = find(&seq, "V", 0).id.clone();
    let freeze_id = freeze_frame(&mut seq, &clip_id, ticks(2), ticks(1)).unwrap();

    let freeze = seq.video_tracks[0]
        .clips
        .iter()
        .find(|clip| clip.id == freeze_id)
        .unwrap();
    assert_eq!(freeze.speed, 0.0);
    assert_eq!((freeze.source_in, freeze.source_out), (ticks(2), ticks(2)));
    assert_eq!(
        (freeze.timeline_in, freeze.timeline_out),
        (ticks(2), ticks(3))
    );
    assert_eq!(find(&seq, "V", 0).timeline_out, ticks(2));
    assert_eq!(find(&seq, "V", 3).timeline_in, ticks(3));
    assert_eq!(find(&seq, "V", 3).source_in, ticks(2));
    assert_eq!(find(&seq, "TAIL", 5).timeline_in, ticks(5));
}

#[test]
fn locked_sync_track_rejects_an_insert_and_keeps_the_sequence() {
    let mut seq = sequence(vec![span("V", 0, 2)], vec![span("A", 0, 2)]);
    let before = seq.clone();
    let audio_id = seq.audio_tracks[0].id.clone();
    let video_id = seq.video_tracks[0].id.clone();
    set_track_locked(&mut seq, &audio_id, true).unwrap();
    let error = insert(
        &mut seq,
        ticks(1),
        vec![Placement {
            track_id: video_id,
            clip: span("NEW", 0, 1),
        }],
    )
    .unwrap_err();
    assert!(matches!(error, EditError::TrackLocked(_)));
    seq.audio_tracks[0].locked = false;
    assert_eq!(seq.video_tracks[0].clips, before.video_tracks[0].clips);
    assert_eq!(seq.audio_tracks[0].clips, before.audio_tracks[0].clips);
}
