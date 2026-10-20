use videocut_model::{Rational, TICKS_PER_SECOND};

use crate::error::MediaError;

/// Format ticks as a decimal second string ffmpeg can parse.
///
/// Values that divide `TICKS_PER_SECOND` stay exact (`1`, `0.5`). Other values
/// keep up to 12 decimal digits.
pub fn format_ticks_as_seconds(ticks: i64) -> String {
    let sign = if ticks < 0 { "-" } else { "" };
    let ticks = ticks.unsigned_abs();
    let tps = TICKS_PER_SECOND as u64;
    let whole = ticks / tps;
    let mut frac = ticks % tps;
    if frac == 0 {
        return format!("{sign}{whole}");
    }
    let mut digits = String::new();
    for _ in 0..12 {
        frac = frac.saturating_mul(10);
        let digit = frac / tps;
        digits.push(char::from(b'0' + u8::try_from(digit).unwrap_or(0)));
        frac %= tps;
        if frac == 0 {
            break;
        }
    }
    format!("{sign}{whole}.{digits}")
}

/// Parse a non-negative decimal second string into ticks.
pub fn parse_decimal_seconds_to_ticks(text: &str) -> Result<i64, MediaError> {
    let text = text.trim();
    if text.is_empty() || text.eq_ignore_ascii_case("n/a") {
        return Err(MediaError::Invalid("missing duration".into()));
    }
    let negative = text.starts_with('-');
    let text = text.trim_start_matches(['+', '-']);
    if text.is_empty() {
        return Err(MediaError::Invalid(format!("invalid duration '{text}'")));
    }
    let (whole_text, frac_text) = text.split_once('.').unwrap_or((text, ""));
    if whole_text.is_empty() || !whole_text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(MediaError::Invalid(format!("invalid duration '{text}'")));
    }
    let whole: i128 = whole_text
        .parse()
        .map_err(|_| MediaError::Invalid(format!("invalid duration '{text}'")))?;
    let mut frac_value: i128 = 0;
    let mut base: i128 = 1;
    for byte in frac_text.bytes().take(12) {
        let Some(digit) = (byte as char).to_digit(10) else {
            return Err(MediaError::Invalid(format!("invalid duration '{text}'")));
        };
        frac_value = frac_value * 10 + i128::from(digit);
        base *= 10;
    }
    let tps = i128::from(TICKS_PER_SECOND);
    let mut ticks = whole * tps;
    if base > 1 {
        let scaled = frac_value * tps;
        let mut frac_ticks = scaled / base;
        if scaled % base * 2 >= base {
            frac_ticks += 1;
        }
        ticks += frac_ticks;
    }
    if negative {
        ticks = -ticks;
    }
    i64::try_from(ticks).map_err(|_| MediaError::Invalid("duration is out of range".into()))
}

pub fn seconds_f64_to_ticks(seconds: f64) -> Result<i64, MediaError> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(MediaError::Invalid(
            "duration must be a non-negative number".into(),
        ));
    }
    let ticks = (seconds * TICKS_PER_SECOND as f64).round();
    if ticks >= i64::MAX as f64 {
        return Ok(i64::MAX);
    }
    Ok(ticks as i64)
}

pub fn scale_ticks(ticks: i64, speed: f64) -> i64 {
    if !speed.is_finite() || (speed - 1.0).abs() < 1e-9 {
        return ticks;
    }
    let scaled = (ticks as f64) * speed;
    if !scaled.is_finite() {
        return ticks;
    }
    scaled.round() as i64
}

pub fn frame_ticks(fps: Rational) -> Result<i64, MediaError> {
    if fps.num <= 0 || fps.den <= 0 {
        return Err(MediaError::InvalidFrameRate);
    }
    videocut_model::ticks_from_frames(1, fps).map_err(|_| MediaError::InvalidFrameRate)
}

pub fn format_rational(value: Rational) -> Result<String, MediaError> {
    if value.num <= 0 || value.den <= 0 {
        return Err(MediaError::InvalidFrameRate);
    }
    Ok(format!("{}/{}", value.num, value.den))
}

pub fn parse_frame_rate(text: &str) -> Result<Rational, MediaError> {
    let text = text.trim();
    if text.is_empty() || text == "0/0" || text.eq_ignore_ascii_case("n/a") {
        return Err(MediaError::InvalidFrameRate);
    }
    let (num_text, den_text) = text.split_once('/').unwrap_or((text, "1"));
    let num: i32 = num_text.parse().map_err(|_| MediaError::InvalidFrameRate)?;
    let den: i32 = den_text.parse().map_err(|_| MediaError::InvalidFrameRate)?;
    Rational::fps(num, den).map_err(|_| MediaError::InvalidFrameRate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use videocut_model::TICKS_PER_SECOND;

    #[test]
    fn one_second_and_half_second_are_exact() {
        assert_eq!(format_ticks_as_seconds(0), "0");
        assert_eq!(format_ticks_as_seconds(TICKS_PER_SECOND), "1");
        assert_eq!(format_ticks_as_seconds(TICKS_PER_SECOND / 2), "0.5");
        assert_eq!(
            parse_decimal_seconds_to_ticks("1").unwrap(),
            TICKS_PER_SECOND
        );
        assert_eq!(
            parse_decimal_seconds_to_ticks("1.0").unwrap(),
            TICKS_PER_SECOND
        );
        assert_eq!(
            parse_decimal_seconds_to_ticks("0.5").unwrap(),
            TICKS_PER_SECOND / 2
        );
        assert_eq!(
            parse_decimal_seconds_to_ticks("10.0").unwrap(),
            10 * TICKS_PER_SECOND
        );
    }

    #[test]
    fn frame_rate_parser_accepts_drop_frame_rationals() {
        let fps = parse_frame_rate("30000/1001").unwrap();
        assert_eq!(
            fps,
            Rational {
                num: 30000,
                den: 1001
            }
        );
        assert!(parse_frame_rate("0/0").is_err());
    }
}
