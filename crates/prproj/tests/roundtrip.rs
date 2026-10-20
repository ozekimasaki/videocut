use std::fs;

use videocut_model::{
    effect_ids, AudioStreamInfo, BinItemKind, BinNode, Clip, ClipKind, EffectInstance, Interp,
    Keyframe, Marker, MarkerKind, MediaRef, Param, ParamValue, Point2, Project, Rational, Sequence,
    SequenceSettings, Track, TrackKind, Transition, TransitionAlign, VideoStreamInfo,
    TICKS_PER_SECOND,
};
use videocut_prproj::{read_prproj, write_prproj, PrprojDocument};

const TICK: i64 = TICKS_PER_SECOND;

#[test]
fn fixture_xml_reads_editorial_data_and_keeps_unknown_objects() {
    let xml = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/sequence.xml"
    ))
    .expect("fixture");
    let doc = PrprojDocument::read_xml(&xml).expect("parse fixture");
    assert!(
        doc.warnings()
            .iter()
            .any(|warning| warning.tag == "LumetriColor"),
        "missing warning: {:?}",
        doc.warnings()
    );

    let project = doc.project();
    assert_eq!(project.id, "proj-demo");
    assert_eq!(project.name, "Demo");
    assert_eq!(project.bins.len(), 2);
    assert_eq!(project.bins[0].name, "Footage");
    assert_eq!(project.bins[0].kind, BinItemKind::Bin);
    assert_eq!(project.bins[0].children.len(), 2);
    assert_eq!(project.bins[0].children[0].kind, BinItemKind::Clip);
    assert_eq!(
        project.bins[0].children[0].media_id.as_deref(),
        Some("media-interview")
    );
    assert_eq!(project.bins[0].children[1].name, "Selects");
    assert!(project.bins[0].children[1].children.is_empty());
    assert_eq!(project.bins[1].kind, BinItemKind::Sequence);
    assert_eq!(project.bins[1].sequence_id.as_deref(), Some("seq-main"));

    assert_eq!(project.media.len(), 1);
    let media = &project.media[0];
    assert_eq!(media.id, "media-interview");
    assert_eq!(media.path, r"C:\media\interview.mp4");
    assert_eq!(media.duration_ticks, TICK * 2);
    assert_eq!(
        media.proxy_path.as_deref(),
        Some(r"C:\media\interview-proxy.mp4")
    );
    let video = media.video.as_ref().expect("video");
    assert_eq!((video.width, video.height), (1920, 1080));
    assert_eq!(video.fps, Rational { num: 30, den: 1 });
    assert_eq!(video.codec, "h264");
    let audio = media.audio.as_ref().expect("audio");
    assert_eq!(audio.sample_rate, 48_000);
    assert_eq!(audio.channels, 2);

    let sequence = &project.sequences[0];
    assert_eq!(sequence.name, "Main");
    assert_eq!(sequence.settings.width, 1920);
    assert_eq!(sequence.settings.height, 1080);
    assert_eq!(sequence.settings.fps, Rational { num: 30, den: 1 });
    assert_eq!(sequence.settings.sample_rate, 48_000);
    assert_eq!(sequence.settings.audio_channels, 2);
    assert_eq!(sequence.markers.len(), 1);
    assert_eq!(sequence.markers[0].id, "marker-cue");
    assert_eq!(sequence.markers[0].name, "cue");
    assert_eq!(sequence.markers[0].comment, "note");
    assert_eq!(sequence.markers[0].kind, MarkerKind::Chapter);
    assert_eq!(sequence.markers[0].color, 1);
    assert_eq!(sequence.markers[0].time_ticks, TICK / 2);

    let video_track = &sequence.video_tracks[0];
    assert_eq!(video_track.clips.len(), 2);
    let first = &video_track.clips[0];
    assert_eq!(first.timeline_in, 0);
    assert_eq!(first.timeline_out, TICK);
    assert_eq!(first.source_in, 0);
    assert_eq!(first.source_out, TICK);
    assert_eq!(first.speed, 1.0);
    assert!(!first.reversed);
    assert_eq!(first.label_color, 3);
    assert_eq!(first.linked_ids, vec!["clip-a1".to_string()]);
    assert_eq!(first.effects.len(), 2);
    assert_eq!(first.effects[0].effect_id, effect_ids::OPACITY);
    assert_eq!(first.effects[0].params[0].id, "opacity");
    assert_eq!(first.effects[0].params[0].value, ParamValue::Float(50.0));
    assert_eq!(first.effects[0].params[0].keyframes.len(), 2);
    assert_eq!(first.effects[0].params[0].keyframes[1].interp, Interp::Hold);
    assert_eq!(first.effects[1].effect_id, effect_ids::MOTION);
    assert_eq!(
        first.effects[1].params[0].value,
        ParamValue::Point(Point2 { x: 100.0, y: 200.0 })
    );
    assert_eq!(first.effects[1].params[1].value, ParamValue::Float(80.0));

    let second = &video_track.clips[1];
    assert_eq!(second.speed, 2.0);
    assert!(second.reversed);
    assert_eq!(second.source_out, TICK * 2);
    assert_eq!(video_track.transitions.len(), 1);
    let dissolve = &video_track.transitions[0];
    assert_eq!(dissolve.effect_id, effect_ids::CROSS_DISSOLVE);
    assert_eq!(dissolve.alignment, TransitionAlign::Center);
    assert_eq!(dissolve.duration_ticks, TICK / 2);
    assert_eq!(dissolve.center_ticks, TICK);

    let audio_clip = &sequence.audio_tracks[0].clips[0];
    assert_eq!(audio_clip.linked_ids, vec!["clip-v1".to_string()]);
    assert_eq!(audio_clip.media_id.as_deref(), Some("media-interview"));

    let saved = doc.to_xml();
    assert!(saved.contains("LumetriColor"));
    assert!(saved.contains("unsupported-grade"));
    let again = PrprojDocument::read_xml(&saved).expect("reparse");
    assert_eq!(again.project(), doc.project());
    assert!(again
        .warnings()
        .iter()
        .any(|warning| warning.tag == "LumetriColor"));

    let gzip = doc.to_bytes().expect("gzip");
    assert_eq!(&gzip[..2], b"\x1f\x8b");
    let from_gzip = PrprojDocument::read_bytes(&gzip).expect("gunzip");
    assert_eq!(from_gzip.project(), doc.project());
}

#[test]
fn project_gzip_round_trip_preserves_mapped_fields() {
    let project = sample_project();
    let bytes = write_prproj(&project).expect("write");
    assert_eq!(&bytes[..2], b"\x1f\x8b");
    let (back, warnings) = read_prproj(&bytes).expect("read");
    assert!(
        warnings.iter().all(|warning| warning.tag != "LumetriColor"),
        "fresh project should not invent unknown objects"
    );
    assert_eq!(back, project);
}

#[test]
fn fractional_frame_rates_and_unknown_effects_round_trip() {
    let mut project = sample_project();
    project.sequences[0].settings.fps = Rational {
        num: 30_000,
        den: 1001,
    };
    project.media[0].video.as_mut().expect("video").fps = Rational {
        num: 24_000,
        den: 1001,
    };
    project.sequences[0].video_tracks[0].clips[0]
        .effects
        .push(EffectInstance {
            id: "fx-custom".into(),
            effect_id: "Custom.Grade".into(),
            enabled: false,
            params: vec![Param {
                id: "amount".into(),
                value: ParamValue::Float(0.25),
                keyframes: Vec::new(),
            }],
        });
    let bytes = write_prproj(&project).expect("write");
    let (back, warnings) = read_prproj(&bytes).expect("read");
    assert_eq!(
        back.sequences[0].settings.fps,
        Rational {
            num: 30_000,
            den: 1001
        }
    );
    assert_eq!(
        back.media[0].video.as_ref().expect("video").fps,
        Rational {
            num: 24_000,
            den: 1001
        }
    );
    let custom = back.sequences[0].video_tracks[0].clips[0]
        .effects
        .iter()
        .find(|effect| effect.id == "fx-custom")
        .expect("custom effect");
    assert_eq!(custom.effect_id, "Custom.Grade");
    assert!(!custom.enabled);
    assert_eq!(custom.params[0].id, "amount");
    assert_eq!(custom.params[0].value, ParamValue::Float(0.25));
    assert!(warnings
        .iter()
        .any(|warning| warning.message.contains("Custom.Grade")));
}

#[test]
fn plain_xml_and_corrupt_gzip_are_distinguished() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8" ?><PremiereData Version="3"><Project ObjectID="1" ClassID="62ad66dd-0dcd-42da-a660-6d8fbde94876" Version="1"><Name>Empty</Name></Project></PremiereData>"#;
    let doc = PrprojDocument::read_bytes(xml.as_bytes()).expect("plain xml");
    assert_eq!(doc.project().name, "Empty");

    let err = PrprojDocument::read_bytes(b"\x1f\x8bnot-a-real-gzip").expect_err("bad gzip");
    let message = err.to_string();
    assert!(message.contains("gzip"), "{message}");

    let err = PrprojDocument::read_bytes(b"hello").expect_err("not a project");
    assert!(err.to_string().contains("not gzip"));
}

fn sample_project() -> Project {
    let media_id = "media-interview".to_string();
    let mut video = Clip::media("interview.mp4", &media_id);
    video.id = "clip-v1".into();
    video.timeline_out = TICK;
    video.source_out = TICK;
    video.label_color = 4;
    video.linked_ids = vec!["clip-a1".into()];
    video.effects = vec![
        EffectInstance {
            id: "fx-opacity".into(),
            effect_id: effect_ids::OPACITY.into(),
            enabled: true,
            params: vec![Param {
                id: "opacity".into(),
                value: ParamValue::Float(75.0),
                keyframes: vec![Keyframe {
                    time_ticks: TICK / 4,
                    value: ParamValue::Float(10.0),
                    interp: Interp::Linear,
                }],
            }],
        },
        EffectInstance {
            id: "fx-motion".into(),
            effect_id: effect_ids::MOTION.into(),
            enabled: true,
            params: vec![
                Param {
                    id: "position".into(),
                    value: ParamValue::Point(Point2 { x: 960.0, y: 540.0 }),
                    keyframes: Vec::new(),
                },
                Param {
                    id: "anchor".into(),
                    value: ParamValue::Point(Point2 { x: 0.0, y: 0.0 }),
                    keyframes: Vec::new(),
                },
                Param {
                    id: "scale".into(),
                    value: ParamValue::Float(100.0),
                    keyframes: Vec::new(),
                },
                Param {
                    id: "rotation".into(),
                    value: ParamValue::Float(15.0),
                    keyframes: vec![Keyframe {
                        time_ticks: 0,
                        value: ParamValue::Float(15.0),
                        interp: Interp::Bezier,
                    }],
                },
            ],
        },
    ];

    let mut sped = Clip::media("interview.mp4", &media_id);
    sped.id = "clip-v2".into();
    sped.timeline_in = TICK;
    sped.timeline_out = TICK * 2;
    sped.source_out = TICK * 4;
    sped.speed = 2.0;
    sped.reversed = true;

    let mut nested = Clip::media("Main", "");
    nested.id = "clip-nest".into();
    nested.media_id = None;
    nested.nested_sequence_id = Some("seq-main".into());
    nested.kind = ClipKind::Nested;
    nested.timeline_in = TICK * 2;
    nested.timeline_out = TICK * 3;
    nested.source_out = TICK;

    let mut title = Clip::media("Lower", "");
    title.id = "clip-title".into();
    title.media_id = None;
    title.kind = ClipKind::Title;
    title.timeline_in = 0;
    title.timeline_out = TICK / 2;
    title.source_out = TICK / 2;

    let mut audio = Clip::media("interview.mp4", &media_id);
    audio.id = "clip-a1".into();
    audio.timeline_out = TICK;
    audio.source_out = TICK;
    audio.linked_ids = vec!["clip-v1".into()];

    let mut video_track = Track::new(0, TrackKind::Video);
    video_track.id = "track-v1".into();
    video_track.clips = vec![video, sped];
    video_track.transitions = vec![Transition {
        id: "tr-dissolve".into(),
        effect_id: effect_ids::CROSS_DISSOLVE.into(),
        center_ticks: TICK,
        duration_ticks: TICK / 2,
        alignment: TransitionAlign::Center,
    }];

    let mut title_track = Track::new(1, TrackKind::Video);
    title_track.id = "track-v2".into();
    title_track.hidden = true;
    title_track.locked = true;
    title_track.sync_lock = false;
    title_track.targeted = false;
    title_track.clips = vec![nested, title];

    let mut audio_track = Track::new(0, TrackKind::Audio);
    audio_track.id = "track-a1".into();
    audio_track.muted = true;
    audio_track.clips = vec![audio];

    Project {
        id: "proj-demo".into(),
        name: "Demo".into(),
        bins: vec![
            BinNode {
                id: "bin-footage".into(),
                name: "Footage".into(),
                kind: BinItemKind::Bin,
                media_id: None,
                sequence_id: None,
                children: vec![
                    BinNode {
                        id: "item-interview".into(),
                        name: "interview.mp4".into(),
                        kind: BinItemKind::Clip,
                        media_id: Some(media_id.clone()),
                        sequence_id: None,
                        children: Vec::new(),
                    },
                    BinNode {
                        id: "bin-selects".into(),
                        name: "Selects".into(),
                        kind: BinItemKind::Bin,
                        media_id: None,
                        sequence_id: None,
                        children: Vec::new(),
                    },
                ],
            },
            BinNode {
                id: "item-seq".into(),
                name: "Main".into(),
                kind: BinItemKind::Sequence,
                media_id: None,
                sequence_id: Some("seq-main".into()),
                children: Vec::new(),
            },
        ],
        sequences: vec![Sequence {
            id: "seq-main".into(),
            name: "Main".into(),
            settings: SequenceSettings::hd1080p30(),
            video_tracks: vec![video_track, title_track],
            audio_tracks: vec![audio_track],
            markers: vec![Marker {
                id: "marker-cue".into(),
                time_ticks: TICK / 2,
                duration_ticks: TICK / 10,
                name: "cue".into(),
                comment: "note".into(),
                color: 1,
                kind: MarkerKind::Chapter,
            }],
            captions: Vec::new(),
            multicam_angles: Vec::new(),
        }],
        media: vec![MediaRef {
            id: media_id,
            name: "interview.mp4".into(),
            path: r"C:\media\interview.mp4".into(),
            duration_ticks: TICK * 4,
            video: Some(VideoStreamInfo {
                width: 1920,
                height: 1080,
                fps: Rational { num: 30, den: 1 },
                codec: "h264".into(),
            }),
            audio: Some(AudioStreamInfo {
                sample_rate: 48_000,
                channels: 2,
                codec: "aac".into(),
            }),
            proxy_path: Some(r"C:\media\interview-proxy.mp4".into()),
        }],
    }
}
