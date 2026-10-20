pub const GAIN_MIN_DB: f64 = -96.0;
pub const GAIN_MAX_DB: f64 = 24.0;
pub const FADER_MIN_DB: f64 = -96.0;
pub const FADER_MAX_DB: f64 = 12.0;
pub const PAN_MIN: f64 = -1.0;
pub const PAN_MAX: f64 = 1.0;
pub const SILENCE_DB: f64 = -120.0;

pub fn db_to_linear(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

pub fn linear_to_db(linear: f64) -> f64 {
    if linear <= 1e-12 {
        SILENCE_DB
    } else {
        20.0 * linear.log10()
    }
}

pub fn clamp_gain_db(db: f64) -> f64 {
    if db.is_finite() {
        db.clamp(GAIN_MIN_DB, GAIN_MAX_DB)
    } else {
        0.0
    }
}

pub fn clamp_fader_db(db: f64) -> f64 {
    if db.is_finite() {
        db.clamp(FADER_MIN_DB, FADER_MAX_DB)
    } else {
        0.0
    }
}

pub fn clamp_pan(pan: f64) -> f64 {
    if pan.is_finite() {
        pan.clamp(PAN_MIN, PAN_MAX)
    } else {
        0.0
    }
}

pub(crate) fn format_ffmpeg_number(value: f64) -> String {
    let finite = if value.is_finite() { value } else { 0.0 };
    let rounded = if finite.abs() < 1e-12 { 0.0 } else { finite };
    let mut text = format!("{rounded:.8}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        "0".to_string()
    } else {
        text
    }
}

pub(crate) fn tick_at_sample(start_tick: i64, sample_index: usize, sample_rate: u32) -> i64 {
    if sample_rate == 0 {
        return start_tick;
    }
    let ticks =
        (sample_index as i128) * (videocut_model::TICKS_PER_SECOND as i128) / (sample_rate as i128);
    start_tick.saturating_add(i64::try_from(ticks).unwrap_or(i64::MAX))
}

pub(crate) fn ticks_to_samples(ticks: i64, sample_rate: u32) -> i64 {
    if ticks <= 0 || sample_rate == 0 {
        return 0;
    }
    let samples =
        (ticks as i128) * (sample_rate as i128) / (videocut_model::TICKS_PER_SECOND as i128);
    i64::try_from(samples).unwrap_or(i64::MAX)
}

pub(crate) fn ticks_to_seconds(ticks: i64) -> f64 {
    ticks as f64 / videocut_model::TICKS_PER_SECOND as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decibel_conversion_matches_known_ratios() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-12);
        assert!((db_to_linear(6.0) - 1.995_262_314_968_879).abs() < 1e-12);
        assert!(linear_to_db(1.0).abs() < 1e-12);
        assert_eq!(linear_to_db(0.0), SILENCE_DB);
    }

    #[test]
    fn non_finite_levels_fall_back_to_unity() {
        assert_eq!(clamp_gain_db(f64::NAN), 0.0);
        assert_eq!(clamp_fader_db(80.0), FADER_MAX_DB);
        assert_eq!(clamp_pan(4.0), 1.0);
    }
}
