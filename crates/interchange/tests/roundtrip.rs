use videocut_interchange::{
    export_edl, export_fcpxml, export_otio, import_edl, import_fcpxml, import_otio,
};
use videocut_model::{
    effect_ids, Clip, Marker, MarkerKind, MediaRef, Project, Sequence, SequenceSettings, Track,
    TrackKind, Transition, TransitionAlign, TICKS_PER_SECOND,
};

fn seconds(value: i64) -> i64 {
    value * TICKS_PER_SECOND
}

fn interview() -> Project {
    let media_a = MediaRef {
        id: "media-a".into(),
        name: "clip-a.mov".into(),
        path: "C:/media/clip-a.mov".into(),
        duration_ticks: seconds(20),
        video: None,
        audio: None,
        proxy_path: None,
    };
    let media_b = MediaRef {
        id: "media-b".into(),
        name: "clip-b.mov".into(),
        path: "C:/media/clip-b.mov".into(),
        duration_ticks: seconds(20),
        video: None,
        audio: None,
        proxy_path: None,
    };
    let mut clip_a = Clip::media("Clip A", &media_a.id);
    clip_a.timeline_in = 0;
    clip_a.timeline_out = seconds(5);
    clip_a.source_in = seconds(1);
    clip_a.source_out = seconds(6);
    let mut clip_b = Clip::media("Clip B", &media_b.id);
    clip_b.timeline_in = seconds(5);
    clip_b.timeline_out = seconds(10);
    clip_b.source_in = seconds(2);
    clip_b.source_out = seconds(7);

    let mut track = Track::new(0, TrackKind::Video);
    track.clips = vec![clip_a, clip_b];
    track.transitions.push(Transition {
        id: "trans-dissolve".into(),
        effect_id: effect_ids::CROSS_DISSOLVE.into(),
        center_ticks: seconds(5),
        duration_ticks: seconds(1),
        alignment: TransitionAlign::Center,
    });

    let mut sequence = Sequence::new("Interview", SequenceSettings::hd1080p30());
    sequence.video_tracks.push(track);
    sequence.markers.push(Marker {
        id: "marker-note".into(),
        time_ticks: seconds(2),
        duration_ticks: 0,
        name: "Note".into(),
        comment: "hello".into(),
        color: 3,
        kind: MarkerKind::Comment,
    });

    let mut project = Project::new("Demo");
    project.media.push(media_a);
    project.media.push(media_b);
    project.sequences.push(sequence);
    project
}

fn assert_interview(project: &Project) {
    assert_eq!(project.name, "Demo");
    assert_eq!(project.sequences.len(), 1);
    let sequence = &project.sequences[0];
    assert_eq!(sequence.name, "Interview");
    assert_eq!(sequence.settings.width, 1920);
    assert_eq!(sequence.settings.height, 1080);
    assert_eq!(sequence.settings.fps.num, 30);
    assert_eq!(sequence.settings.fps.den, 1);
    assert_eq!(sequence.video_tracks.len(), 1);
    assert!(sequence.audio_tracks.is_empty());

    let track = &sequence.video_tracks[0];
    assert_eq!(track.clips.len(), 2);
    let clip_a = &track.clips[0];
    let clip_b = &track.clips[1];
    assert_eq!(clip_a.name, "Clip A");
    assert_eq!(clip_b.name, "Clip B");
    assert_eq!(clip_a.timeline_in, 0);
    assert_eq!(clip_a.timeline_out, seconds(5));
    assert_eq!(clip_b.timeline_in, seconds(5));
    assert_eq!(clip_b.timeline_out, seconds(10));
    assert_eq!(clip_a.source_in, seconds(1));
    assert_eq!(clip_a.source_out, seconds(6));
    assert_eq!(clip_b.source_in, seconds(2));
    assert_eq!(clip_b.source_out, seconds(7));

    let path = |clip: &Clip| {
        project
            .media(clip.media_id.as_ref().expect("media"))
            .expect("media ref")
            .path
            .clone()
    };
    assert_eq!(path(clip_a), "C:/media/clip-a.mov");
    assert_eq!(path(clip_b), "C:/media/clip-b.mov");

    assert_eq!(track.transitions.len(), 1);
    let transition = &track.transitions[0];
    assert_eq!(transition.effect_id, effect_ids::CROSS_DISSOLVE);
    assert_eq!(transition.center_ticks, seconds(5));
    assert_eq!(transition.duration_ticks, seconds(1));
    assert_eq!(transition.alignment, TransitionAlign::Center);

    assert_eq!(sequence.markers.len(), 1);
    let marker = &sequence.markers[0];
    assert_eq!(marker.time_ticks, seconds(2));
    assert_eq!(marker.duration_ticks, 0);
    assert_eq!(marker.name, "Note");
    assert_eq!(marker.comment, "hello");
    assert_eq!(marker.kind, MarkerKind::Comment);
}

fn assert_marker_color(project: &Project) {
    assert_eq!(project.sequences[0].markers[0].color, 3);
}

#[test]
fn otio_roundtrip_keeps_two_clips_dissolve_and_marker() {
    let project = interview();
    let exported = export_otio(&project).expect("export otio");
    assert!(exported.text.contains("\"OTIO_SCHEMA\": \"Timeline.1\""));
    assert!(exported.text.contains("SMPTE_Dissolve"));
    assert!(exported.text.contains("\"OTIO_SCHEMA\": \"Marker.2\""));
    assert!(exported.text.contains("Note"));
    let imported = import_otio(&exported.text).expect("import otio");
    assert_interview(&imported.project);
    assert_marker_color(&imported.project);
}

#[test]
fn fcpxml_roundtrip_keeps_two_clips_dissolve_and_marker() {
    let project = interview();
    let exported = export_fcpxml(&project).expect("export fcpxml");
    assert!(exported.text.contains("<fcpxml version=\"1.9\">"));
    assert!(exported.text.contains("<transition "));
    assert!(exported.text.contains("value=\"Note\""));
    assert!(exported.text.contains("note=\"hello\""));
    let imported = import_fcpxml(&exported.text).expect("import fcpxml");
    assert_interview(&imported.project);
    assert_marker_color(&imported.project);
}

#[test]
fn edl_roundtrip_keeps_two_clips_dissolve_and_marker() {
    let project = interview();
    let exported = export_edl(&project).expect("export edl");
    assert!(exported.text.contains("TITLE: Interview"));
    assert!(exported.text.contains("FCM: NON-DROP FRAME"));
    assert!(exported.text.contains(" D    030") || exported.text.contains("\n002"));
    assert!(exported.text.contains("MARKER:"));
    assert!(exported.text.contains("Note"));
    let imported = import_edl(&exported.text).expect("import edl");
    assert_interview(&imported.project);
    assert_marker_color(&imported.project);
}

#[test]
fn handwritten_fcpxml_and_edl_import_the_same_sequence() {
    let fcpxml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE fcpxml>
<fcpxml version="1.9">
  <resources>
    <format id="r1" name="FFVideoFormat1080p30" frameDuration="1/30s" width="1920" height="1080"/>
    <asset id="r2" name="clip-a.mov" src="file:///C:/media/clip-a.mov" start="0s" duration="20s" hasVideo="1" format="r1"/>
    <asset id="r3" name="clip-b.mov" src="file:///C:/media/clip-b.mov" start="0s" duration="20s" hasVideo="1" format="r1"/>
    <effect id="r4" name="Cross Dissolve" uid="videocut.builtin.cross-dissolve"/>
  </resources>
  <library>
    <event name="Demo">
      <project name="Interview">
        <sequence format="r1" tcStart="0s" tcFormat="NDF" audioRate="48k" audioLayout="stereo">
          <spine>
            <asset-clip ref="r2" name="Clip A" offset="0s" start="1s" duration="11/2s">
              <marker start="3s" duration="0s" value="Note" note="hello"/>
            </asset-clip>
            <transition ref="r4" name="Cross Dissolve" offset="9/2s" duration="1s"/>
            <asset-clip ref="r3" name="Clip B" offset="9/2s" start="3/2s" duration="11/2s"/>
          </spine>
        </sequence>
      </project>
    </event>
  </library>
</fcpxml>
"#;
    assert_interview(&import_fcpxml(fcpxml).expect("fcpxml").project);

    let edl = "\
TITLE: Interview
FCM: NON-DROP FRAME

001  CLIPA    V     C        00:00:01:00 00:00:06:15 00:00:00:00 00:00:05:15
* FROM CLIP NAME: Clip A
* SOURCE FILE: C:/media/clip-a.mov
002  CLIPB    V     D    030 00:00:01:15 00:00:07:00 00:00:04:15 00:00:10:00
* FROM CLIP NAME: Clip B
* SOURCE FILE: C:/media/clip-b.mov
* MARKER: 00:00:02:00 | Note | hello
";
    let imported = import_edl(edl).expect("edl");
    assert_eq!(imported.project.sequences[0].name, "Interview");
    assert_eq!(imported.project.sequences[0].video_tracks[0].clips.len(), 2);
    assert_eq!(
        imported.project.sequences[0].video_tracks[0].clips[0].timeline_out,
        seconds(5)
    );
    assert_eq!(
        imported.project.sequences[0].video_tracks[0].clips[1].timeline_in,
        seconds(5)
    );
    assert_eq!(
        imported.project.sequences[0].video_tracks[0].transitions[0].duration_ticks,
        seconds(1)
    );
    assert_eq!(imported.project.sequences[0].markers[0].name, "Note");
    assert_eq!(imported.project.sequences[0].markers[0].comment, "hello");
    assert_eq!(
        imported.project.sequences[0].markers[0].time_ticks,
        seconds(2)
    );
}

#[test]
fn otio_gap_and_start_aligned_dissolve_roundtrip() {
    let mut project = interview();
    let sequence = &mut project.sequences[0];
    sequence.video_tracks[0].clips[1].timeline_in = seconds(8);
    sequence.video_tracks[0].clips[1].timeline_out = seconds(12);
    sequence.video_tracks[0].transitions.clear();
    sequence.markers.clear();
    let exported = export_otio(&project).expect("export");
    assert!(exported.text.contains("Gap.1"));
    let imported = import_otio(&exported.text).expect("import");
    let track = &imported.project.sequences[0].video_tracks[0];
    assert_eq!(track.clips[0].timeline_out, seconds(5));
    assert_eq!(track.clips[1].timeline_in, seconds(8));
    assert!(track.transitions.is_empty());

    let mut project = interview();
    let transition = &mut project.sequences[0].video_tracks[0].transitions[0];
    transition.alignment = TransitionAlign::Start;
    transition.duration_ticks = seconds(1);
    transition.center_ticks = seconds(5) + seconds(1) / 2;
    let exported = export_otio(&project).expect("export start");
    let imported = import_otio(&exported.text).expect("import start");
    let transition = &imported.project.sequences[0].video_tracks[0].transitions[0];
    assert_eq!(transition.alignment, TransitionAlign::Start);
    assert_eq!(transition.duration_ticks, seconds(1));
    assert_eq!(transition.center_ticks, seconds(5) + seconds(1) / 2);
}

#[test]
fn foreign_otio_timeline_places_clips_around_a_transition() {
    let otio = r#"{
      "OTIO_SCHEMA": "Timeline.1",
      "name": "Cuts",
      "global_start_time": {"OTIO_SCHEMA": "RationalTime.1", "rate": 24, "value": 0},
      "metadata": {},
      "tracks": {
        "OTIO_SCHEMA": "Stack.1",
        "name": "tracks",
        "children": [{
          "OTIO_SCHEMA": "Track.1",
          "name": "V1",
          "kind": "Video",
          "markers": [{
            "OTIO_SCHEMA": "Marker.2",
            "name": "Note",
            "comment": "hello",
            "color": "GREEN",
            "marked_range": {
              "OTIO_SCHEMA": "TimeRange.1",
              "start_time": {"OTIO_SCHEMA": "RationalTime.1", "rate": 24, "value": 48},
              "duration": {"OTIO_SCHEMA": "RationalTime.1", "rate": 24, "value": 0}
            },
            "metadata": {}
          }],
          "children": [
            {
              "OTIO_SCHEMA": "Clip.1",
              "name": "Clip A",
              "source_range": {
                "OTIO_SCHEMA": "TimeRange.1",
                "start_time": {"OTIO_SCHEMA": "RationalTime.1", "rate": 24, "value": 24},
                "duration": {"OTIO_SCHEMA": "RationalTime.1", "rate": 24, "value": 120}
              },
              "media_reference": {
                "OTIO_SCHEMA": "ExternalReference.1",
                "name": "a.mov",
                "target_url": "file:///media/a.mov"
              }
            },
            {
              "OTIO_SCHEMA": "Transition.1",
              "name": "Cross Dissolve",
              "transition_type": "SMPTE_Dissolve",
              "in_offset": {"OTIO_SCHEMA": "RationalTime.1", "rate": 24, "value": 12},
              "out_offset": {"OTIO_SCHEMA": "RationalTime.1", "rate": 24, "value": 12}
            },
            {
              "OTIO_SCHEMA": "Clip.1",
              "name": "Clip B",
              "source_range": {
                "OTIO_SCHEMA": "TimeRange.1",
                "start_time": {"OTIO_SCHEMA": "RationalTime.1", "rate": 24, "value": 0},
                "duration": {"OTIO_SCHEMA": "RationalTime.1", "rate": 24, "value": 96}
              },
              "media_reference": {
                "OTIO_SCHEMA": "ExternalReference.1",
                "name": "b.mov",
                "target_url": "file:///media/b.mov"
              }
            }
          ]
        }]
      }
    }"#;
    let imported = import_otio(otio).expect("import");
    let sequence = &imported.project.sequences[0];
    assert_eq!(
        sequence.settings.fps,
        videocut_model::Rational { num: 24, den: 1 }
    );
    let frame = TICKS_PER_SECOND / 24;
    let track = &sequence.video_tracks[0];
    assert_eq!(track.clips[0].timeline_in, 0);
    assert_eq!(track.clips[0].timeline_out, 120 * frame);
    assert_eq!(track.clips[0].source_in, 24 * frame);
    assert_eq!(track.clips[1].timeline_in, 120 * frame);
    assert_eq!(track.clips[1].timeline_out, 216 * frame);
    assert_eq!(track.transitions.len(), 1);
    assert_eq!(track.transitions[0].duration_ticks, 24 * frame);
    assert_eq!(track.transitions[0].center_ticks, 120 * frame);
    assert_eq!(sequence.markers[0].name, "Note");
    assert_eq!(sequence.markers[0].comment, "hello");
    assert_eq!(sequence.markers[0].time_ticks, 48 * frame);
    assert_eq!(
        imported
            .project
            .media(track.clips[0].media_id.as_ref().unwrap())
            .unwrap()
            .path,
        "/media/a.mov"
    );
}

#[test]
fn invalid_documents_are_rejected() {
    assert!(import_otio("{").is_err());
    assert!(import_fcpxml("not xml").is_err());
    assert!(import_edl("   ").is_err());
    assert!(export_otio(&Project::new("empty")).is_err());
}
