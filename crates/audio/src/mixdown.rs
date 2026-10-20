use serde::{Deserialize, Serialize};
use videocut_model::{Clip, ClipKind, Interp, MediaRef, Project, Sequence};

use crate::compressor::CompressorSettings;
use crate::curve::{sorted_gain_keys, GainKeyframe};
use crate::effects::parse_clip_audio;
use crate::eq::EqSettings;
use crate::mixer::MixerState;
use crate::pan::balance_gains;
use crate::units::{
    db_to_linear, format_ffmpeg_number, ticks_to_samples, ticks_to_seconds, FADER_MAX_DB,
    FADER_MIN_DB,
};
use crate::AudioError;

pub const FFMPEG_OUTPUT_LABEL: &str = "aout";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WarningCode {
    MissingMedia,
    NoAudioStream,
    NestedNotExpanded,
    UnsupportedClip,
    InvalidSpeed,
    EmptyClip,
    ExtraEffectIgnored,
    UnsupportedChannelCount,
    EmptySequence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixdownWarning {
    pub code: WarningCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixdownInput {
    pub index: u32,
    pub media_id: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedClip {
    pub clip_id: String,
    pub track_id: String,
    pub media_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_index: Option<u32>,
    pub timeline_in: i64,
    pub timeline_out: i64,
    pub source_in: i64,
    pub source_out: i64,
    pub speed: f64,
    pub reversed: bool,
    pub enabled: bool,
    pub rendered: bool,
    pub gain_db: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gain_keyframes: Vec<GainKeyframe>,
    pub pan: f64,
    pub left_gain: f64,
    pub right_gain: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eq: Option<EqSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compressor: Option<CompressorSettings>,
    pub delay_samples: i64,
    pub source_channels: u16,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tempo_stages: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedTrack {
    pub track_id: String,
    pub index: u32,
    pub muted: bool,
    pub solo: bool,
    pub audible: bool,
    pub fader_db: f64,
    pub pan: f64,
    pub clips: Vec<PlannedClip>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixdownPlan {
    pub sequence_id: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub channel_layout: String,
    pub duration_ticks: i64,
    pub duration_samples: i64,
    pub master_gain_db: f64,
    pub inputs: Vec<MixdownInput>,
    pub tracks: Vec<PlannedTrack>,
    pub ffmpeg_filter_complex: String,
    pub ffmpeg_output_label: String,
    #[serde(default)]
    pub warnings: Vec<MixdownWarning>,
}

impl MixdownPlan {
    pub fn is_executable(&self) -> bool {
        !self.ffmpeg_filter_complex.is_empty()
    }

    pub fn to_json(&self) -> Result<String, AudioError> {
        serde_json::to_string(self).map_err(|_| AudioError::PlanEncode)
    }

    pub fn from_json(json: &str) -> Result<Self, AudioError> {
        serde_json::from_str(json).map_err(|_| AudioError::PlanDecode)
    }
}

pub fn plan_mixdown(
    project: &Project,
    sequence_id: &str,
    mixer: &MixerState,
) -> Result<MixdownPlan, AudioError> {
    let sequence = project
        .sequences
        .iter()
        .find(|item| item.id == sequence_id)
        .ok_or(AudioError::SequenceNotFound)?;
    if sequence.settings.sample_rate == 0 {
        return Err(AudioError::InvalidSampleRate);
    }
    let mut warnings = Vec::new();
    let (channels, layout) = output_layout(sequence, &mut warnings);
    let duration_ticks = sequence
        .audio_tracks
        .iter()
        .flat_map(|track| track.clips.iter())
        .map(|clip| clip.timeline_out)
        .max()
        .unwrap_or(0)
        .max(0);
    let duration_samples = ticks_to_samples(duration_ticks, sequence.settings.sample_rate);
    let any_solo = sequence
        .audio_tracks
        .iter()
        .any(|track| mixer.solo(&track.id));
    let mut inputs = Vec::new();
    let mut tracks = Vec::with_capacity(sequence.audio_tracks.len());

    for track in &sequence.audio_tracks {
        let solo = mixer.solo(&track.id);
        let audible = crate::is_audible(track.muted, solo, any_solo);
        let mut clips = Vec::with_capacity(track.clips.len());
        for clip in &track.clips {
            clips.push(plan_clip(
                project,
                sequence,
                track.id.as_str(),
                clip,
                audible,
                channels,
                &mut inputs,
                &mut warnings,
            ));
        }
        tracks.push(PlannedTrack {
            track_id: track.id.clone(),
            index: track.index,
            muted: track.muted,
            solo,
            audible,
            fader_db: clamp_fader(mixer.fader_db(&track.id)),
            pan: crate::clamp_pan(mixer.pan(&track.id)),
            clips,
        });
    }

    if duration_ticks == 0 {
        warnings.push(warning(
            WarningCode::EmptySequence,
            None,
            None,
            "sequence has no audio to mix",
        ));
    }

    let ffmpeg_filter_complex = build_graph(
        &tracks,
        &inputs,
        sequence.settings.sample_rate,
        channels,
        layout,
        duration_ticks,
        clamp_fader(mixer.master_gain_db),
    );

    Ok(MixdownPlan {
        sequence_id: sequence.id.clone(),
        sample_rate: sequence.settings.sample_rate,
        channels,
        channel_layout: layout.to_string(),
        duration_ticks,
        duration_samples,
        master_gain_db: clamp_fader(mixer.master_gain_db),
        inputs,
        tracks,
        ffmpeg_filter_complex,
        ffmpeg_output_label: FFMPEG_OUTPUT_LABEL.to_string(),
        warnings,
    })
}

fn plan_clip(
    project: &Project,
    sequence: &Sequence,
    track_id: &str,
    clip: &Clip,
    track_audible: bool,
    output_channels: u16,
    inputs: &mut Vec<MixdownInput>,
    warnings: &mut Vec<MixdownWarning>,
) -> PlannedClip {
    let parsed = parse_clip_audio(&clip.effects);
    if parsed.ignored_duplicate {
        warnings.push(warning(
            WarningCode::ExtraEffectIgnored,
            Some(clip.id.clone()),
            Some(track_id.to_string()),
            "extra audio effects on the clip were ignored",
        ));
    }
    let params = parsed.params;
    let (left_gain, right_gain) = balance_gains(params.pan);
    let sample_rate = sequence.settings.sample_rate;
    let mut planned = PlannedClip {
        clip_id: clip.id.clone(),
        track_id: track_id.to_string(),
        media_id: clip.media_id.clone(),
        input_index: None,
        timeline_in: clip.timeline_in,
        timeline_out: clip.timeline_out,
        source_in: clip.source_in,
        source_out: clip.source_out,
        speed: clip.speed,
        reversed: clip.reversed,
        enabled: clip.enabled,
        rendered: false,
        gain_db: params.gain_db,
        gain_keyframes: params.gain_keyframes,
        pan: params.pan,
        left_gain,
        right_gain,
        eq: params.eq,
        compressor: params.compressor,
        delay_samples: ticks_to_samples(clip.timeline_in.max(0), sample_rate),
        source_channels: output_channels,
        tempo_stages: Vec::new(),
    };
    if !track_audible || !clip.enabled {
        return planned;
    }
    let media = match classify(project, clip) {
        Ok(media) => media,
        Err(code) => {
            warnings.push(warning(
                code,
                Some(clip.id.clone()),
                Some(track_id.to_string()),
                message_for(code),
            ));
            return planned;
        }
    };
    if clip.timeline_out <= clip.timeline_in || clip.source_out <= clip.source_in {
        warnings.push(warning(
            WarningCode::EmptyClip,
            Some(clip.id.clone()),
            Some(track_id.to_string()),
            "clip has no audio duration",
        ));
        return planned;
    }
    let Some(stages) = tempo_stages(clip.speed) else {
        warnings.push(warning(
            WarningCode::InvalidSpeed,
            Some(clip.id.clone()),
            Some(track_id.to_string()),
            "clip speed cannot be rendered",
        ));
        return planned;
    };
    let index = input_index_for(inputs, media);
    planned.input_index = Some(index);
    planned.rendered = true;
    planned.tempo_stages = stages;
    planned.source_channels = media
        .audio
        .as_ref()
        .map(|audio| audio.channels.max(1))
        .unwrap_or(output_channels);
    planned
}

fn classify<'a>(project: &'a Project, clip: &Clip) -> Result<&'a MediaRef, WarningCode> {
    if clip.nested_sequence_id.is_some() {
        return Err(WarningCode::NestedNotExpanded);
    }
    match clip.kind {
        ClipKind::Nested => Err(WarningCode::NestedNotExpanded),
        ClipKind::Adjustment | ClipKind::Title | ClipKind::ColorMatte | ClipKind::Caption => {
            Err(WarningCode::UnsupportedClip)
        }
        ClipKind::Media => {
            let Some(media_id) = clip.media_id.as_deref() else {
                return Err(WarningCode::MissingMedia);
            };
            let Some(media) = project.media.iter().find(|item| item.id == media_id) else {
                return Err(WarningCode::MissingMedia);
            };
            if media.audio.is_none() && media.video.is_some() {
                Err(WarningCode::NoAudioStream)
            } else {
                Ok(media)
            }
        }
    }
}

fn message_for(code: WarningCode) -> &'static str {
    match code {
        WarningCode::MissingMedia => "clip media is missing",
        WarningCode::NoAudioStream => "media has no audio stream",
        WarningCode::NestedNotExpanded => "nested sequence audio is not expanded",
        WarningCode::UnsupportedClip => "clip kind has no audio to mix",
        WarningCode::InvalidSpeed => "clip speed cannot be rendered",
        WarningCode::EmptyClip => "clip has no audio duration",
        WarningCode::ExtraEffectIgnored => "extra audio effects on the clip were ignored",
        WarningCode::UnsupportedChannelCount => "sequence channel count is rendered as stereo",
        WarningCode::EmptySequence => "sequence has no audio to mix",
    }
}

fn output_layout(sequence: &Sequence, warnings: &mut Vec<MixdownWarning>) -> (u16, &'static str) {
    match sequence.settings.audio_channels {
        1 => (1, "mono"),
        2 => (2, "stereo"),
        _ => {
            warnings.push(warning(
                WarningCode::UnsupportedChannelCount,
                None,
                None,
                message_for(WarningCode::UnsupportedChannelCount),
            ));
            (2, "stereo")
        }
    }
}

fn input_index_for(inputs: &mut Vec<MixdownInput>, media: &MediaRef) -> u32 {
    if let Some(existing) = inputs.iter().find(|input| input.media_id == media.id) {
        return existing.index;
    }
    let index = inputs.len() as u32;
    inputs.push(MixdownInput {
        index,
        media_id: media.id.clone(),
        path: media.path.clone(),
        sample_rate: media.audio.as_ref().map(|audio| audio.sample_rate),
        channels: media.audio.as_ref().map(|audio| audio.channels),
    });
    index
}

fn build_graph(
    tracks: &[PlannedTrack],
    inputs: &[MixdownInput],
    sample_rate: u32,
    channels: u16,
    layout: &str,
    duration_ticks: i64,
    master_gain_db: f64,
) -> String {
    if duration_ticks <= 0 {
        return String::new();
    }
    let seconds = format_ffmpeg_number(ticks_to_seconds(duration_ticks));
    let rendered: Vec<&PlannedClip> = tracks
        .iter()
        .filter(|track| track.audible)
        .flat_map(|track| track.clips.iter())
        .filter(|clip| clip.rendered)
        .collect();
    if rendered.is_empty() {
        return format!(
            "anullsrc=channel_layout={layout}:sample_rate={sample_rate},atrim=end={seconds},asetpts=PTS-STARTPTS[{FFMPEG_OUTPUT_LABEL}]"
        );
    }
    let mut use_counts = vec![0usize; inputs.len()];
    for clip in &rendered {
        if let Some(index) = clip.input_index {
            use_counts[index as usize] += 1;
        }
    }
    let mut parts = Vec::new();
    for (index, count) in use_counts.iter().enumerate() {
        if *count > 1 {
            let pads: String = (0..*count)
                .map(|use_i| format!("[in{index}_{use_i}]"))
                .collect();
            parts.push(format!("[{index}:a]asplit={count}{pads}"));
        }
    }
    let mut next_use = vec![0usize; inputs.len()];
    let mut clip_labels: Vec<(usize, String)> = Vec::new();
    for clip in &rendered {
        let index = clip.input_index.unwrap_or(0) as usize;
        let use_i = next_use[index];
        next_use[index] += 1;
        let source = if use_counts[index] <= 1 {
            format!("[{index}:a]")
        } else {
            format!("[in{index}_{use_i}]")
        };
        let label = format!("clip{}", clip_labels.len());
        let filters = clip_filters(clip, sample_rate, channels, layout);
        parts.push(format!("{source}{}[{label}]", filters.join(",")));
        let track_slot = tracks
            .iter()
            .position(|track| track.track_id == clip.track_id)
            .unwrap_or(0);
        clip_labels.push((track_slot, label));
    }
    let mut bus_labels = Vec::new();
    for (track_index, track) in tracks.iter().enumerate() {
        if !track.audible {
            continue;
        }
        let labels: Vec<&str> = clip_labels
            .iter()
            .filter(|(slot, _)| *slot == track_index)
            .map(|(_, label)| label.as_str())
            .collect();
        if labels.is_empty() {
            continue;
        }
        let bus = if labels.len() == 1 {
            format!("[{}]", labels[0])
        } else {
            let mix = format!("bus{track_index}");
            let pads: String = labels.iter().map(|label| format!("[{label}]")).collect();
            parts.push(format!(
                "{pads}amix=inputs={}:duration=longest:dropout_transition=0:normalize=0[{mix}]",
                labels.len()
            ));
            format!("[{mix}]")
        };
        let track_label = format!("track{track_index}");
        let mut tail = vec![volume_filter(track.fader_db)];
        push_balance(&mut tail, track.pan, channels, layout);
        parts.push(format!("{bus}{}[{track_label}]", tail.join(",")));
        bus_labels.push(track_label);
    }
    let master = volume_filter(master_gain_db);
    let tail = format!("{master},apad=whole_dur={seconds},atrim=end={seconds},asetpts=PTS-STARTPTS[{FFMPEG_OUTPUT_LABEL}]");
    if bus_labels.len() == 1 {
        parts.push(format!("[{}]{tail}", bus_labels[0]));
    } else {
        let pads: String = bus_labels
            .iter()
            .map(|label| format!("[{label}]"))
            .collect();
        parts.push(format!(
            "{pads}amix=inputs={}:duration=longest:dropout_transition=0:normalize=0,{tail}",
            bus_labels.len()
        ));
    }
    parts.join(";")
}

fn clip_filters(clip: &PlannedClip, sample_rate: u32, channels: u16, layout: &str) -> Vec<String> {
    let mut filters = vec![format!(
        "aformat=sample_rates={sample_rate}:sample_fmts=fltp"
    )];
    push_channel_adapter(&mut filters, clip.source_channels, channels, layout);
    let start = ticks_to_samples(clip.source_in.max(0), sample_rate);
    let end = ticks_to_samples(clip.source_out.max(0), sample_rate);
    filters.push(format!("atrim=start_sample={start}:end_sample={end}"));
    filters.push("asetpts=PTS-STARTPTS".to_string());
    if clip.reversed {
        filters.push("areverse".to_string());
    }
    for stage in &clip.tempo_stages {
        filters.push(format!("atempo={}", format_ffmpeg_number(*stage)));
    }
    let timeline_samples = ticks_to_samples(
        clip.timeline_out.saturating_sub(clip.timeline_in).max(0),
        sample_rate,
    );
    filters.push(format!("atrim=end_sample={timeline_samples}"));
    filters.push("asetpts=PTS-STARTPTS".to_string());
    filters.push(gain_filter(clip));
    push_balance(&mut filters, clip.pan, channels, layout);
    if let Some(eq) = clip.eq.as_ref() {
        push_eq(&mut filters, eq);
    }
    if let Some(compressor) = clip.compressor.as_ref() {
        if !compressor.is_neutral() {
            filters.push(compand_filter(compressor));
        }
    }
    if clip.delay_samples > 0 {
        filters.push(adelay(clip.delay_samples, channels));
    }
    filters
}

fn push_channel_adapter(
    filters: &mut Vec<String>,
    source_channels: u16,
    channels: u16,
    layout: &str,
) {
    if source_channels <= 1 && channels >= 2 {
        filters.push(format!("pan={layout}|c0=c0|c1=c0"));
    } else if source_channels >= 2 && channels == 1 {
        filters.push("pan=mono|c0=0.5*c0+0.5*c1".to_string());
    } else if channels >= 2 {
        filters.push(format!("pan={layout}|c0=c0|c1=c1"));
    }
}

fn gain_filter(clip: &PlannedClip) -> String {
    let keys = sorted_gain_keys(&clip.gain_keyframes);
    if keys.len() >= 2 {
        let expression = gain_volume_expression(&keys);
        format!(
            "volume=volume='{}':eval=frame",
            escape_filter_value(&expression)
        )
    } else {
        let db = keys.first().map(|key| key.gain_db).unwrap_or(clip.gain_db);
        volume_filter(db)
    }
}

pub(crate) fn gain_volume_expression(keyframes: &[GainKeyframe]) -> String {
    let points = sorted_gain_keys(keyframes);
    if points.is_empty() {
        return "1".to_string();
    }
    if points.len() == 1 {
        return format_ffmpeg_number(db_to_linear(points[0].gain_db));
    }
    let last = format_ffmpeg_number(db_to_linear(points[points.len() - 1].gain_db));
    let mut expression = last;
    for index in (0..points.len() - 1).rev() {
        let right_time = format_ffmpeg_number(ticks_to_seconds(points[index + 1].time_ticks));
        let segment = segment_expression(&points[index], &points[index + 1]);
        expression = format!("if(lt(t,{right_time}),{segment},{expression})");
    }
    if points[0].time_ticks > 0 {
        let start = format_ffmpeg_number(ticks_to_seconds(points[0].time_ticks));
        let gain = format_ffmpeg_number(db_to_linear(points[0].gain_db));
        expression = format!("if(lt(t,{start}),{gain},{expression})");
    }
    expression
}

fn segment_expression(left: &GainKeyframe, right: &GainKeyframe) -> String {
    let start_gain = format_ffmpeg_number(db_to_linear(left.gain_db));
    let end_gain = format_ffmpeg_number(db_to_linear(right.gain_db));
    let start = format_ffmpeg_number(ticks_to_seconds(left.time_ticks));
    let end = format_ffmpeg_number(ticks_to_seconds(right.time_ticks));
    if left.time_ticks == right.time_ticks {
        return end_gain;
    }
    match left.interp {
        Interp::Hold => start_gain,
        Interp::Linear => {
            format!("{start_gain}+({end_gain}-{start_gain})*(t-{start})/({end}-{start})")
        }
        Interp::Bezier => {
            let unit = format!("((t-{start})/({end}-{start}))");
            format!(
                "{start_gain}+({end_gain}-{start_gain})*(3*{unit}*{unit}-2*{unit}*{unit}*{unit})"
            )
        }
    }
}

fn push_balance(filters: &mut Vec<String>, pan: f64, channels: u16, layout: &str) {
    if channels < 2 {
        return;
    }
    let (left, right) = balance_gains(pan);
    if (left - 1.0).abs() < 1e-6 && (right - 1.0).abs() < 1e-6 {
        return;
    }
    filters.push(format!(
        "pan={layout}|c0={}*c0|c1={}*c1",
        format_ffmpeg_number(left),
        format_ffmpeg_number(right)
    ));
}

fn push_eq(filters: &mut Vec<String>, eq: &EqSettings) {
    if eq.low_gain_db.abs() >= 1e-6 {
        filters.push(format!(
            "lowshelf=f={}:g={}",
            format_ffmpeg_number(eq.low_freq_hz),
            format_ffmpeg_number(eq.low_gain_db)
        ));
    }
    if eq.mid_gain_db.abs() >= 1e-6 {
        filters.push(format!(
            "equalizer=f={}:t=q:w={}:g={}",
            format_ffmpeg_number(eq.mid_freq_hz),
            format_ffmpeg_number(eq.mid_q),
            format_ffmpeg_number(eq.mid_gain_db)
        ));
    }
    if eq.high_gain_db.abs() >= 1e-6 {
        filters.push(format!(
            "highshelf=f={}:g={}",
            format_ffmpeg_number(eq.high_freq_hz),
            format_ffmpeg_number(eq.high_gain_db)
        ));
    }
}

fn compand_filter(settings: &CompressorSettings) -> String {
    let mut levels = vec![-96.0, -72.0, -48.0, -36.0, -24.0, -12.0, 0.0, 12.0];
    levels.push(settings.threshold_db);
    levels.sort_by(f64::total_cmp);
    levels.dedup_by(|left, right| (*left - *right).abs() < 1e-3);
    let points = levels
        .into_iter()
        .map(|input| {
            let output = crate::compressor_output_db(
                input,
                settings.threshold_db,
                settings.ratio,
                settings.knee_db,
            );
            format!(
                "{}/{}",
                format_ffmpeg_number(input),
                format_ffmpeg_number(output)
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    let knee = settings.knee_db.max(0.01);
    format!(
        "compand=attacks={}:decays={}:points={points}:soft-knee={}:gain={}",
        format_ffmpeg_number(settings.attack_ms / 1000.0),
        format_ffmpeg_number(settings.release_ms / 1000.0),
        format_ffmpeg_number(knee),
        format_ffmpeg_number(settings.makeup_db)
    )
}

fn adelay(samples: i64, channels: u16) -> String {
    let count = channels.max(1);
    let delay = format!("{samples}S");
    let delays = std::iter::repeat(delay.as_str())
        .take(count as usize)
        .collect::<Vec<_>>()
        .join("|");
    format!("adelay={delays}")
}

fn volume_filter(gain_db: f64) -> String {
    format!("volume={}", format_ffmpeg_number(db_to_linear(gain_db)))
}

fn escape_filter_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace(',', "\\,")
        .replace('\'', "\\'")
}

fn tempo_stages(speed: f64) -> Option<Vec<f64>> {
    if !speed.is_finite() || speed <= 0.0 {
        return None;
    }
    if (speed - 1.0).abs() < 1e-4 {
        return Some(Vec::new());
    }
    let mut stages = Vec::new();
    let mut remaining = speed;
    for _ in 0..8 {
        if (remaining - 1.0).abs() < 1e-3 {
            return Some(stages);
        }
        if remaining > 2.0 {
            stages.push(2.0);
            remaining /= 2.0;
        } else if remaining < 0.5 {
            stages.push(0.5);
            remaining /= 0.5;
        } else {
            stages.push(remaining);
            return Some(stages);
        }
    }
    if (remaining - 1.0).abs() < 1e-2 {
        Some(stages)
    } else {
        None
    }
}

fn clamp_fader(db: f64) -> f64 {
    if db.is_finite() {
        db.clamp(FADER_MIN_DB, FADER_MAX_DB)
    } else {
        0.0
    }
}

fn warning(
    code: WarningCode,
    clip_id: Option<String>,
    track_id: Option<String>,
    message: &str,
) -> MixdownWarning {
    MixdownWarning {
        code,
        clip_id,
        track_id,
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::{compressor_effect, eq_effect, gain_effect, pan_effect};
    use crate::eq::EqSettings;
    use crate::mixer::{set_track_mute, MixerState};
    use crate::units::format_ffmpeg_number;
    use crate::{compressor_output_db, db_to_linear, AudioError, CompressorSettings, GainKeyframe};
    use videocut_model::{
        AudioStreamInfo, Clip, ClipKind, Interp, MediaRef, Project, Sequence, SequenceSettings,
        Track, TrackKind, VideoStreamInfo, TICKS_PER_SECOND,
    };

    fn media(id: &str, channels: u16) -> MediaRef {
        MediaRef {
            id: id.to_string(),
            name: format!("{id}.wav"),
            path: format!("C:/media/{id}.wav"),
            duration_ticks: TICKS_PER_SECOND * 10,
            video: None,
            audio: Some(AudioStreamInfo {
                sample_rate: 48_000,
                channels,
                codec: "pcm".into(),
            }),
            proxy_path: None,
        }
    }

    fn place(clip: &mut Clip, start: i64, end: i64) {
        clip.timeline_in = start;
        clip.timeline_out = end;
        clip.source_in = 0;
        clip.source_out = end - start;
    }

    #[test]
    fn mixdown_encodes_gain_pan_eq_compressor_and_roundtrips() {
        let mut project = Project::new("demo");
        project.media.push(media("media-1", 2));
        let mut sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
        let mut track = Track::new(0, TrackKind::Audio);
        let mut clip = Clip::media("dialogue", "media-1");
        place(&mut clip, 0, TICKS_PER_SECOND);
        let gain = gain_effect(
            0.0,
            &[
                GainKeyframe {
                    time_ticks: 0,
                    gain_db: 0.0,
                    interp: Interp::Linear,
                },
                GainKeyframe {
                    time_ticks: TICKS_PER_SECOND,
                    gain_db: -6.0,
                    interp: Interp::Hold,
                },
            ],
        )
        .unwrap();
        let pan = pan_effect(-1.0).unwrap();
        let mut eq = EqSettings::default();
        eq.low_gain_db = 3.0;
        eq.mid_gain_db = 2.0;
        eq.high_gain_db = -1.5;
        let eq = eq_effect(eq).unwrap();
        let compressor = compressor_effect(CompressorSettings {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            knee_db: 0.0,
            makeup_db: 0.0,
        })
        .unwrap();
        clip.effects = vec![gain, pan, eq, compressor];
        track.clips.push(clip);
        sequence.audio_tracks.push(track);
        let sequence_id = sequence.id.clone();
        project.sequences.push(sequence);

        let plan = plan_mixdown(&project, &sequence_id, &MixerState::default()).unwrap();
        let graph = &plan.ffmpeg_filter_complex;
        assert!(plan.is_executable());
        assert!(graph.contains("eval=frame"));
        assert!(graph.contains(r"lt(t\,"));
        assert!(graph.contains("pan=stereo|c0=1*c0|c1=0*c1"));
        assert!(graph.contains("lowshelf=f=100:g=3"));
        assert!(graph.contains("equalizer=f=1000:t=q:w=1:g=2"));
        assert!(graph.contains("highshelf=f=8000:g=-1.5"));
        assert!(graph.contains("compand="));
        assert!(graph.contains("0/-15"));
        assert!(graph.contains("soft-knee=0.01"));
        assert!(graph.contains("[aout]"));
        assert_eq!(plan.inputs.len(), 1);
        assert_eq!(plan.inputs[0].path, "C:/media/media-1.wav");
        let output = compressor_output_db(0.0, -20.0, 4.0, 0.0);
        assert!((output + 15.0).abs() < 1e-9);

        let json = plan.to_json().unwrap();
        assert!(json.contains("\"ffmpegFilterComplex\""));
        assert!(json.contains("\"sequenceId\""));
        let restored = MixdownPlan::from_json(&json).unwrap();
        assert_eq!(restored, plan);
        assert_eq!(MixdownPlan::from_json("{"), Err(AudioError::PlanDecode));
    }

    #[test]
    fn mute_solo_speed_reverse_and_shared_media() {
        let mut project = Project::new("demo");
        project.media.push(media("media-1", 1));
        let mut sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
        let mut audible = Track::new(0, TrackKind::Audio);
        let mut muted = Track::new(1, TrackKind::Audio);
        let audible_id = audible.id.clone();
        let muted_id = muted.id.clone();

        let mut first = Clip::media("a", "media-1");
        place(&mut first, 0, TICKS_PER_SECOND);
        first.speed = 4.0;
        first.reversed = true;
        first.effects = vec![gain_effect(0.0, &[]).unwrap()];
        let mut second = Clip::media("b", "media-1");
        place(&mut second, TICKS_PER_SECOND, TICKS_PER_SECOND * 2);
        audible.clips.push(first);
        audible.clips.push(second);

        let mut quiet = Clip::media("quiet", "media-1");
        place(&mut quiet, 0, TICKS_PER_SECOND);
        quiet.effects = vec![gain_effect(-12.0, &[]).unwrap()];
        muted.clips.push(quiet);
        muted.muted = true;

        sequence.audio_tracks.push(audible);
        sequence.audio_tracks.push(muted);
        let sequence_id = sequence.id.clone();
        project.sequences.push(sequence);

        let mut mixer = MixerState::default();
        mixer.set_fader_db(&audible_id, 3.0).unwrap();
        mixer.master_gain_db = -6.0;
        let plan = plan_mixdown(&project, &sequence_id, &mixer).unwrap();
        let graph = &plan.ffmpeg_filter_complex;
        assert!(graph.contains("atempo=2,atempo=2"));
        assert!(graph.contains("areverse"));
        assert!(graph.contains("asplit=2"));
        assert!(graph.contains("amix=inputs=2:duration=longest:dropout_transition=0:normalize=0"));
        assert!(graph.contains("pan=stereo|c0=c0|c1=c0"));
        let quiet = format_ffmpeg_number(db_to_linear(-12.0));
        assert!(!graph.contains(&quiet));
        assert!(
            !plan
                .tracks
                .iter()
                .find(|track| track.track_id == muted_id)
                .unwrap()
                .clips[0]
                .rendered
        );
        assert!(graph.contains(&format!(
            "volume={}",
            format_ffmpeg_number(db_to_linear(3.0))
        )));
        assert!(graph.contains(&format!(
            "volume={}",
            format_ffmpeg_number(db_to_linear(-6.0))
        )));
        assert_eq!(plan.inputs.len(), 1);
        assert_eq!(plan.duration_ticks, TICKS_PER_SECOND * 2);
    }

    #[test]
    fn solo_silence_and_skipped_clips_warn() {
        let mut project = Project::new("demo");
        project.media.push(media("voice", 2));
        project.media.push(MediaRef {
            id: "picture".into(),
            name: "picture.png".into(),
            path: "C:/media/picture.png".into(),
            duration_ticks: TICKS_PER_SECOND,
            video: Some(VideoStreamInfo {
                width: 1920,
                height: 1080,
                fps: videocut_model::Rational { num: 30, den: 1 },
                codec: "png".into(),
            }),
            audio: None,
            proxy_path: None,
        });
        let mut sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
        let mut voice = Track::new(0, TrackKind::Audio);
        let mut other = Track::new(1, TrackKind::Audio);
        let voice_id = voice.id.clone();
        let mut voiced = Clip::media("voice", "voice");
        place(&mut voiced, 0, TICKS_PER_SECOND);
        voice.clips.push(voiced);
        let mut picture = Clip::media("picture", "picture");
        place(&mut picture, 0, TICKS_PER_SECOND);
        let mut missing = Clip::media("missing", "gone");
        place(&mut missing, 0, TICKS_PER_SECOND);
        let mut nested = Clip::media("nested", "voice");
        nested.kind = ClipKind::Nested;
        nested.nested_sequence_id = Some("seq-child".into());
        place(&mut nested, 0, TICKS_PER_SECOND);
        let mut fast = Clip::media("fast", "voice");
        place(&mut fast, 0, TICKS_PER_SECOND);
        fast.speed = 0.0;
        other.clips.extend([picture, missing, nested, fast]);
        sequence.audio_tracks.push(voice);
        sequence.audio_tracks.push(other);
        let sequence_id = sequence.id.clone();
        project.sequences.push(sequence);

        let mut mixer = MixerState::default();
        mixer.set_solo(&voice_id, true);
        let plan = plan_mixdown(&project, &sequence_id, &mixer).unwrap();
        assert!(plan.tracks[0].audible);
        assert!(!plan.tracks[1].audible);
        assert!(plan.tracks[1].clips.iter().all(|clip| !clip.rendered));
        assert_eq!(plan.inputs.len(), 1);
        assert!(!plan
            .warnings
            .iter()
            .any(|item| item.code == WarningCode::NoAudioStream));

        let mut open = MixerState::default();
        let audible = plan_mixdown(&project, &sequence_id, &open).unwrap();
        let codes: Vec<_> = audible.warnings.iter().map(|item| item.code).collect();
        assert!(codes.contains(&WarningCode::NoAudioStream));
        assert!(codes.contains(&WarningCode::MissingMedia));
        assert!(codes.contains(&WarningCode::NestedNotExpanded));
        assert!(codes.contains(&WarningCode::InvalidSpeed));

        let muted = project.sequences[0].audio_tracks[0].id.clone();
        set_track_mute(project.sequence_mut(&sequence_id).unwrap(), &muted, true).unwrap();
        open.set_solo(&voice_id, false);
        let silent = plan_mixdown(&project, &sequence_id, &open).unwrap();
        assert!(silent.ffmpeg_filter_complex.starts_with("anullsrc="));
        assert!(silent.inputs.is_empty());

        let empty = Project::new("empty");
        let sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
        let sequence_id = sequence.id.clone();
        let mut empty = empty;
        empty.sequences.push(sequence);
        let none = plan_mixdown(&empty, &sequence_id, &MixerState::default()).unwrap();
        assert!(!none.is_executable());
        assert_eq!(none.warnings[0].code, WarningCode::EmptySequence);
        assert!(plan_mixdown(&empty, "missing", &MixerState::default()).is_err());
    }

    #[test]
    fn hold_expression_does_not_ramp() {
        let expression = gain_volume_expression(&[
            GainKeyframe {
                time_ticks: 0,
                gain_db: 0.0,
                interp: Interp::Hold,
            },
            GainKeyframe {
                time_ticks: TICKS_PER_SECOND,
                gain_db: -6.0,
                interp: Interp::Linear,
            },
        ]);
        assert!(!expression.contains("*(t-"));
        assert!(expression.contains("if(lt(t,"));
    }
}
