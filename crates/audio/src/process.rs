use videocut_model::EffectInstance;

use crate::compressor::apply_compressor;
use crate::curve::{eval_sorted_gain_db, sorted_gain_keys};
use crate::effects::clip_audio_params;
use crate::eq::apply_eq;
use crate::pan::apply_balance;
use crate::units::{db_to_linear, tick_at_sample};
use crate::AudioError;

pub fn process_clip_audio(
    input: &[f32],
    input_channels: u16,
    output_channels: u16,
    sample_rate: u32,
    start_tick: i64,
    effects: &[EffectInstance],
) -> Result<Vec<f32>, AudioError> {
    let in_channels = input_channels as usize;
    let out_channels = output_channels as usize;
    if sample_rate == 0 {
        return Err(AudioError::InvalidSampleRate);
    }
    if in_channels == 0 || out_channels == 0 {
        return Err(AudioError::InvalidChannels);
    }
    if input.len() % in_channels != 0 {
        return Err(AudioError::BufferLength);
    }
    let params = clip_audio_params(effects);
    let mut samples = fit_channels(input, in_channels, out_channels);
    let frames = samples.len() / out_channels;
    let keys = sorted_gain_keys(&params.gain_keyframes);
    for frame in 0..frames {
        let tick = tick_at_sample(start_tick, frame, sample_rate);
        let gain = db_to_linear(eval_sorted_gain_db(params.gain_db, &keys, tick)) as f32;
        for channel in 0..out_channels {
            samples[frame * out_channels + channel] *= gain;
        }
    }
    if out_channels >= 2 {
        apply_balance(&mut samples, out_channels, params.pan);
    }
    if let Some(eq) = params.eq.as_ref() {
        apply_eq(&mut samples, out_channels, sample_rate, eq)?;
    }
    if let Some(compressor) = params.compressor.as_ref() {
        apply_compressor(&mut samples, out_channels, sample_rate, compressor);
    }
    Ok(samples)
}

fn fit_channels(input: &[f32], in_channels: usize, out_channels: usize) -> Vec<f32> {
    let frames = input.len() / in_channels;
    let mut output = vec![0.0; frames * out_channels];
    for frame in 0..frames {
        if in_channels == 1 && out_channels >= 2 {
            let sample = input[frame];
            output[frame * out_channels] = sample;
            output[frame * out_channels + 1] = sample;
        } else if out_channels == 1 && in_channels >= 2 {
            let left = input[frame * in_channels];
            let right = input[frame * in_channels + 1];
            output[frame] = 0.5 * (left + right);
        } else {
            for channel in 0..out_channels.min(in_channels) {
                output[frame * out_channels + channel] = input[frame * in_channels + channel];
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compressor::CompressorSettings;
    use crate::curve::GainKeyframe;
    use crate::effects::{compressor_effect, gain_effect, pan_effect};
    use videocut_model::{Interp, TICKS_PER_SECOND};

    #[test]
    fn keyframed_gain_pan_and_compressor_process_a_buffer() {
        let gain = gain_effect(
            0.0,
            &[
                GainKeyframe {
                    time_ticks: 0,
                    gain_db: 0.0,
                    interp: Interp::Linear,
                },
                GainKeyframe {
                    time_ticks: TICKS_PER_SECOND / 2,
                    gain_db: -6.0,
                    interp: Interp::Linear,
                },
            ],
        )
        .unwrap();
        let pan = pan_effect(-1.0).unwrap();
        let compressor = compressor_effect(CompressorSettings {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 0.0,
            release_ms: 0.0,
            knee_db: 0.0,
            makeup_db: 0.0,
        })
        .unwrap();
        let input = vec![1.0f32; 3];
        let processed = process_clip_audio(&input, 1, 2, 4, 0, &[gain, pan]).unwrap();
        assert_eq!(processed.len(), 6);
        let mid = db_to_amp(-3.0);
        assert!((processed[2] - mid).abs() < 1e-4, "left {}", processed[2]);
        assert_eq!(processed[3], 0.0);

        let compressed = process_clip_audio(&input, 1, 1, 48_000, 0, &[compressor]).unwrap();
        let target = 10f64.powf(-15.0 / 20.0) as f32;
        assert!((compressed[0] - target).abs() < 1e-4);
    }

    fn db_to_amp(db: f64) -> f32 {
        10f64.powf(db / 20.0) as f32
    }
}
