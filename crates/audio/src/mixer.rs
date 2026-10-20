use serde::{Deserialize, Serialize};
use videocut_model::{Sequence, TrackKind};

use crate::meter::{measure, ChannelMeter};
use crate::pan::balance_gains;
use crate::units::{clamp_fader_db, db_to_linear, FADER_MAX_DB, FADER_MIN_DB, PAN_MAX, PAN_MIN};
use crate::AudioError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackStrip {
    pub track_id: String,
    #[serde(default)]
    pub solo: bool,
    #[serde(default)]
    pub fader_db: f64,
    #[serde(default)]
    pub pan: f64,
}

impl TrackStrip {
    pub fn new(track_id: impl Into<String>) -> Self {
        Self {
            track_id: track_id.into(),
            solo: false,
            fader_db: 0.0,
            pan: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MixerState {
    #[serde(default)]
    pub master_gain_db: f64,
    #[serde(default)]
    pub strips: Vec<TrackStrip>,
}

impl MixerState {
    pub fn strip_mut(&mut self, track_id: &str) -> &mut TrackStrip {
        let index = match self
            .strips
            .iter()
            .position(|strip| strip.track_id == track_id)
        {
            Some(index) => index,
            None => {
                self.strips.push(TrackStrip::new(track_id));
                self.strips.len() - 1
            }
        };
        &mut self.strips[index]
    }

    pub fn solo(&self, track_id: &str) -> bool {
        self.strips
            .iter()
            .find(|strip| strip.track_id == track_id)
            .is_some_and(|strip| strip.solo)
    }

    pub fn set_solo(&mut self, track_id: &str, solo: bool) {
        self.strip_mut(track_id).solo = solo;
    }

    pub fn set_fader_db(&mut self, track_id: &str, fader_db: f64) -> Result<(), AudioError> {
        if !fader_db.is_finite() || !(FADER_MIN_DB..=FADER_MAX_DB).contains(&fader_db) {
            return Err(AudioError::InvalidParameter);
        }
        self.strip_mut(track_id).fader_db = fader_db;
        Ok(())
    }

    pub fn set_pan(&mut self, track_id: &str, pan: f64) -> Result<(), AudioError> {
        if !pan.is_finite() || !(PAN_MIN..=PAN_MAX).contains(&pan) {
            return Err(AudioError::InvalidParameter);
        }
        self.strip_mut(track_id).pan = pan;
        Ok(())
    }

    pub fn fader_db(&self, track_id: &str) -> f64 {
        self.strips
            .iter()
            .find(|strip| strip.track_id == track_id)
            .map(|strip| strip.fader_db)
            .unwrap_or(0.0)
    }

    pub fn pan(&self, track_id: &str) -> f64 {
        self.strips
            .iter()
            .find(|strip| strip.track_id == track_id)
            .map(|strip| strip.pan)
            .unwrap_or(0.0)
    }
}

pub fn is_audible(muted: bool, solo: bool, any_solo: bool) -> bool {
    !muted && (!any_solo || solo)
}

pub fn set_track_mute(
    sequence: &mut Sequence,
    track_id: &str,
    muted: bool,
) -> Result<(), AudioError> {
    let track = sequence
        .track_mut(track_id)
        .ok_or(AudioError::TrackNotFound)?;
    if track.kind != TrackKind::Audio {
        return Err(AudioError::NotAudioTrack);
    }
    track.muted = muted;
    Ok(())
}

pub struct TrackBuffer<'a> {
    pub track_id: &'a str,
    pub channels: u16,
    pub interleaved: &'a [f32],
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackMeter {
    pub track_id: String,
    pub audible: bool,
    pub channels: Vec<ChannelMeter>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MixedAudio {
    pub channels: u16,
    pub samples: Vec<f32>,
    pub track_meters: Vec<TrackMeter>,
    pub master_meters: Vec<ChannelMeter>,
}

pub fn mix_tracks(
    sequence: &Sequence,
    mixer: &MixerState,
    tracks: &[TrackBuffer<'_>],
) -> Result<MixedAudio, AudioError> {
    let out_channels = sequence.settings.audio_channels as usize;
    if out_channels == 0 {
        return Err(AudioError::InvalidChannels);
    }
    for buffer in tracks {
        let channels = buffer.channels as usize;
        if channels == 0 {
            return Err(AudioError::InvalidChannels);
        }
        if buffer.interleaved.len() % channels != 0 {
            return Err(AudioError::BufferLength);
        }
    }
    let frames = tracks
        .iter()
        .map(|buffer| buffer.interleaved.len() / buffer.channels as usize)
        .max()
        .unwrap_or(0);
    let any_solo = sequence
        .audio_tracks
        .iter()
        .any(|track| mixer.solo(&track.id));
    let master_gain = db_to_linear(clamp_fader_db(mixer.master_gain_db)) as f32;
    let mut mixed = vec![0.0f32; frames * out_channels];
    let mut track_meters = Vec::with_capacity(sequence.audio_tracks.len());
    for track in &sequence.audio_tracks {
        let audible = is_audible(track.muted, mixer.solo(&track.id), any_solo);
        let mut rendered = vec![0.0f32; frames * out_channels];
        if audible {
            if let Some(buffer) = tracks.iter().find(|item| item.track_id == track.id) {
                render_strip(
                    &mut rendered,
                    buffer,
                    mixer.fader_db(&track.id),
                    mixer.pan(&track.id),
                    out_channels,
                );
            }
        }
        for (dst, src) in mixed.iter_mut().zip(&rendered) {
            *dst += src;
        }
        track_meters.push(TrackMeter {
            track_id: track.id.clone(),
            audible,
            channels: measure(&rendered, out_channels)?,
        });
    }
    for sample in &mut mixed {
        *sample *= master_gain;
    }
    Ok(MixedAudio {
        channels: sequence.settings.audio_channels,
        samples: mixed.clone(),
        track_meters,
        master_meters: measure(&mixed, out_channels)?,
    })
}

fn render_strip(
    dst: &mut [f32],
    src: &TrackBuffer<'_>,
    fader_db: f64,
    pan: f64,
    out_channels: usize,
) {
    let in_channels = src.channels as usize;
    let src_frames = src.interleaved.len() / in_channels;
    let frames = dst.len() / out_channels;
    let (left_gain, right_gain) = balance_gains(pan);
    let fader = db_to_linear(clamp_fader_db(fader_db)) as f32;
    let left_gain = left_gain as f32 * fader;
    let right_gain = right_gain as f32 * fader;
    for frame in 0..frames.min(src_frames) {
        let left = src.interleaved[frame * in_channels] * left_gain;
        let right = if in_channels >= 2 {
            src.interleaved[frame * in_channels + 1] * right_gain
        } else {
            src.interleaved[frame * in_channels] * right_gain
        };
        if out_channels == 1 {
            dst[frame] = 0.5 * (left + right);
        } else {
            let base = frame * out_channels;
            dst[base] = left;
            dst[base + 1] = right;
            for channel in 2..out_channels.min(in_channels) {
                dst[base + channel] = src.interleaved[frame * in_channels + channel] * fader;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use videocut_model::{SequenceSettings, Track};

    fn two_tracks() -> (Sequence, String, String) {
        let mut sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
        let left = Track::new(0, TrackKind::Audio);
        let right = Track::new(1, TrackKind::Audio);
        let left_id = left.id.clone();
        let right_id = right.id.clone();
        sequence.audio_tracks.push(left);
        sequence.audio_tracks.push(right);
        (sequence, left_id, right_id)
    }

    #[test]
    fn mute_solo_fader_and_pan_sum_the_master() {
        let (mut sequence, left_id, right_id) = two_tracks();
        let left = [0.5f32, 0.25];
        let right = [0.25f32, 0.25];
        let buffers = [
            TrackBuffer {
                track_id: &left_id,
                channels: 2,
                interleaved: &left,
            },
            TrackBuffer {
                track_id: &right_id,
                channels: 2,
                interleaved: &right,
            },
        ];
        let mixed = mix_tracks(&sequence, &MixerState::default(), &buffers).unwrap();
        assert_eq!(mixed.samples, vec![0.75, 0.5]);

        set_track_mute(&mut sequence, &left_id, true).unwrap();
        let muted = mix_tracks(&sequence, &MixerState::default(), &buffers).unwrap();
        assert_eq!(muted.samples, vec![0.25, 0.25]);
        assert!(!muted.track_meters[0].audible);
        assert_eq!(muted.track_meters[0].channels[0].peak, 0.0);

        set_track_mute(&mut sequence, &left_id, false).unwrap();
        let mut solo = MixerState::default();
        solo.set_solo(&left_id, true);
        let soloed = mix_tracks(&sequence, &solo, &buffers).unwrap();
        assert_eq!(soloed.samples, vec![0.5, 0.25]);

        set_track_mute(&mut sequence, &left_id, true).unwrap();
        let silent = mix_tracks(&sequence, &solo, &buffers).unwrap();
        assert_eq!(silent.samples, vec![0.0, 0.0]);

        set_track_mute(&mut sequence, &left_id, false).unwrap();
        let mut panned = MixerState::default();
        panned.set_pan(&left_id, -1.0).unwrap();
        let hard_left = mix_tracks(&sequence, &panned, &buffers).unwrap();
        assert_eq!(hard_left.samples, vec![0.75, 0.25]);
    }

    #[test]
    fn video_tracks_cannot_be_muted_by_the_mixer() {
        let mut sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
        let video = Track::new(0, TrackKind::Video);
        let id = video.id.clone();
        sequence.video_tracks.push(video);
        assert_eq!(
            set_track_mute(&mut sequence, &id, true),
            Err(AudioError::NotAudioTrack)
        );
        assert_eq!(
            set_track_mute(&mut sequence, "missing", true),
            Err(AudioError::TrackNotFound)
        );
    }

    #[test]
    fn solo_rules() {
        assert!(is_audible(false, false, false));
        assert!(!is_audible(true, true, true));
        assert!(!is_audible(false, false, true));
        assert!(is_audible(false, true, true));
    }
}
