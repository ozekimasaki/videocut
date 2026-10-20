use crate::units::{linear_to_db, SILENCE_DB};
use crate::AudioError;

#[derive(Debug, Clone, PartialEq)]
pub struct ChannelMeter {
    pub peak: f32,
    pub peak_dbfs: f64,
    pub rms_dbfs: f64,
    pub clipped: bool,
}

pub fn measure(interleaved: &[f32], channels: usize) -> Result<Vec<ChannelMeter>, AudioError> {
    if channels == 0 {
        return Err(AudioError::InvalidChannels);
    }
    if interleaved.len() % channels != 0 {
        return Err(AudioError::BufferLength);
    }
    let frames = interleaved.len() / channels;
    let mut peak = vec![0.0f32; channels];
    let mut sum_sq = vec![0.0f64; channels];
    for frame in 0..frames {
        for channel in 0..channels {
            let sample = interleaved[frame * channels + channel];
            let amplitude = sample.abs();
            if amplitude > peak[channel] {
                peak[channel] = amplitude;
            }
            sum_sq[channel] += (sample as f64) * (sample as f64);
        }
    }
    Ok((0..channels)
        .map(|channel| {
            let rms = if frames == 0 {
                0.0
            } else {
                (sum_sq[channel] / frames as f64).sqrt()
            };
            ChannelMeter {
                peak: peak[channel],
                peak_dbfs: level_db(peak[channel] as f64),
                rms_dbfs: level_db(rms),
                clipped: peak[channel] >= 1.0,
            }
        })
        .collect())
}

fn level_db(linear: f64) -> f64 {
    if linear <= 0.0 {
        SILENCE_DB
    } else {
        linear_to_db(linear)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_rms_and_clip_are_measured_per_channel() {
        let meters = measure(&[1.0, -1.0, 0.0, 0.0], 2).unwrap();
        assert!((meters[0].peak - 1.0).abs() < 1e-6);
        assert!(meters[0].clipped);
        assert!(meters[0].peak_dbfs.abs() < 1e-6);
        assert!((meters[0].rms_dbfs + 3.0103).abs() < 0.02);
        assert!(meters[1].clipped);
    }

    #[test]
    fn silence_uses_the_meter_floor() {
        let meters = measure(&[0.0, 0.0], 1).unwrap();
        assert_eq!(meters[0].peak_dbfs, SILENCE_DB);
        assert!(!meters[0].clipped);
    }
}
