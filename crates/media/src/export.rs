use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use videocut_model::{Project, Sequence};

use crate::error::MediaError;
use crate::graph::{self, build_timeline_graph, GraphOptions};
use crate::proxy::{ensure_extension, prelude};
use crate::sidecar::{encoder_listed, path_arg, FfmpegSidecar};
use crate::time::format_rational;

/// Encoders accepted for a ProRes MOV, in preference order.
pub const PRORES_ENCODERS: &[&str] = &["prores_ks", "prores_aw", "prores"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProResProfile {
    Proxy,
    Lt,
    Standard,
    Hq,
    FourFourFour,
}

impl ProResProfile {
    pub fn ffmpeg_profile(self) -> u8 {
        match self {
            Self::Proxy => 0,
            Self::Lt => 1,
            Self::Standard => 2,
            Self::Hq => 3,
            Self::FourFourFour => 4,
        }
    }

    pub fn pixel_format(self) -> &'static str {
        match self {
            Self::FourFourFour => "yuva444p10le",
            Self::Proxy | Self::Lt | Self::Standard | Self::Hq => "yuv422p10le",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportPreset {
    /// H.264 video and AAC audio in an MP4 container.
    H264Mp4 { crf: u8 },
    /// ProRes in a MOV container when the ffmpeg build has a ProRes encoder.
    ProResMov { profile: ProResProfile },
    /// PCM signed 16-bit WAV. Picture is not written.
    Wav,
}

impl ExportPreset {
    pub fn h264_mp4() -> Self {
        Self::H264Mp4 { crf: 18 }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::H264Mp4 { .. } => "h264",
            Self::ProResMov { .. } => "prores",
            Self::Wav => "wav",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::H264Mp4 { .. } => "mp4",
            Self::ProResMov { .. } => "mov",
            Self::Wav => "wav",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportRange {
    Entire,
    InOut { in_ticks: i64, out_ticks: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportRequest {
    pub sequence_id: String,
    pub preset: ExportPreset,
    pub output_path: PathBuf,
    pub range: ExportRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPlan {
    pub args: Vec<String>,
    pub filter_complex: String,
    pub script_path: PathBuf,
    pub duration_ticks: i64,
    pub warnings: Vec<String>,
    pub output_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportResult {
    pub output_path: PathBuf,
    pub duration_ticks: i64,
    pub warnings: Vec<String>,
    pub filter_complex: String,
}

pub fn select_prores_encoder(listing: &str) -> Option<&'static str> {
    PRORES_ENCODERS
        .iter()
        .copied()
        .find(|name| encoder_listed(listing, name))
}

/// Build the ffmpeg invocation for a sequence. Does not run ffmpeg and does
/// not require a ProRes encoder to be installed.
pub fn plan_export(project: &Project, request: &ExportRequest) -> Result<ExportPlan, MediaError> {
    let sequence = project
        .sequences
        .iter()
        .find(|sequence| sequence.id == request.sequence_id)
        .ok_or_else(|| MediaError::SequenceNotFound(request.sequence_id.clone()))?;
    ensure_extension(
        &request.output_path,
        request.preset.extension(),
        request.preset.label(),
    )?;
    if let ExportPreset::H264Mp4 { crf } = request.preset {
        if crf > 51 {
            return Err(MediaError::Invalid("crf must be 0..=51".into()));
        }
    }
    let (range_in, range_out) = resolve_range(sequence, request.range)?;
    let options = graph_options(&request.preset);
    let graph = build_timeline_graph(project, sequence, range_in, range_out, &options)?;
    let script_path = filter_script_path(&request.output_path);
    let mut args = prelude();
    let fps = format_rational(sequence.settings.fps)?;
    for input in &graph.inputs {
        if input.still {
            let ticks = if input.still_duration_ticks > 0 {
                input.still_duration_ticks
            } else {
                videocut_model::TICKS_PER_SECOND
            };
            args.extend([
                "-loop".into(),
                "1".into(),
                "-framerate".into(),
                fps.clone(),
                "-t".into(),
                crate::time::format_ticks_as_seconds(ticks),
            ]);
        }
        args.push("-i".into());
        args.push(input.path.clone());
    }
    args.push("-filter_complex_script".into());
    args.push(path_arg(&script_path));
    if let Some(label) = &graph.video_label {
        args.push("-map".into());
        args.push(format!("[{label}]"));
    }
    if let Some(label) = &graph.audio_label {
        args.push("-map".into());
        args.push(format!("[{label}]"));
    }
    args.extend(codec_args(
        &request.preset,
        graph.audio_label.is_some(),
        &graph,
    )?);
    args.push(path_arg(&request.output_path));
    Ok(ExportPlan {
        args,
        filter_complex: graph.filter_complex,
        script_path,
        duration_ticks: graph.duration_ticks,
        warnings: graph.warnings,
        output_path: request.output_path.clone(),
    })
}

pub fn export_sequence(
    sidecar: &FfmpegSidecar,
    project: &Project,
    request: &ExportRequest,
) -> Result<ExportResult, MediaError> {
    let mut plan = plan_export(project, request)?;
    if let ExportPreset::ProResMov { .. } = request.preset {
        let listing = sidecar.encoder_listing()?;
        let encoder =
            select_prores_encoder(&listing).ok_or_else(|| MediaError::EncoderUnavailable {
                name: "prores_ks".into(),
            })?;
        if let Some(slot) = plan.args.iter_mut().find(|arg| arg.as_str() == "prores_ks") {
            *slot = encoder.to_string();
        }
    }
    if let Some(parent) = plan.output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    if let Some(parent) = plan.script_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(&plan.script_path, format!("{}\n", plan.filter_complex))?;
    sidecar.run(&plan.args).map_err(|error| match error {
        MediaError::FfmpegFailed { message } => MediaError::FfmpegFailed {
            message: format!("{message} (filter graph: {})", plan.script_path.display()),
        },
        other => other,
    })?;
    Ok(ExportResult {
        output_path: plan.output_path,
        duration_ticks: plan.duration_ticks,
        warnings: plan.warnings,
        filter_complex: plan.filter_complex,
    })
}

pub fn resolve_range(sequence: &Sequence, range: ExportRange) -> Result<(i64, i64), MediaError> {
    match range {
        ExportRange::Entire => {
            let end = graph::sequence_end_ticks(sequence);
            if end <= 0 {
                return Err(MediaError::EmptyTimeline);
            }
            Ok((0, end))
        }
        ExportRange::InOut {
            in_ticks,
            out_ticks,
        } => {
            if out_ticks <= in_ticks {
                return Err(MediaError::EmptyRange);
            }
            Ok((in_ticks, out_ticks))
        }
    }
}

fn graph_options(preset: &ExportPreset) -> GraphOptions {
    match preset {
        ExportPreset::H264Mp4 { .. } => GraphOptions::h264(),
        ExportPreset::ProResMov { profile } => GraphOptions::prores(profile.pixel_format()),
        ExportPreset::Wav => GraphOptions::audio_only(),
    }
}

fn codec_args(
    preset: &ExportPreset,
    has_audio: bool,
    graph: &graph::FilterGraph,
) -> Result<Vec<String>, MediaError> {
    let mut args = Vec::new();
    match preset {
        ExportPreset::H264Mp4 { crf } => {
            args.extend([
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                "medium".into(),
                "-crf".into(),
                crf.to_string(),
                "-pix_fmt".into(),
                "yuv420p".into(),
            ]);
            if has_audio {
                args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "320k".into()]);
            }
            args.extend(["-movflags".into(), "+faststart".into()]);
        }
        ExportPreset::ProResMov { profile } => {
            args.extend([
                "-c:v".into(),
                "prores_ks".into(),
                "-profile:v".into(),
                profile.ffmpeg_profile().to_string(),
                "-pix_fmt".into(),
                profile.pixel_format().into(),
            ]);
            if has_audio {
                args.extend(["-c:a".into(), "pcm_s16le".into()]);
            }
        }
        ExportPreset::Wav => {
            args.extend([
                "-c:a".into(),
                "pcm_s16le".into(),
                "-ar".into(),
                graph.sample_rate.to_string(),
                "-ac".into(),
                graph.audio_channels.to_string(),
            ]);
        }
    }
    Ok(args)
}

fn filter_script_path(output: &Path) -> PathBuf {
    let stem = output
        .file_stem()
        .unwrap_or(OsStr::new("export"))
        .to_string_lossy();
    let name = format!("{stem}.videocut.filtergraph.txt");
    match output.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}
