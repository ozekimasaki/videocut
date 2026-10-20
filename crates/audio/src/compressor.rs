use serde::{Deserialize, Serialize};

use crate::units::{db_to_linear, linear_to_db, SILENCE_DB};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressorSettings {
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
    pub knee_db: f64,
    pub makeup_db: f64,
}

impl Default for CompressorSettings {
    fn default() -> Self {
        Self {
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            knee_db: 0.0,
            makeup_db: 0.0,
        }
    }
}

impl CompressorSettings {
    pub fn is_neutral(&self) -> bool {
        self.ratio <= 1.0 + 1e-6 && self.makeup_db.abs() < 1e-6
    }
}

pub fn compressor_output_db(input_db: f64, threshold_db: f64, ratio: f64, knee_db: f64) -> f64 {
    let ratio = ratio.max(1.0);
    if ratio <= 1.0 {
        return input_db;
    }
    let knee = knee_db.max(0.0);
    if knee == 0.0 {
        if input_db <= threshold_db {
            input_db
        } else {
            threshold_db + (input_db - threshold_db) / ratio
        }
    } else {
        let half = knee / 2.0;
        if input_db < threshold_db - half {
            input_db
        } else if input_db > threshold_db + half {
            threshold_db + (input_db - threshold_db) / ratio
        } else {
            let width = input_db - threshold_db + half;
            input_db + (1.0 / ratio - 1.0) * (width * width) / (2.0 * knee)
        }
    }
}

pub(crate) fn apply_compressor(
    samples: &mut [f32],
    channels: usize,
    sample_rate: u32,
    settings: &CompressorSettings,
) {
    if channels == 0 || sample_rate == 0 || settings.is_neutral() {
        return;
    }
    let attack = envelope_coeff(settings.attack_ms, sample_rate);
    let release = envelope_coeff(settings.release_ms, sample_rate);
    let makeup = db_to_linear(settings.makeup_db) as f32;
    let mut envelope_db = 0.0;
    let frames = samples.len() / channels;
    for frame in 0..frames {
        let mut peak = 0.0f32;
        for channel in 0..channels {
            peak = peak.max(samples[frame * channels + channel].abs());
        }
        let input_db = if peak == 0.0 {
            SILENCE_DB
        } else {
            linear_to_db(peak as f64)
        };
        let target_db = compressor_output_db(
            input_db,
            settings.threshold_db,
            settings.ratio,
            settings.knee_db,
        ) - input_db;
        let coeff = if target_db < envelope_db {
            attack
        } else {
            release
        };
        envelope_db = target_db + coeff * (envelope_db - target_db);
        let gain = db_to_linear(envelope_db) as f32 * makeup;
        for channel in 0..channels {
            samples[frame * channels + channel] *= gain;
        }
    }
}

fn envelope_coeff(time_ms: f64, sample_rate: u32) -> f64 {
    if time_ms <= 0.0 {
        0.0
    } else {
        (-1.0 / ((time_ms / 1000.0) * sample_rate as f64)).exp()
    }
}

pub(crate) fn compressor_in_range(settings: &CompressorSettings) -> bool {
    finite_range(settings.threshold_db, -96.0, 0.0)
        && finite_range(settings.ratio, 1.0, 100.0)
        && finite_range(settings.attack_ms, 0.0, 5_000.0)
        && finite_range(settings.release_ms, 0.0, 5_000.0)
        && finite_range(settings.knee_db, 0.0, 24.0)
        && finite_range(settings.makeup_db, -24.0, 24.0)
}

pub(crate) fn clamp_compressor(settings: &CompressorSettings) -> CompressorSettings {
    CompressorSettings {
        threshold_db: clamp_or(settings.threshold_db, -96.0, 0.0, -18.0),
        ratio: clamp_or(settings.ratio, 1.0, 100.0, 4.0),
        attack_ms: clamp_or(settings.attack_ms, 0.0, 5_000.0, 10.0),
        release_ms: clamp_or(settings.release_ms, 0.0, 5_000.0, 100.0),
        knee_db: clamp_or(settings.knee_db, 0.0, 24.0, 0.0),
        makeup_db: clamp_or(settings.makeup_db, -24.0, 24.0, 0.0),
    }
}

fn clamp_or(value: f64, min: f64, max: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

fn finite_range(value: f64, min: f64, max: f64) -> bool {
    value.is_finite() && (min..=max).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_knee_reduces_level_above_threshold() {
        let output = compressor_output_db(0.0, -20.0, 4.0, 0.0);
        assert!((output + 15.0).abs() < 1e-9);
        assert!((compressor_output_db(-40.0, -20.0, 4.0, 0.0) + 40.0).abs() < 1e-9);
    }

    #[test]
    fn soft_knee_matches_the_hard_curve_at_its_edges() {
        let threshold = -18.0;
        let ratio = 3.0;
        let knee = 6.0;
        let low = threshold - knee / 2.0;
        let high = threshold + knee / 2.0;
        assert!((compressor_output_db(low, threshold, ratio, knee) - low).abs() < 1e-9);
        let hard_high = threshold + (high - threshold) / ratio;
        assert!((compressor_output_db(high, threshold, ratio, knee) - hard_high).abs() < 1e-9);
    }

    #[test]
    fn zero_attack_reaches_the_static_target() {
        let mut samples = [1.0f32, 0.5];
        apply_compressor(
            &mut samples,
            2,
            48_000,
            &CompressorSettings {
                threshold_db: -20.0,
                ratio: 4.0,
                attack_ms: 0.0,
                release_ms: 0.0,
                knee_db: 0.0,
                makeup_db: 0.0,
            },
        );
        let target = 10f64.powf(-15.0 / 20.0) as f32;
        assert!((samples[0] - target).abs() < 1e-4, "left {}", samples[0]);
        assert!(
            (samples[1] - target * 0.5).abs() < 1e-4,
            "right {}",
            samples[1]
        );
    }
}
