use videocut_model::{effect_ids, new_id, EffectInstance, Keyframe, Param, ParamValue};

use crate::compressor::{clamp_compressor, compressor_in_range, CompressorSettings};
use crate::curve::GainKeyframe;
use crate::eq::{clamp_eq_freq, clamp_eq_gain, clamp_q, eq_in_range, EqSettings};
use crate::param_ids;
use crate::units::{clamp_gain_db, clamp_pan, GAIN_MAX_DB, GAIN_MIN_DB, PAN_MAX, PAN_MIN};
use crate::AudioError;

#[derive(Debug, Clone, PartialEq)]
pub struct ClipAudioParams {
    pub gain_db: f64,
    pub gain_keyframes: Vec<GainKeyframe>,
    pub pan: f64,
    pub eq: Option<EqSettings>,
    pub compressor: Option<CompressorSettings>,
}

impl Default for ClipAudioParams {
    fn default() -> Self {
        Self {
            gain_db: 0.0,
            gain_keyframes: Vec::new(),
            pan: 0.0,
            eq: None,
            compressor: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedClipAudio {
    pub params: ClipAudioParams,
    pub ignored_duplicate: bool,
}

pub fn parse_clip_audio(effects: &[EffectInstance]) -> ParsedClipAudio {
    let mut params = ClipAudioParams::default();
    let mut saw_gain = false;
    let mut saw_pan = false;
    let mut saw_eq = false;
    let mut saw_compressor = false;
    let mut ignored_duplicate = false;
    for effect in effects {
        if !effect.enabled {
            continue;
        }
        if effect.effect_id == effect_ids::GAIN {
            if saw_gain {
                ignored_duplicate = true;
                continue;
            }
            saw_gain = true;
            if let Some(param) = find_param(effect, param_ids::GAIN) {
                params.gain_db = match float_value(&param.value) {
                    Some(value) => clamp_gain_db(value),
                    None => 0.0,
                };
                params.gain_keyframes = gain_keyframes(param);
            }
        } else if effect.effect_id == effect_ids::PAN {
            if saw_pan {
                ignored_duplicate = true;
                continue;
            }
            saw_pan = true;
            params.pan = find_param(effect, param_ids::PAN)
                .and_then(|param| float_value(&param.value))
                .map(clamp_pan)
                .unwrap_or(0.0);
        } else if effect.effect_id == effect_ids::EQ {
            if saw_eq {
                ignored_duplicate = true;
                continue;
            }
            saw_eq = true;
            params.eq = Some(read_eq(effect));
        } else if effect.effect_id == effect_ids::COMPRESSOR {
            if saw_compressor {
                ignored_duplicate = true;
                continue;
            }
            saw_compressor = true;
            params.compressor = Some(read_compressor(effect));
        }
    }
    ParsedClipAudio {
        params,
        ignored_duplicate,
    }
}

pub fn clip_audio_params(effects: &[EffectInstance]) -> ClipAudioParams {
    parse_clip_audio(effects).params
}

pub fn gain_effect(gain_db: f64, keyframes: &[GainKeyframe]) -> Result<EffectInstance, AudioError> {
    if !in_gain_range(gain_db) {
        return Err(AudioError::InvalidParameter);
    }
    let mut keys = Vec::with_capacity(keyframes.len());
    for key in keyframes {
        if !in_gain_range(key.gain_db) {
            return Err(AudioError::InvalidParameter);
        }
        keys.push(Keyframe {
            time_ticks: key.time_ticks,
            value: ParamValue::Float(key.gain_db),
            interp: key.interp,
        });
    }
    Ok(instance(
        effect_ids::GAIN,
        vec![float_param(param_ids::GAIN, gain_db, keys)],
    ))
}

pub fn pan_effect(pan: f64) -> Result<EffectInstance, AudioError> {
    if !pan.is_finite() || !(PAN_MIN..=PAN_MAX).contains(&pan) {
        return Err(AudioError::InvalidParameter);
    }
    Ok(instance(
        effect_ids::PAN,
        vec![float_param(param_ids::PAN, pan, Vec::new())],
    ))
}

pub fn eq_effect(settings: EqSettings) -> Result<EffectInstance, AudioError> {
    if !eq_in_range(&settings) {
        return Err(AudioError::InvalidParameter);
    }
    Ok(instance(
        effect_ids::EQ,
        vec![
            float_param(param_ids::LOW_FREQ, settings.low_freq_hz, Vec::new()),
            float_param(param_ids::LOW_GAIN, settings.low_gain_db, Vec::new()),
            float_param(param_ids::MID_FREQ, settings.mid_freq_hz, Vec::new()),
            float_param(param_ids::MID_GAIN, settings.mid_gain_db, Vec::new()),
            float_param(param_ids::MID_Q, settings.mid_q, Vec::new()),
            float_param(param_ids::HIGH_FREQ, settings.high_freq_hz, Vec::new()),
            float_param(param_ids::HIGH_GAIN, settings.high_gain_db, Vec::new()),
        ],
    ))
}

pub fn compressor_effect(settings: CompressorSettings) -> Result<EffectInstance, AudioError> {
    if !compressor_in_range(&settings) {
        return Err(AudioError::InvalidParameter);
    }
    Ok(instance(
        effect_ids::COMPRESSOR,
        vec![
            float_param(param_ids::THRESHOLD, settings.threshold_db, Vec::new()),
            float_param(param_ids::RATIO, settings.ratio, Vec::new()),
            float_param(param_ids::ATTACK, settings.attack_ms, Vec::new()),
            float_param(param_ids::RELEASE, settings.release_ms, Vec::new()),
            float_param(param_ids::KNEE, settings.knee_db, Vec::new()),
            float_param(param_ids::MAKEUP, settings.makeup_db, Vec::new()),
        ],
    ))
}

fn read_eq(effect: &EffectInstance) -> EqSettings {
    let defaults = EqSettings::default();
    EqSettings {
        low_freq_hz: clamp_eq_freq(optional_float(
            effect,
            param_ids::LOW_FREQ,
            defaults.low_freq_hz,
        )),
        low_gain_db: clamp_eq_gain(optional_float(
            effect,
            param_ids::LOW_GAIN,
            defaults.low_gain_db,
        )),
        mid_freq_hz: clamp_eq_freq(optional_float(
            effect,
            param_ids::MID_FREQ,
            defaults.mid_freq_hz,
        )),
        mid_gain_db: clamp_eq_gain(optional_float(
            effect,
            param_ids::MID_GAIN,
            defaults.mid_gain_db,
        )),
        mid_q: clamp_q(optional_float(effect, param_ids::MID_Q, defaults.mid_q)),
        high_freq_hz: clamp_eq_freq(optional_float(
            effect,
            param_ids::HIGH_FREQ,
            defaults.high_freq_hz,
        )),
        high_gain_db: clamp_eq_gain(optional_float(
            effect,
            param_ids::HIGH_GAIN,
            defaults.high_gain_db,
        )),
    }
}

fn read_compressor(effect: &EffectInstance) -> CompressorSettings {
    let defaults = CompressorSettings::default();
    clamp_compressor(&CompressorSettings {
        threshold_db: optional_float(effect, param_ids::THRESHOLD, defaults.threshold_db),
        ratio: optional_float(effect, param_ids::RATIO, defaults.ratio),
        attack_ms: optional_float(effect, param_ids::ATTACK, defaults.attack_ms),
        release_ms: optional_float(effect, param_ids::RELEASE, defaults.release_ms),
        knee_db: optional_float(effect, param_ids::KNEE, defaults.knee_db),
        makeup_db: optional_float(effect, param_ids::MAKEUP, defaults.makeup_db),
    })
}

fn gain_keyframes(param: &Param) -> Vec<GainKeyframe> {
    param
        .keyframes
        .iter()
        .filter_map(|key| {
            float_value(&key.value).map(|gain_db| GainKeyframe {
                time_ticks: key.time_ticks,
                gain_db: clamp_gain_db(gain_db),
                interp: key.interp,
            })
        })
        .collect()
}

fn optional_float(effect: &EffectInstance, id: &str, default: f64) -> f64 {
    find_param(effect, id)
        .and_then(|param| float_value(&param.value))
        .unwrap_or(default)
}

fn find_param<'a>(effect: &'a EffectInstance, id: &str) -> Option<&'a Param> {
    effect.params.iter().find(|param| param.id == id)
}

fn float_value(value: &ParamValue) -> Option<f64> {
    match value {
        ParamValue::Float(number) if number.is_finite() => Some(*number),
        ParamValue::Int(number) => Some(*number as f64),
        ParamValue::Float(_)
        | ParamValue::Bool(_)
        | ParamValue::Color(_)
        | ParamValue::Point(_)
        | ParamValue::Str(_) => None,
    }
}

fn in_gain_range(db: f64) -> bool {
    db.is_finite() && (GAIN_MIN_DB..=GAIN_MAX_DB).contains(&db)
}

fn float_param(id: &str, value: f64, keyframes: Vec<Keyframe>) -> Param {
    Param {
        id: id.to_string(),
        value: ParamValue::Float(value),
        keyframes,
    }
}

fn instance(effect_id: &str, params: Vec<Param>) -> EffectInstance {
    EffectInstance {
        id: new_id("fx"),
        effect_id: effect_id.to_string(),
        enabled: true,
        params,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use videocut_model::{effect_ids, Interp};

    #[test]
    fn builders_round_trip_through_the_parser() {
        let gain = gain_effect(
            -6.0,
            &[GainKeyframe {
                time_ticks: 0,
                gain_db: -6.0,
                interp: Interp::Linear,
            }],
        )
        .unwrap();
        let pan = pan_effect(-0.25).unwrap();
        let mut eq = EqSettings::default();
        eq.mid_gain_db = 3.0;
        let eq = eq_effect(eq).unwrap();
        let compressor = compressor_effect(CompressorSettings::default()).unwrap();
        let parsed = parse_clip_audio(&[gain, pan, eq, compressor]);
        assert!((parsed.params.gain_db + 6.0).abs() < 1e-9);
        assert_eq!(parsed.params.gain_keyframes.len(), 1);
        assert!((parsed.params.pan + 0.25).abs() < 1e-9);
        assert!((parsed.params.eq.unwrap().mid_gain_db - 3.0).abs() < 1e-9);
        assert!((parsed.params.compressor.unwrap().ratio - 4.0).abs() < 1e-9);
        assert!(!parsed.ignored_duplicate);
    }

    #[test]
    fn disabled_and_duplicate_effects_are_ignored() {
        let mut first = gain_effect(-3.0, &[]).unwrap();
        first.enabled = false;
        let second = gain_effect(-6.0, &[]).unwrap();
        let third = gain_effect(-9.0, &[]).unwrap();
        assert_eq!(second.effect_id, effect_ids::GAIN);
        let parsed = parse_clip_audio(&[first, second, third]);
        assert!((parsed.params.gain_db + 6.0).abs() < 1e-9);
        assert!(parsed.ignored_duplicate);
    }

    #[test]
    fn builders_reject_out_of_range_values() {
        assert_eq!(gain_effect(80.0, &[]), Err(AudioError::InvalidParameter));
        assert_eq!(pan_effect(2.0), Err(AudioError::InvalidParameter));
        let mut eq = EqSettings::default();
        eq.mid_q = 0.0;
        assert_eq!(eq_effect(eq), Err(AudioError::InvalidParameter));
    }
}
