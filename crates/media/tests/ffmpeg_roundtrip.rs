use std::path::{Path, PathBuf};

use videocut_media::{
    export_sequence, generate_proxy, import_media, waveform_peaks, write_poster, ExportPreset,
    ExportRange, ExportRequest, FfmpegSidecar, MediaError, ProResProfile, ProxyOptions,
    WaveformOptions,
};
use videocut_model::{
    effect_ids, Clip, MediaRef, Project, Sequence, SequenceSettings, Track, TrackKind, Transition,
    TransitionAlign, TICKS_PER_SECOND,
};

fn sidecar() -> Option<FfmpegSidecar> {
    FfmpegSidecar::locate().ok()
}

fn run(sidecar: &FfmpegSidecar, args: &[&str]) {
    let args: Vec<String> = args.iter().copied().map(str::to_string).collect();
    sidecar
        .run(&args)
        .unwrap_or_else(|error| panic!("ffmpeg failed: {error}"));
}

fn sec(seconds: i64) -> i64 {
    seconds * TICKS_PER_SECOND
}

fn near(ticks: i64, seconds: f64) {
    let actual = ticks as f64 / TICKS_PER_SECOND as f64;
    assert!(
        (actual - seconds).abs() < 0.2,
        "duration {actual}s, expected about {seconds}s"
    );
}

#[test]
fn ingest_proxy_poster_waveform_and_export_when_ffmpeg_is_installed() {
    let Some(ffmpeg) = sidecar() else {
        return;
    };
    let root = std::env::temp_dir().join(format!("videocut-media-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    let red = root.join("red.mp4");
    let blue = root.join("blue.mp4");
    let tone = root.join("tone.wav");
    let still = root.join("still.png");
    run(
        &ffmpeg,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=320x240:r=25:d=1",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=1",
            "-shortest",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-ac",
            "2",
            red.to_str().unwrap(),
        ],
    );
    run(
        &ffmpeg,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=320x240:r=25:d=1",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=880:sample_rate=48000:duration=1",
            "-shortest",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-ac",
            "2",
            blue.to_str().unwrap(),
        ],
    );
    run(
        &ffmpeg,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=1",
            "-c:a",
            "pcm_s16le",
            tone.to_str().unwrap(),
        ],
    );
    run(
        &ffmpeg,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=green:s=160x120",
            "-frames:v",
            "1",
            still.to_str().unwrap(),
        ],
    );

    let red_media = import_media(&ffmpeg, &red).unwrap();
    assert_eq!(red_media.video.as_ref().unwrap().width, 320);
    assert_eq!(red_media.video.as_ref().unwrap().height, 240);
    assert_eq!(red_media.audio.as_ref().unwrap().channels, 2);
    near(red_media.duration_ticks, 1.0);

    let tone_media = import_media(&ffmpeg, &tone).unwrap();
    assert!(tone_media.video.is_none());
    assert_eq!(tone_media.audio.as_ref().unwrap().sample_rate, 48_000);
    near(tone_media.duration_ticks, 1.0);

    let still_media = import_media(&ffmpeg, &still).unwrap();
    assert_eq!(still_media.duration_ticks, 0);
    assert_eq!(still_media.video.as_ref().unwrap().width, 160);
    assert!(still_media.audio.is_none());

    let poster = root.join("poster.jpg");
    write_poster(&ffmpeg, &red_media, &poster).unwrap();
    assert!(poster.metadata().unwrap().len() > 0);

    let proxy = generate_proxy(
        &ffmpeg,
        &red_media,
        &root.join("red.proxy.mp4"),
        &ProxyOptions::default(),
    )
    .unwrap();
    assert!(proxy.proxy_path.is_some());
    let proxy_media = import_media(&ffmpeg, Path::new(proxy.proxy_path.as_ref().unwrap())).unwrap();
    assert!(proxy_media.video.as_ref().unwrap().width <= 960);

    let peaks = waveform_peaks(&ffmpeg, &tone_media, &WaveformOptions::default()).unwrap();
    assert!(!peaks.min_max.is_empty());
    assert!(peaks.min_max.iter().any(|sample| sample.abs() > 0.05));

    let exported = export_pair(&ffmpeg, &root, &red_media, &blue_media(&ffmpeg, &blue));
    near(exported.0, 2.0);
    assert!(exported.1.metadata().unwrap().len() > 0);
    assert!(exported.2.is_file(), "filter graph script was not written");

    let wav_out = root.join("mix.wav");
    let project = pair_project(&red_media, &blue_media(&ffmpeg, &blue), false);
    export_sequence(
        &ffmpeg,
        &project,
        &ExportRequest {
            sequence_id: project.sequences[0].id.clone(),
            preset: ExportPreset::Wav,
            output_path: wav_out.clone(),
            range: ExportRange::Entire,
        },
    )
    .unwrap();
    let wav_media = import_media(&ffmpeg, &wav_out).unwrap();
    near(wav_media.duration_ticks, 2.0);
    assert!(wav_media.video.is_none());

    if ffmpeg.has_encoder("prores_ks").unwrap()
        || ffmpeg.has_encoder("prores").unwrap()
        || ffmpeg.has_encoder("prores_aw").unwrap()
    {
        let mov = root.join("prores.mov");
        export_sequence(
            &ffmpeg,
            &project,
            &ExportRequest {
                sequence_id: project.sequences[0].id.clone(),
                preset: ExportPreset::ProResMov {
                    profile: ProResProfile::Proxy,
                },
                output_path: mov.clone(),
                range: ExportRange::Entire,
            },
        )
        .unwrap();
        let mov_media = import_media(&ffmpeg, &mov).unwrap();
        near(mov_media.duration_ticks, 2.0);
        assert_eq!(mov_media.video.as_ref().unwrap().codec, "prores");
    } else {
        let error = export_sequence(
            &ffmpeg,
            &project,
            &ExportRequest {
                sequence_id: project.sequences[0].id.clone(),
                preset: ExportPreset::ProResMov {
                    profile: ProResProfile::Hq,
                },
                output_path: root.join("missing.mov"),
                range: ExportRange::Entire,
            },
        )
        .unwrap_err();
        assert!(matches!(error, MediaError::EncoderUnavailable { .. }));
    }

    let dissolve = root.join("dissolve.mp4");
    let dissolved = dissolve_project(&red_media, &blue_media(&ffmpeg, &blue));
    export_sequence(
        &ffmpeg,
        &dissolved,
        &ExportRequest {
            sequence_id: dissolved.sequences[0].id.clone(),
            preset: ExportPreset::h264_mp4(),
            output_path: dissolve.clone(),
            range: ExportRange::Entire,
        },
    )
    .unwrap();
    let dissolve_media = import_media(&ffmpeg, &dissolve).unwrap();
    near(dissolve_media.duration_ticks, 1.5);

    let _ = std::fs::remove_dir_all(&root);
}

fn blue_media(ffmpeg: &FfmpegSidecar, path: &Path) -> MediaRef {
    import_media(ffmpeg, path).unwrap()
}

fn export_pair(
    ffmpeg: &FfmpegSidecar,
    root: &Path,
    red: &MediaRef,
    blue: &MediaRef,
) -> (i64, PathBuf, PathBuf) {
    let project = pair_project(red, blue, false);
    let output = root.join("cut.mp4");
    let result = export_sequence(
        ffmpeg,
        &project,
        &ExportRequest {
            sequence_id: project.sequences[0].id.clone(),
            preset: ExportPreset::H264Mp4 { crf: 28 },
            output_path: output.clone(),
            range: ExportRange::Entire,
        },
    )
    .unwrap();
    let media = import_media(ffmpeg, &output).unwrap();
    let script = root.join("cut.videocut.filtergraph.txt");
    (media.duration_ticks, result.output_path, script)
}

fn pair_project(red: &MediaRef, blue: &MediaRef, dissolve: bool) -> Project {
    let mut project = Project::new("roundtrip");
    project.media.push(red.clone());
    project.media.push(blue.clone());
    let mut sequence = Sequence::new(
        "seq",
        SequenceSettings {
            width: 320,
            height: 240,
            fps: videocut_model::Rational { num: 25, den: 1 },
            sample_rate: 48_000,
            audio_channels: 2,
            start_timecode_ticks: 0,
        },
    );
    let mut video = Track::new(0, TrackKind::Video);
    let mut audio = Track::new(0, TrackKind::Audio);
    if dissolve {
        video.clips.push(span("red", &red.id, 0, sec(1)));
        video
            .clips
            .push(span("blue", &blue.id, sec(1) / 2, sec(1) + sec(1) / 2));
        audio.clips.push(span("red", &red.id, 0, sec(1)));
        audio
            .clips
            .push(span("blue", &blue.id, sec(1) / 2, sec(1) + sec(1) / 2));
        let transition = Transition {
            id: "tr".into(),
            effect_id: effect_ids::CROSS_DISSOLVE.into(),
            center_ticks: sec(1) / 2 + sec(1) / 4,
            duration_ticks: sec(1) / 2,
            alignment: TransitionAlign::Center,
        };
        video.transitions.push(transition.clone());
        audio.transitions.push(transition);
    } else {
        video.clips.push(span("red", &red.id, 0, sec(1)));
        video.clips.push(span("blue", &blue.id, sec(1), sec(2)));
        audio.clips.push(span("red", &red.id, 0, sec(1)));
        audio.clips.push(span("blue", &blue.id, sec(1), sec(2)));
    }
    sequence.video_tracks.push(video);
    sequence.audio_tracks.push(audio);
    project.sequences.push(sequence);
    project
}

fn dissolve_project(red: &MediaRef, blue: &MediaRef) -> Project {
    pair_project(red, blue, true)
}

fn span(name: &str, media_id: &str, start: i64, end: i64) -> Clip {
    let mut clip = Clip::media(name, media_id);
    clip.timeline_in = start;
    clip.timeline_out = end;
    clip.source_in = 0;
    clip.source_out = end - start;
    clip
}

#[test]
fn ffprobe_rejects_a_missing_file_without_panicking() {
    let Some(ffmpeg) = sidecar() else {
        return;
    };
    let error = import_media(&ffmpeg, Path::new("no-such-clip.mp4")).unwrap_err();
    assert!(matches!(error, MediaError::NotFound { .. }));
}
