use std::path::PathBuf;

use videocut_media::{
    build_timeline_graph, plan_export, plan_proxy, ExportPreset, ExportRange, ExportRequest,
    GraphOptions, MediaError, ProResProfile, ProxyOptions,
};
use videocut_model::{
    effect_ids, AudioStreamInfo, Clip, ClipKind, MediaRef, Project, Rational, Sequence,
    SequenceSettings, Track, TrackKind, Transition, TransitionAlign, VideoStreamInfo,
    TICKS_PER_SECOND,
};

fn sec(seconds: i64) -> i64 {
    seconds * TICKS_PER_SECOND
}

fn video_audio(id: &str, path: &str) -> MediaRef {
    MediaRef {
        id: id.into(),
        name: path.into(),
        path: path.into(),
        duration_ticks: sec(10),
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
        proxy_path: None,
    }
}

fn placed(
    name: &str,
    media_id: &str,
    start: i64,
    end: i64,
    source_in: i64,
    source_out: i64,
) -> Clip {
    let mut clip = Clip::media(name, media_id);
    clip.timeline_in = start;
    clip.timeline_out = end;
    clip.source_in = source_in;
    clip.source_out = source_out;
    clip
}

fn sequence_with(
    video: Vec<Clip>,
    audio: Vec<Clip>,
    video_transitions: Vec<Transition>,
) -> (Project, Sequence) {
    let mut project = Project::new("demo");
    project.media.push(video_audio("a", "C:/media/a.mp4"));
    project.media.push(video_audio("b", "C:/media/b.mp4"));
    let mut sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
    let mut video_track = Track::new(0, TrackKind::Video);
    video_track.clips = video;
    video_track.transitions = video_transitions;
    let mut audio_track = Track::new(0, TrackKind::Audio);
    audio_track.clips = audio;
    sequence.video_tracks.push(video_track);
    sequence.audio_tracks.push(audio_track);
    project.sequences.push(sequence.clone());
    let sequence = project.sequences[0].clone();
    (project, sequence)
}

fn hard_cut_project() -> (Project, Sequence) {
    sequence_with(
        vec![
            placed("a", "a", 0, sec(1), 0, sec(1)),
            placed("b", "b", sec(1), sec(2), 0, sec(1)),
        ],
        vec![
            placed("a", "a", 0, sec(1), 0, sec(1)),
            placed("b", "b", sec(1), sec(2), 0, sec(1)),
        ],
        Vec::new(),
    )
}

const HARD_CUT: &str = "\
[0:v]trim=start=0:end=1,setpts=PTS-STARTPTS,scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:black,fps=30/1,setsar=1,format=yuv420p[v0];\
[1:v]trim=start=0:end=1,setpts=PTS-STARTPTS,scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:black,fps=30/1,setsar=1,format=yuv420p[v1];\
[v0][v1]concat=n=2:v=1:a=0[vout];\
[0:a]atrim=start=0:end=1,asetpts=PTS-STARTPTS,aformat=sample_fmts=fltp:sample_rates=48000:channel_layouts=stereo[a0];\
[1:a]atrim=start=0:end=1,asetpts=PTS-STARTPTS,aformat=sample_fmts=fltp:sample_rates=48000:channel_layouts=stereo[a1];\
[a0][a1]concat=n=2:v=0:a=1[aout]";

#[test]
fn hard_cut_graph_matches_the_documented_example() {
    let (project, sequence) = hard_cut_project();
    let graph =
        build_timeline_graph(&project, &sequence, 0, sec(2), &GraphOptions::h264()).unwrap();
    assert_eq!(graph.filter_complex, HARD_CUT);
    assert_eq!(graph.video_label.as_deref(), Some("vout"));
    assert_eq!(graph.audio_label.as_deref(), Some("aout"));
    assert_eq!(graph.duration_ticks, sec(2));
    assert!(graph.warnings.is_empty());
    let doc = include_str!("../FILTER_GRAPH.md");
    assert!(
        doc.contains(HARD_CUT),
        "FILTER_GRAPH.md does not contain the hard-cut graph"
    );
    assert!(doc.contains("xfade=transition=fade"));
    assert!(doc.contains("acrossfade"));
    assert!(doc.contains("ffmpeg.exe"));
}

#[test]
fn cross_dissolve_uses_xfade_and_acrossfade() {
    let half = TICKS_PER_SECOND / 2;
    let transition = Transition {
        id: "tr-1".into(),
        effect_id: effect_ids::CROSS_DISSOLVE.into(),
        center_ticks: half + half / 2,
        duration_ticks: half,
        alignment: TransitionAlign::Center,
    };
    let (mut project, _) = sequence_with(
        vec![
            placed("a", "a", 0, sec(1), 0, sec(1)),
            placed("b", "b", half, sec(1) + half, 0, sec(1)),
        ],
        vec![
            placed("a", "a", 0, sec(1), 0, sec(1)),
            placed("b", "b", half, sec(1) + half, 0, sec(1)),
        ],
        vec![transition.clone()],
    );
    project.sequences[0].audio_tracks[0].transitions = vec![transition];
    let sequence = project.sequences[0].clone();
    let graph =
        build_timeline_graph(&project, &sequence, 0, sec(1) + half, &GraphOptions::h264()).unwrap();
    assert!(graph
        .filter_complex
        .contains("xfade=transition=fade:duration=0.5:offset=0"));
    assert!(graph.filter_complex.contains("acrossfade=d=0.5"));
    assert!(graph.filter_complex.contains("concat=n=3:v=1:a=0"));
    assert!(graph.warnings.is_empty(), "{:?}", graph.warnings);
}

#[test]
fn gap_is_black_and_a_lower_track_is_not_composited() {
    let (mut project, _) = sequence_with(
        vec![placed("a", "a", sec(1), sec(2), 0, sec(1))],
        Vec::new(),
        Vec::new(),
    );
    let mut lower = Track::new(0, TrackKind::Video);
    lower.clips.push(placed("b", "b", 0, sec(2), 0, sec(2)));
    project.sequences[0].video_tracks.insert(0, lower);
    project.sequences[0].video_tracks[1].index = 1;
    let sequence = project.sequences[0].clone();
    let graph =
        build_timeline_graph(&project, &sequence, 0, sec(2), &GraphOptions::h264()).unwrap();
    assert!(graph.filter_complex.contains("color=c=black"));
    assert!(graph.filter_complex.contains("duration=1"));
    assert!(graph
        .warnings
        .iter()
        .any(|warning| warning.contains("not composited")));
}

#[test]
fn overlap_without_a_dissolve_lets_the_incoming_clip_win_and_restores_the_tail() {
    let (project, _) = sequence_with(
        vec![
            placed("a", "a", 0, sec(10), 0, sec(10)),
            placed("b", "b", sec(2), sec(4), 0, sec(2)),
        ],
        Vec::new(),
        Vec::new(),
    );
    let sequence = project.sequences[0].clone();
    let graph =
        build_timeline_graph(&project, &sequence, 0, sec(10), &GraphOptions::h264()).unwrap();
    assert!(graph.filter_complex.contains("trim=start=0:end=2"));
    assert!(graph.filter_complex.contains("trim=start=4:end=10"));
    assert!(graph.filter_complex.contains("concat=n=3:v=1:a=0"));
    assert!(!graph.filter_complex.contains("xfade"));
}

#[test]
fn speed_and_reverse_are_filter_operations() {
    let (mut project, _) = sequence_with(
        vec![placed("a", "a", 0, sec(1), 0, sec(2))],
        vec![placed("a", "a", 0, sec(1), 0, sec(2))],
        Vec::new(),
    );
    project.sequences[0].video_tracks[0].clips[0].speed = 2.0;
    project.sequences[0].audio_tracks[0].clips[0].speed = 2.0;
    let sequence = project.sequences[0].clone();
    let graph =
        build_timeline_graph(&project, &sequence, 0, sec(1), &GraphOptions::h264()).unwrap();
    assert!(graph.filter_complex.contains("setpts=(PTS-STARTPTS)/2"));
    assert!(graph.filter_complex.contains("atempo=2"));

    project.sequences[0].video_tracks[0].clips[0].speed = 1.0;
    project.sequences[0].video_tracks[0].clips[0].reversed = true;
    project.sequences[0].video_tracks[0].clips[0].source_out = sec(1);
    project.sequences[0].audio_tracks[0].clips[0].speed = 1.0;
    project.sequences[0].audio_tracks[0].clips[0].reversed = true;
    project.sequences[0].audio_tracks[0].clips[0].source_out = sec(1);
    let sequence = project.sequences[0].clone();
    let graph =
        build_timeline_graph(&project, &sequence, 0, sec(1), &GraphOptions::h264()).unwrap();
    assert!(graph.filter_complex.contains("reverse"));
    assert!(graph.filter_complex.contains("areverse"));
}

#[test]
fn in_out_range_trims_the_source() {
    let (project, _) = sequence_with(
        vec![placed("a", "a", 0, sec(2), 0, sec(2))],
        vec![placed("a", "a", 0, sec(2), 0, sec(2))],
        Vec::new(),
    );
    let sequence = project.sequences[0].clone();
    let graph =
        build_timeline_graph(&project, &sequence, sec(1), sec(2), &GraphOptions::h264()).unwrap();
    assert!(graph.filter_complex.contains("[0:v]trim=start=1:end=2"));
    assert!(graph.filter_complex.contains("[vout]"));
    assert!(!graph.filter_complex.contains("concat"));
    assert_eq!(graph.duration_ticks, sec(1));
}

#[test]
fn other_transitions_are_cuts_and_stills_loop() {
    let half = TICKS_PER_SECOND / 2;
    let (mut project, _) = sequence_with(
        vec![
            placed("a", "a", 0, sec(1), 0, sec(1)),
            placed("b", "b", half, sec(1) + half, 0, sec(1)),
        ],
        Vec::new(),
        vec![Transition {
            id: "tr-dip".into(),
            effect_id: effect_ids::DIP_TO_BLACK.into(),
            center_ticks: sec(1) - half / 2,
            duration_ticks: half,
            alignment: TransitionAlign::Center,
        }],
    );
    let sequence = project.sequences[0].clone();
    let graph =
        build_timeline_graph(&project, &sequence, 0, sec(1) + half, &GraphOptions::h264()).unwrap();
    assert!(graph
        .warnings
        .iter()
        .any(|warning| warning.contains(effect_ids::DIP_TO_BLACK)
            && warning.contains("exported as a cut")));
    assert!(!graph.filter_complex.contains("xfade"));

    project.media.push(MediaRef {
        id: "still".into(),
        name: "title.png".into(),
        path: "C:/media/title.png".into(),
        duration_ticks: 0,
        video: Some(VideoStreamInfo {
            width: 1280,
            height: 720,
            fps: Rational { num: 1, den: 1 },
            codec: "png".into(),
        }),
        audio: None,
        proxy_path: None,
    });
    let mut still_sequence = Sequence::new("stills", SequenceSettings::hd1080p30());
    let mut track = Track::new(0, TrackKind::Video);
    track
        .clips
        .push(placed("title", "still", 0, sec(1), 0, sec(1)));
    still_sequence.video_tracks.push(track);
    project.sequences = vec![still_sequence];
    let sequence = project.sequences[0].clone();
    let graph =
        build_timeline_graph(&project, &sequence, 0, sec(1), &GraphOptions::h264()).unwrap();
    assert!(graph.filter_complex.contains("trim=duration=1"));
    assert!(graph
        .inputs
        .iter()
        .any(|input| input.still && input.path.ends_with("title.png")));
    let plan = plan_export(
        &project,
        &ExportRequest {
            sequence_id: sequence.id.clone(),
            preset: ExportPreset::h264_mp4(),
            output_path: PathBuf::from("C:/out/title.mp4"),
            range: ExportRange::Entire,
        },
    )
    .unwrap();
    assert!(plan
        .args
        .windows(2)
        .any(|pair| pair[0] == "-loop" && pair[1] == "1"));
    let path_args = plan
        .args
        .iter()
        .filter(|arg| arg.contains("My Media") || arg.contains("title.png"))
        .count();
    assert!(path_args >= 1);
}

#[test]
fn export_plans_select_containers_and_codecs() {
    let (project, sequence) = hard_cut_project();
    let h264 = plan_export(
        &project,
        &ExportRequest {
            sequence_id: sequence.id.clone(),
            preset: ExportPreset::h264_mp4(),
            output_path: PathBuf::from("C:/My Media/out.mp4"),
            range: ExportRange::Entire,
        },
    )
    .unwrap();
    assert!(h264.args.iter().any(|arg| arg == "C:/My Media/out.mp4"));
    assert!(h264.args.iter().any(|arg| arg == "-filter_complex_script"));
    assert!(h264.args.iter().any(|arg| arg == "libx264"));
    assert_eq!(h264.filter_complex, HARD_CUT);

    let prores = plan_export(
        &project,
        &ExportRequest {
            sequence_id: sequence.id.clone(),
            preset: ExportPreset::ProResMov {
                profile: ProResProfile::Hq,
            },
            output_path: PathBuf::from("out.mov"),
            range: ExportRange::Entire,
        },
    )
    .unwrap();
    assert!(prores.args.iter().any(|arg| arg == "prores_ks"));
    assert!(prores
        .args
        .windows(2)
        .any(|pair| pair[0] == "-profile:v" && pair[1] == "3"));
    assert!(prores.filter_complex.contains("format=yuv422p10le"));
    assert_eq!(ProResProfile::Proxy.ffmpeg_profile(), 0);
    assert_eq!(ProResProfile::FourFourFour.pixel_format(), "yuva444p10le");

    let wav = plan_export(
        &project,
        &ExportRequest {
            sequence_id: sequence.id,
            preset: ExportPreset::Wav,
            output_path: PathBuf::from("out.wav"),
            range: ExportRange::Entire,
        },
    )
    .unwrap();
    assert!(wav.args.iter().any(|arg| arg == "pcm_s16le"));
    assert!(!wav.args.iter().any(|arg| arg == "[vout]"));
    assert!(!wav.filter_complex.contains("scale="));

    let error = plan_export(
        &project,
        &ExportRequest {
            sequence_id: "missing".into(),
            preset: ExportPreset::h264_mp4(),
            output_path: PathBuf::from("out.mp4"),
            range: ExportRange::Entire,
        },
    )
    .unwrap_err();
    assert!(matches!(error, MediaError::SequenceNotFound(_)));
}

#[test]
fn empty_timeline_and_audio_mix_are_explicit() {
    let mut project = Project::new("empty");
    let sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
    project.sequences.push(sequence);
    let error = plan_export(
        &project,
        &ExportRequest {
            sequence_id: project.sequences[0].id.clone(),
            preset: ExportPreset::h264_mp4(),
            output_path: PathBuf::from("out.mp4"),
            range: ExportRange::Entire,
        },
    )
    .unwrap_err();
    assert!(matches!(error, MediaError::EmptyTimeline));

    let (mut project, _) = sequence_with(
        Vec::new(),
        vec![placed("a", "a", 0, sec(1), 0, sec(1))],
        Vec::new(),
    );
    let mut second = Track::new(1, TrackKind::Audio);
    second.clips.push(placed("b", "b", 0, sec(1), 0, sec(1)));
    project.sequences[0].audio_tracks.push(second);
    let sequence = project.sequences[0].clone();
    let graph =
        build_timeline_graph(&project, &sequence, 0, sec(1), &GraphOptions::h264()).unwrap();
    assert!(graph
        .filter_complex
        .contains("amix=inputs=2:duration=longest:dropout_transition=0:normalize=0"));
}

#[test]
fn proxy_plan_keeps_a_spaced_path_as_one_argument() {
    let media = video_audio("a", "C:/My Media/clip.mp4");
    let plan = plan_proxy(
        &media,
        std::path::Path::new("proxy.mp4"),
        &ProxyOptions::default(),
    )
    .unwrap();
    assert_eq!(
        plan.args
            .iter()
            .filter(|arg| arg.as_str() == "C:/My Media/clip.mp4")
            .count(),
        1
    );
}

#[test]
fn titles_are_reported_instead_of_being_dropped_quietly() {
    let (mut project, _) = sequence_with(Vec::new(), Vec::new(), Vec::new());
    let mut title = Clip::media("lower third", "a");
    title.kind = ClipKind::Title;
    title.media_id = None;
    title.timeline_out = sec(1);
    project.sequences[0].video_tracks[0].clips.push(title);
    let sequence = project.sequences[0].clone();
    let graph =
        build_timeline_graph(&project, &sequence, 0, sec(1), &GraphOptions::h264()).unwrap();
    assert!(graph
        .warnings
        .iter()
        .any(|warning| warning.contains("title")));
    assert!(graph.filter_complex.contains("color=c=black"));
}
