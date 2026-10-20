use serde::{Deserialize, Serialize};

use crate::AudioError;

const EQ_GAIN_MIN_DB: f64 = -24.0;
const EQ_GAIN_MAX_DB: f64 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqSettings {
    pub low_freq_hz: f64,
    pub low_gain_db: f64,
    pub mid_freq_hz: f64,
    pub mid_gain_db: f64,
    pub mid_q: f64,
    pub high_freq_hz: f64,
    pub high_gain_db: f64,
}

impl Default for EqSettings {
    fn default() -> Self {
        Self {
            low_freq_hz: 100.0,
            low_gain_db: 0.0,
            mid_freq_hz: 1000.0,
            mid_gain_db: 0.0,
            mid_q: 1.0,
            high_freq_hz: 8000.0,
            high_gain_db: 0.0,
        }
    }
}

impl EqSettings {
    pub fn is_neutral(&self) -> bool {
        self.low_gain_db.abs() < 1e-6
            && self.mid_gain_db.abs() < 1e-6
            && self.high_gain_db.abs() < 1e-6
    }
}

#[derive(Debug, Clone)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    fn process(&mut self, sample: f32) -> f32 {
        let x = sample as f64;
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y as f32
    }

    #[cfg(test)]
    fn response_db(&self, freq_hz: f64, sample_rate: f64) -> f64 {
        let omega = std::f64::consts::TAU * freq_hz / sample_rate;
        let z = (omega.cos(), -omega.sin());
        let z2 = complex_mul(z, z);
        let numerator = complex_add(
            (self.b0, 0.0),
            complex_add(
                complex_mul((self.b1, 0.0), z),
                complex_mul((self.b2, 0.0), z2),
            ),
        );
        let denominator = complex_add(
            (1.0, 0.0),
            complex_add(
                complex_mul((self.a1, 0.0), z),
                complex_mul((self.a2, 0.0), z2),
            ),
        );
        let magnitude = complex_abs(numerator) / complex_abs(denominator);
        if magnitude <= 1e-12 {
            -120.0
        } else {
            20.0 * magnitude.log10()
        }
    }
}

pub fn apply_eq(
    samples: &mut [f32],
    channels: usize,
    sample_rate: u32,
    settings: &EqSettings,
) -> Result<(), AudioError> {
    if channels == 0 || samples.len() % channels != 0 {
        return Err(AudioError::BufferLength);
    }
    if settings.is_neutral() {
        return Ok(());
    }
    let prototypes = design_eq(settings, sample_rate)?;
    if prototypes.is_empty() {
        return Ok(());
    }
    let mut states: Vec<Vec<Biquad>> = (0..channels).map(|_| prototypes.clone()).collect();
    let frames = samples.len() / channels;
    for frame in 0..frames {
        for channel in 0..channels {
            let index = frame * channels + channel;
            let mut sample = samples[index];
            for filter in &mut states[channel] {
                sample = filter.process(sample);
            }
            samples[index] = sample;
        }
    }
    Ok(())
}

fn design_eq(settings: &EqSettings, sample_rate: u32) -> Result<Vec<Biquad>, AudioError> {
    if sample_rate == 0 {
        return Err(AudioError::InvalidSampleRate);
    }
    let rate = sample_rate as f64;
    let mut filters = Vec::new();
    if settings.low_gain_db.abs() >= 1e-6 {
        if let Some(freq) = usable_freq(settings.low_freq_hz, rate) {
            filters.push(low_shelf(freq, settings.low_gain_db, rate));
        }
    }
    if settings.mid_gain_db.abs() >= 1e-6 {
        if let Some(freq) = usable_freq(settings.mid_freq_hz, rate) {
            let q = if settings.mid_q.is_finite() {
                settings.mid_q.clamp(0.1, 18.0)
            } else {
                1.0
            };
            filters.push(peaking(freq, q, settings.mid_gain_db, rate));
        }
    }
    if settings.high_gain_db.abs() >= 1e-6 {
        if let Some(freq) = usable_freq(settings.high_freq_hz, rate) {
            filters.push(high_shelf(freq, settings.high_gain_db, rate));
        }
    }
    Ok(filters)
}

fn usable_freq(freq: f64, sample_rate: f64) -> Option<f64> {
    if !freq.is_finite() || freq <= 0.0 {
        return None;
    }
    let nyquist = sample_rate * 0.45;
    if nyquist <= 10.0 {
        return None;
    }
    Some(freq.clamp(10.0, nyquist))
}

fn low_shelf(freq: f64, gain_db: f64, sample_rate: f64) -> Biquad {
    let (a, alpha, cos_w) = shelf_parts(freq, gain_db, sample_rate);
    let sqrt_a = a.sqrt();
    let two_sqrt_a_alpha = 2.0 * sqrt_a * alpha;
    coeffs(
        a * ((a + 1.0) - (a - 1.0) * cos_w + two_sqrt_a_alpha),
        2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w),
        a * ((a + 1.0) - (a - 1.0) * cos_w - two_sqrt_a_alpha),
        (a + 1.0) + (a - 1.0) * cos_w + two_sqrt_a_alpha,
        -2.0 * ((a - 1.0) + (a + 1.0) * cos_w),
        (a + 1.0) + (a - 1.0) * cos_w - two_sqrt_a_alpha,
    )
}

fn high_shelf(freq: f64, gain_db: f64, sample_rate: f64) -> Biquad {
    let (a, alpha, cos_w) = shelf_parts(freq, gain_db, sample_rate);
    let sqrt_a = a.sqrt();
    let two_sqrt_a_alpha = 2.0 * sqrt_a * alpha;
    coeffs(
        a * ((a + 1.0) + (a - 1.0) * cos_w + two_sqrt_a_alpha),
        -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w),
        a * ((a + 1.0) + (a - 1.0) * cos_w - two_sqrt_a_alpha),
        (a + 1.0) - (a - 1.0) * cos_w + two_sqrt_a_alpha,
        2.0 * ((a - 1.0) - (a + 1.0) * cos_w),
        (a + 1.0) - (a - 1.0) * cos_w - two_sqrt_a_alpha,
    )
}

fn peaking(freq: f64, q: f64, gain_db: f64, sample_rate: f64) -> Biquad {
    let a = 10f64.powf(gain_db / 40.0);
    let omega = std::f64::consts::TAU * freq / sample_rate;
    let alpha = omega.sin() / (2.0 * q);
    let cos_w = omega.cos();
    coeffs(
        1.0 + alpha * a,
        -2.0 * cos_w,
        1.0 - alpha * a,
        1.0 + alpha / a,
        -2.0 * cos_w,
        1.0 - alpha / a,
    )
}

fn shelf_parts(freq: f64, gain_db: f64, sample_rate: f64) -> (f64, f64, f64) {
    let a = 10f64.powf(gain_db / 40.0);
    let omega = std::f64::consts::TAU * freq / sample_rate;
    let alpha = omega.sin() / 2.0 * std::f64::consts::SQRT_2;
    (a, alpha, omega.cos())
}

fn coeffs(b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Biquad {
    Biquad {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
        z1: 0.0,
        z2: 0.0,
    }
}

#[cfg(test)]
fn complex_mul(left: (f64, f64), right: (f64, f64)) -> (f64, f64) {
    (
        left.0 * right.0 - left.1 * right.1,
        left.0 * right.1 + left.1 * right.0,
    )
}

#[cfg(test)]
fn complex_add(left: (f64, f64), right: (f64, f64)) -> (f64, f64) {
    (left.0 + right.0, left.1 + right.1)
}

#[cfg(test)]
fn complex_abs(value: (f64, f64)) -> f64 {
    value.0.hypot(value.1)
}

pub(crate) fn clamp_eq_gain(db: f64) -> f64 {
    if db.is_finite() {
        db.clamp(EQ_GAIN_MIN_DB, EQ_GAIN_MAX_DB)
    } else {
        0.0
    }
}

pub(crate) fn clamp_eq_freq(freq: f64) -> f64 {
    if freq.is_finite() {
        freq.clamp(10.0, 20_000.0)
    } else {
        1_000.0
    }
}

pub(crate) fn clamp_q(q: f64) -> f64 {
    if q.is_finite() {
        q.clamp(0.1, 18.0)
    } else {
        1.0
    }
}

pub(crate) fn eq_in_range(settings: &EqSettings) -> bool {
    finite_range(settings.low_freq_hz, 10.0, 20_000.0)
        && finite_range(settings.low_gain_db, EQ_GAIN_MIN_DB, EQ_GAIN_MAX_DB)
        && finite_range(settings.mid_freq_hz, 10.0, 20_000.0)
        && finite_range(settings.mid_gain_db, EQ_GAIN_MIN_DB, EQ_GAIN_MAX_DB)
        && finite_range(settings.mid_q, 0.1, 18.0)
        && finite_range(settings.high_freq_hz, 10.0, 20_000.0)
        && finite_range(settings.high_gain_db, EQ_GAIN_MIN_DB, EQ_GAIN_MAX_DB)
}

fn finite_range(value: f64, min: f64, max: f64) -> bool {
    value.is_finite() && (min..=max).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaking_center_matches_the_requested_gain() {
        let filter = peaking(1_000.0, 1.0, 6.0, 48_000.0);
        let response = filter.response_db(1_000.0, 48_000.0);
        assert!((response - 6.0).abs() < 0.05, "response {response}");
    }

    #[test]
    fn low_shelf_lifts_dc_and_leaves_highs() {
        let mut settings = EqSettings::default();
        settings.low_freq_hz = 200.0;
        settings.low_gain_db = 6.0;
        let mut samples = vec![0.1f32; 4_000];
        apply_eq(&mut samples, 1, 48_000, &settings).unwrap();
        let settled = samples[samples.len() - 1];
        assert!((settled - 0.2).abs() < 0.01, "settled {settled}");

        let filters = design_eq(&settings, 48_000).unwrap();
        let high = filters[0].response_db(12_000.0, 48_000.0);
        assert!(high.abs() < 0.4, "high {high}");
    }

    #[test]
    fn neutral_eq_does_not_change_samples() {
        let mut samples = [0.2, -0.4, 0.6];
        let original = samples;
        apply_eq(&mut samples, 1, 48_000, &EqSettings::default()).unwrap();
        assert_eq!(samples, original);
    }
}
