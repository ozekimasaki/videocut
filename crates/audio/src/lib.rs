//! Clip audio for VideoCut.
//!
//! Gain keyframes are clip-local ticks measured from `timeline_in`.
//! Track mute lives on the sequence. Solo, track fader, and track pan live on
//! [`MixerState`]. [`plan_mixdown`] turns that into a structured plan and an
//! FFmpeg `-filter_complex` graph a media exporter can run.

mod compressor;
mod curve;
mod effects;
mod eq;
mod meter;
mod mixdown;
mod mixer;
mod pan;
mod process;
mod units;

pub use compressor::{compressor_output_db, CompressorSettings};
pub use curve::{eval_gain_db, GainKeyframe};
pub use effects::{
    clip_audio_params, compressor_effect, eq_effect, gain_effect, pan_effect, parse_clip_audio,
    ClipAudioParams, ParsedClipAudio,
};
pub use eq::{apply_eq, EqSettings};
pub use meter::{measure, ChannelMeter};
pub use mixdown::{
    plan_mixdown, MixdownInput, MixdownPlan, MixdownWarning, PlannedClip, PlannedTrack,
    WarningCode, FFMPEG_OUTPUT_LABEL,
};
pub use mixer::{
    is_audible, mix_tracks, set_track_mute, MixedAudio, MixerState, TrackBuffer, TrackMeter,
    TrackStrip,
};
pub use pan::{apply_balance, balance_gains};
pub use process::process_clip_audio;
pub use units::{
    clamp_fader_db, clamp_gain_db, clamp_pan, db_to_linear, linear_to_db, FADER_MAX_DB,
    FADER_MIN_DB, GAIN_MAX_DB, GAIN_MIN_DB, PAN_MAX, PAN_MIN, SILENCE_DB,
};

pub mod param_ids {
    pub const GAIN: &str = "gain";
    pub const PAN: &str = "pan";
    pub const LOW_FREQ: &str = "lowFreq";
    pub const LOW_GAIN: &str = "lowGain";
    pub const MID_FREQ: &str = "midFreq";
    pub const MID_GAIN: &str = "midGain";
    pub const MID_Q: &str = "midQ";
    pub const HIGH_FREQ: &str = "highFreq";
    pub const HIGH_GAIN: &str = "highGain";
    pub const THRESHOLD: &str = "threshold";
    pub const RATIO: &str = "ratio";
    pub const ATTACK: &str = "attack";
    pub const RELEASE: &str = "release";
    pub const KNEE: &str = "knee";
    pub const MAKEUP: &str = "makeup";
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AudioError {
    #[error("sequence was not found")]
    SequenceNotFound,
    #[error("audio track was not found")]
    TrackNotFound,
    #[error("track is not an audio track")]
    NotAudioTrack,
    #[error("sample rate must be positive")]
    InvalidSampleRate,
    #[error("channel count must be positive")]
    InvalidChannels,
    #[error("audio buffer length is not divisible by the channel count")]
    BufferLength,
    #[error("parameter is out of range")]
    InvalidParameter,
    #[error("mixdown plan could not be encoded")]
    PlanEncode,
    #[error("mixdown plan could not be decoded")]
    PlanDecode,
}
