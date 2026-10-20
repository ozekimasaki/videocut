use videocut_model::{Project, Sequence, SequenceSettings, Track, TrackKind};

use crate::error::MediaError;
use crate::filters::{self, channel_layout, PictureFormat, SoundFormat};
use crate::spans::{self, InputTable};
use crate::time::frame_ticks;

/// A simple timeline filter graph: hard cuts, black/silence gaps, and cross dissolves.
///
/// The exact filter text for a two-clip cut is documented in `FILTER_GRAPH.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterGraph {
    pub inputs: Vec<GraphInput>,
    /// `-filter_complex` script. Empty when there is nothing to render.
    pub filter_complex: String,
    pub video_label: Option<String>,
    pub audio_label: Option<String>,
    /// Duration of the rendered range, in ticks.
    pub duration_ticks: i64,
    pub sample_rate: u32,
    /// Channel count written by the audio graph after layout fallback.
    pub audio_channels: u16,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphInput {
    pub path: String,
    pub still: bool,
    /// Decode length for a looped still. Zero for timed media.
    pub still_duration_ticks: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphOptions {
    pub include_video: bool,
    pub include_audio: bool,
    pub pixel_format: String,
}

impl GraphOptions {
    pub fn h264() -> Self {
        Self {
            include_video: true,
            include_audio: true,
            pixel_format: "yuv420p".into(),
        }
    }

    pub fn prores(pixel_format: &str) -> Self {
        Self {
            include_video: true,
            include_audio: true,
            pixel_format: pixel_format.to_string(),
        }
    }

    pub fn audio_only() -> Self {
        Self {
            include_video: false,
            include_audio: true,
            pixel_format: "yuv420p".into(),
        }
    }
}

pub fn build_timeline_graph(
    project: &Project,
    sequence: &Sequence,
    range_in: i64,
    range_out: i64,
    options: &GraphOptions,
) -> Result<FilterGraph, MediaError> {
    if range_out <= range_in {
        return Err(MediaError::EmptyRange);
    }
    let picture = picture_format(&sequence.settings, &options.pixel_format)?;
    let sound = sound_format(&sequence.settings)?;
    let mut warnings = Vec::new();
    if sound.fallback {
        warnings.push(format!(
            "audio channel count {} is exported as {}",
            sequence.settings.audio_channels, sound.format.layout
        ));
    }

    let mut inputs = InputTable::default();
    let video_filters = if options.include_video {
        render_video(
            project,
            sequence,
            range_in,
            range_out,
            &picture,
            &mut inputs,
            &mut warnings,
        )?
    } else {
        Vec::new()
    };
    let audio_filters = if options.include_audio {
        render_audio(
            project,
            sequence,
            range_in,
            range_out,
            &sound.format,
            &mut inputs,
            &mut warnings,
        )?
    } else {
        Vec::new()
    };

    let video_label = if video_filters.is_empty() {
        None
    } else {
        Some("vout".to_string())
    };
    let audio_label = if audio_filters.is_empty() {
        None
    } else {
        Some("aout".to_string())
    };
    if options.include_audio && !options.include_video && audio_label.is_none() {
        return Err(MediaError::NoAudioStream(sequence.name.clone()));
    }
    if video_label.is_none() && audio_label.is_none() {
        return Err(MediaError::EmptyTimeline);
    }

    let filter_complex = match (&video_label, &audio_label) {
        (Some(_), Some(_)) => format!("{};{}", video_filters.join(";"), audio_filters.join(";")),
        (Some(_), None) => video_filters.join(";"),
        (None, Some(_)) => audio_filters.join(";"),
        (None, None) => String::new(),
    };

    Ok(FilterGraph {
        inputs: inputs
            .inputs
            .into_iter()
            .map(|input| GraphInput {
                path: input.path,
                still: input.still,
                still_duration_ticks: input.still_duration_ticks,
            })
            .collect(),
        filter_complex,
        video_label,
        audio_label,
        duration_ticks: range_out - range_in,
        sample_rate: sound.format.sample_rate,
        audio_channels: sound.format.channels,
        warnings,
    })
}

struct SoundBuild {
    format: SoundFormat,
    fallback: bool,
}

fn picture_format(
    settings: &SequenceSettings,
    pixel_format: &str,
) -> Result<PictureFormat, MediaError> {
    if settings.width == 0 || settings.height == 0 {
        return Err(MediaError::Invalid(
            "sequence frame size must be non-zero".into(),
        ));
    }
    if pixel_format.is_empty() {
        return Err(MediaError::Invalid("pixel format is empty".into()));
    }
    Ok(PictureFormat {
        width: settings.width,
        height: settings.height,
        fps: settings.fps,
        pixel_format: pixel_format.to_string(),
        frame_ticks: frame_ticks(settings.fps)?,
    })
}

fn sound_format(settings: &SequenceSettings) -> Result<SoundBuild, MediaError> {
    if settings.sample_rate == 0 {
        return Err(MediaError::Invalid("sample rate must be non-zero".into()));
    }
    if settings.audio_channels == 0 {
        return Err(MediaError::Invalid(
            "audio channel count must be non-zero".into(),
        ));
    }
    let (layout, channels, fallback) = channel_layout(settings.audio_channels);
    Ok(SoundBuild {
        format: SoundFormat {
            sample_rate: settings.sample_rate,
            layout: layout.to_string(),
            channels,
        },
        fallback,
    })
}

fn render_video(
    project: &Project,
    sequence: &Sequence,
    range_in: i64,
    range_out: i64,
    picture: &PictureFormat,
    inputs: &mut InputTable,
    warnings: &mut Vec<String>,
) -> Result<Vec<String>, MediaError> {
    for track in &sequence.video_tracks {
        if track.kind != TrackKind::Video {
            warnings.push(format!(
                "track '{}' is listed with the video tracks but is not a video track",
                track.id
            ));
        }
    }
    let candidates: Vec<&Track> = sequence
        .video_tracks
        .iter()
        .filter(|track| track.kind == TrackKind::Video && !track.hidden && track_has_clip(track))
        .collect();
    let chosen = candidates.iter().copied().max_by_key(|track| track.index);
    if candidates.len() > 1 {
        let index = chosen.map(|track| track.index).unwrap_or(0);
        warnings.push(format!(
            "only the top visible video track (index {index}) is exported; {} other visible video track(s) are not composited",
            candidates.len() - 1
        ));
    }
    let prepared = match chosen {
        Some(track) => {
            spans::prepare_video_track(track, |id| lookup_media(project, id), inputs, warnings)?
        }
        None => Vec::new(),
    };
    let built = spans::build_spans(
        &prepared,
        chosen
            .map(|track| track.transitions.as_slice())
            .unwrap_or(&[]),
        range_in,
        range_out,
        warnings,
    )?;
    filters::render_picture(&built, picture, inputs, warnings)
}

fn render_audio(
    project: &Project,
    sequence: &Sequence,
    range_in: i64,
    range_out: i64,
    sound: &SoundFormat,
    inputs: &mut InputTable,
    warnings: &mut Vec<String>,
) -> Result<Vec<String>, MediaError> {
    let mut active = Vec::new();
    for track in &sequence.audio_tracks {
        match track.kind {
            TrackKind::Audio => {}
            TrackKind::Video => {
                warnings.push(format!(
                    "track '{}' is listed with the audio tracks but is not an audio track",
                    track.id
                ));
                continue;
            }
        }
        if track.hidden || track.muted || !track_has_clip(track) {
            continue;
        }
        let prepared =
            spans::prepare_audio_track(track, |id| lookup_media(project, id), inputs, warnings)?;
        if prepared.is_empty() {
            continue;
        }
        let built =
            spans::build_spans(&prepared, &track.transitions, range_in, range_out, warnings)?;
        active.push(built);
    }
    if active.is_empty() {
        return Ok(Vec::new());
    }
    let mut label_cursor = 0_u32;
    if active.len() == 1 {
        return filters::render_sound(
            &active[0],
            sound,
            "aout",
            &mut label_cursor,
            inputs,
            warnings,
        );
    }
    let mut filters = Vec::new();
    let mut mix_labels = Vec::new();
    for (index, spans) in active.iter().enumerate() {
        let label = format!("at{index}");
        filters.extend(filters::render_sound(
            spans,
            sound,
            &label,
            &mut label_cursor,
            inputs,
            warnings,
        )?);
        mix_labels.push(label);
    }
    filters.push(filters::amix(&mix_labels));
    Ok(filters)
}

fn track_has_clip(track: &Track) -> bool {
    track
        .clips
        .iter()
        .any(|clip| clip.enabled && clip.timeline_out > clip.timeline_in)
}

fn lookup_media(project: &Project, id: &str) -> Result<videocut_model::MediaRef, MediaError> {
    project
        .media(id)
        .cloned()
        .ok_or_else(|| MediaError::MediaNotFound(id.to_string()))
}

pub fn sequence_end_ticks(sequence: &Sequence) -> i64 {
    sequence
        .video_tracks
        .iter()
        .chain(sequence.audio_tracks.iter())
        .filter(|track| !track.hidden && !(track.kind == TrackKind::Audio && track.muted))
        .flat_map(|track| track.clips.iter())
        .filter(|clip| clip.enabled)
        .map(|clip| clip.timeline_out)
        .max()
        .unwrap_or(0)
}
