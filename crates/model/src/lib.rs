//! Editorial model for VideoCut.
//!
//! Time is stored in ticks. One second is [`TICKS_PER_SECOND`], the same rate
//! used by Premiere project XML, so interchange does not rescale clip edges.
//! Field names serialize as camelCase and must match `packages/model`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const TICKS_PER_SECOND: i64 = 254_016_000_000;

pub fn new_id(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rational {
    pub num: i32,
    pub den: i32,
}

impl Rational {
    pub fn fps(num: i32, den: i32) -> Result<Self, ModelError> {
        if num <= 0 || den <= 0 {
            return Err(ModelError::InvalidFrameRate);
        }
        Ok(Self { num, den })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub bins: Vec<BinNode>,
    pub sequences: Vec<Sequence>,
    pub media: Vec<MediaRef>,
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: new_id("proj"),
            name: name.into(),
            bins: Vec::new(),
            sequences: Vec::new(),
            media: Vec::new(),
        }
    }

    pub fn sequence_mut(&mut self, id: &str) -> Option<&mut Sequence> {
        self.sequences.iter_mut().find(|sequence| sequence.id == id)
    }

    pub fn media(&self, id: &str) -> Option<&MediaRef> {
        self.media.iter().find(|media| media.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinNode {
    pub id: String,
    pub name: String,
    pub kind: BinItemKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<BinNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BinItemKind {
    Bin,
    Clip,
    Sequence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaRef {
    pub id: String,
    pub name: String,
    pub path: String,
    pub duration_ticks: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<VideoStreamInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioStreamInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoStreamInfo {
    pub width: u32,
    pub height: u32,
    pub fps: Rational,
    pub codec: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStreamInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub codec: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sequence {
    pub id: String,
    pub name: String,
    pub settings: SequenceSettings,
    pub video_tracks: Vec<Track>,
    pub audio_tracks: Vec<Track>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<Marker>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub captions: Vec<Caption>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub multicam_angles: Vec<MulticamAngle>,
}

impl Sequence {
    pub fn new(name: impl Into<String>, settings: SequenceSettings) -> Self {
        Self {
            id: new_id("seq"),
            name: name.into(),
            settings,
            video_tracks: Vec::new(),
            audio_tracks: Vec::new(),
            markers: Vec::new(),
            captions: Vec::new(),
            multicam_angles: Vec::new(),
        }
    }

    pub fn track_mut(&mut self, id: &str) -> Option<&mut Track> {
        self.video_tracks
            .iter_mut()
            .chain(self.audio_tracks.iter_mut())
            .find(|track| track.id == id)
    }

    pub fn clip_mut(&mut self, id: &str) -> Option<&mut Clip> {
        self.video_tracks
            .iter_mut()
            .chain(self.audio_tracks.iter_mut())
            .find_map(|track| track.clips.iter_mut().find(|clip| clip.id == id))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequenceSettings {
    pub width: u32,
    pub height: u32,
    pub fps: Rational,
    pub sample_rate: u32,
    pub audio_channels: u16,
    pub start_timecode_ticks: i64,
}

impl SequenceSettings {
    pub fn hd1080p30() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: Rational { num: 30, den: 1 },
            sample_rate: 48_000,
            audio_channels: 2,
            start_timecode_ticks: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub index: u32,
    pub kind: TrackKind,
    pub clips: Vec<Clip>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<Transition>,
    pub locked: bool,
    pub muted: bool,
    pub hidden: bool,
    pub sync_lock: bool,
    pub targeted: bool,
}

impl Track {
    pub fn new(index: u32, kind: TrackKind) -> Self {
        Self {
            id: new_id("track"),
            index,
            kind,
            clips: Vec::new(),
            transitions: Vec::new(),
            locked: false,
            muted: false,
            hidden: false,
            sync_lock: true,
            targeted: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nested_sequence_id: Option<String>,
    pub kind: ClipKind,
    pub timeline_in: i64,
    pub timeline_out: i64,
    pub source_in: i64,
    pub source_out: i64,
    pub speed: f64,
    pub reversed: bool,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<EffectInstance>,
    pub label_color: u8,
}

impl Clip {
    pub fn media(name: impl Into<String>, media_id: impl Into<String>) -> Self {
        Self {
            id: new_id("clip"),
            name: name.into(),
            media_id: Some(media_id.into()),
            nested_sequence_id: None,
            kind: ClipKind::Media,
            timeline_in: 0,
            timeline_out: 0,
            source_in: 0,
            source_out: 0,
            speed: 1.0,
            reversed: false,
            enabled: true,
            linked_ids: Vec::new(),
            effects: Vec::new(),
            label_color: 0,
        }
    }

    pub fn duration_ticks(&self) -> i64 {
        self.timeline_out.saturating_sub(self.timeline_in)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipKind {
    Media,
    Nested,
    Adjustment,
    Title,
    ColorMatte,
    Caption,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectInstance {
    pub id: String,
    pub effect_id: String,
    pub enabled: bool,
    pub params: Vec<Param>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Param {
    pub id: String,
    pub value: ParamValue,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keyframes: Vec<Keyframe>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum ParamValue {
    Float(f64),
    Int(i64),
    Bool(bool),
    Color(Rgba),
    Point(Point2),
    Str(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Keyframe {
    pub time_ticks: i64,
    pub value: ParamValue,
    pub interp: Interp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Interp {
    Linear,
    Bezier,
    Hold,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transition {
    pub id: String,
    pub effect_id: String,
    pub center_ticks: i64,
    pub duration_ticks: i64,
    pub alignment: TransitionAlign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransitionAlign {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    pub id: String,
    pub time_ticks: i64,
    pub duration_ticks: i64,
    pub name: String,
    pub comment: String,
    pub color: u8,
    pub kind: MarkerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MarkerKind {
    Comment,
    Chapter,
    Segmentation,
    Caption,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Caption {
    pub id: String,
    pub start_ticks: i64,
    pub end_ticks: i64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MulticamAngle {
    pub id: String,
    pub name: String,
    pub media_id: String,
    pub sync_ticks: i64,
}

/// Builtin effect identifiers. These are VideoCut ids, not host-application ids.
pub mod effect_ids {
    pub const MOTION: &str = "builtin.motion";
    pub const OPACITY: &str = "builtin.opacity";
    pub const CROP: &str = "builtin.crop";
    pub const TRANSFORM: &str = "builtin.transform";
    pub const CROSS_DISSOLVE: &str = "builtin.cross-dissolve";
    pub const DIP_TO_BLACK: &str = "builtin.dip-to-black";
    pub const DIP_TO_WHITE: &str = "builtin.dip-to-white";
    pub const WIPE: &str = "builtin.wipe";
    pub const GAIN: &str = "builtin.gain";
    pub const PAN: &str = "builtin.pan";
    pub const EQ: &str = "builtin.eq";
    pub const COMPRESSOR: &str = "builtin.compressor";
    pub const COLOR_WHEELS: &str = "builtin.color-wheels";
    pub const CURVES: &str = "builtin.curves";
    pub const HSL: &str = "builtin.hsl";
    pub const LUT: &str = "builtin.lut";
}

pub fn ticks_from_frames(frames: i64, fps: Rational) -> Result<i64, ModelError> {
    if fps.num <= 0 || fps.den <= 0 {
        return Err(ModelError::InvalidFrameRate);
    }
    let ticks = (frames as i128)
        .saturating_mul(TICKS_PER_SECOND as i128)
        .saturating_mul(fps.den as i128)
        / (fps.num as i128);
    Ok(i64::try_from(ticks).unwrap_or(i64::MAX))
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    #[error("frame rate must be a positive rational")]
    InvalidFrameRate,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_json_roundtrip() {
        let mut project = Project::new("demo");
        let media_id = new_id("media");
        project.media.push(MediaRef {
            id: media_id.clone(),
            name: "clip.mp4".into(),
            path: "C:/media/clip.mp4".into(),
            duration_ticks: TICKS_PER_SECOND,
            video: Some(VideoStreamInfo {
                width: 1920,
                height: 1080,
                fps: Rational { num: 30, den: 1 },
                codec: "h264".into(),
            }),
            audio: None,
            proxy_path: None,
        });
        let mut sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
        let mut track = Track::new(0, TrackKind::Video);
        let mut clip = Clip::media("clip.mp4", media_id);
        clip.timeline_out = TICKS_PER_SECOND;
        clip.source_out = TICKS_PER_SECOND;
        track.clips.push(clip);
        sequence.video_tracks.push(track);
        project.sequences.push(sequence);

        let json = serde_json::to_string(&project).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(project, back);
    }

    #[test]
    fn one_second_of_30fps_is_thirty_frames_of_ticks() {
        let ticks = ticks_from_frames(30, Rational { num: 30, den: 1 }).unwrap();
        assert_eq!(ticks, TICKS_PER_SECOND);
    }
}
