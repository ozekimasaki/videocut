//! Tick conversions for OTIO rational time, FCPXML rational seconds, and EDL timecode.
//!
//! One second is [`TICKS_PER_SECOND`]. Frame-aligned times stay on integer frame
//! boundaries for the sequence frame rate.

use videocut_model::{Rational, TICKS_PER_SECOND};

use crate::error::Error;

pub fn mul_div(value: i128, mul: i128, div: i128) -> i64 {
    if div == 0 || mul == 0 || value == 0 {
        return 0;
    }
    let neg = (value < 0) ^ (mul < 0) ^ (div < 0);
    let num = value.abs().saturating_mul(mul.abs());
    let den = div.abs();
    let q = num / den;
    let r = num % den;
    let rounded = if r * 2 >= den { q + 1 } else { q };
    let signed = if neg { -rounded } else { rounded };
    i64::try_from(signed).unwrap_or(if neg { i64::MIN } else { i64::MAX })
}

pub fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a.max(1)
}

pub fn reduce(mut n: i128, mut d: i128) -> (i128, i128) {
    if d < 0 {
        n = -n;
        d = -d;
    }
    if d == 0 {
        return (0, 1);
    }
    let g = gcd(n, d);
    (n / g, d / g)
}

/// Encode ticks as an integer OTIO `RationalTime` `(value, rate)`.
///
/// Frame-aligned times use `value = frames * fps.den` and `rate = fps.num`,
/// so `value / rate` is the time in seconds. Other times use the tick rate.
pub fn ticks_to_rational(ticks: i64, fps: Rational) -> (i64, i64) {
    if fps.num > 0 && fps.den > 0 {
        let num = ticks as i128 * i128::from(fps.num);
        let den = i128::from(TICKS_PER_SECOND) * i128::from(fps.den);
        if den != 0 && num % den == 0 {
            let frames = num / den;
            let value = frames * i128::from(fps.den);
            if let (Ok(v), Ok(r)) = (i64::try_from(value), i64::try_from(fps.num)) {
                return (v, r);
            }
        }
    }
    (ticks, TICKS_PER_SECOND)
}

pub fn rational_to_ticks(value: i128, rate: i128) -> i64 {
    mul_div(value, i128::from(TICKS_PER_SECOND), rate)
}

pub fn f64_to_ticks(value: f64, rate: f64) -> i64 {
    if let Some((num, den)) = snap_rate(rate) {
        if value.is_finite() {
            let scaled = (value * den as f64).round() as i128;
            return mul_div(scaled, i128::from(TICKS_PER_SECOND), num);
        }
    }
    if !value.is_finite() || !rate.is_finite() || rate == 0.0 {
        return 0;
    }
    seconds_to_ticks(value / rate)
}

fn snap_rate(rate: f64) -> Option<(i128, i128)> {
    const KNOWN: &[(f64, i128, i128)] = &[
        (23.976, 24000, 1001),
        (23.98, 24000, 1001),
        (24.0, 24, 1),
        (25.0, 25, 1),
        (29.97, 30000, 1001),
        (30.0, 30, 1),
        (48.0, 48, 1),
        (50.0, 50, 1),
        (59.94, 60000, 1001),
        (60.0, 60, 1),
    ];
    KNOWN
        .iter()
        .find(|(approx, _, _)| (rate - approx).abs() < 0.02)
        .map(|(_, n, d)| (*n, *d))
}

pub fn seconds_to_ticks(seconds: f64) -> i64 {
    if !seconds.is_finite() {
        return 0;
    }
    let neg = seconds < 0.0;
    // 254016000000 is exactly representable in f64.
    let ticks = (seconds.abs() * TICKS_PER_SECOND as f64).round() as i64;
    if neg {
        -ticks
    } else {
        ticks
    }
}

pub fn format_fcpx_time(ticks: i64) -> String {
    if ticks == 0 {
        return "0s".to_string();
    }
    let neg = ticks < 0;
    let (n, d) = reduce(i128::from(ticks.abs()), i128::from(TICKS_PER_SECOND));
    let body = if d == 1 {
        format!("{n}s")
    } else {
        format!("{n}/{d}s")
    };
    if neg {
        format!("-{body}")
    } else {
        body
    }
}

pub fn parse_fcpx_time(text: &str) -> Result<i64, Error> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::Fcpxml("missing time value".into()));
    }
    let (neg, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let rest = rest.strip_suffix('s').unwrap_or(rest);
    let (num, den) = if let Some((a, b)) = rest.split_once('/') {
        (parse_i128(a)?, parse_i128(b)?)
    } else {
        (parse_i128(rest)?, 1)
    };
    if den == 0 {
        return Err(Error::Fcpxml(format!("invalid time '{text}'")));
    }
    let ticks = rational_to_ticks(num, den);
    Ok(if neg { -ticks } else { ticks })
}

fn parse_i128(text: &str) -> Result<i128, Error> {
    text.trim()
        .parse::<i128>()
        .map_err(|_| Error::Fcpxml(format!("invalid integer '{text}'")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timecode {
    pub hours: i64,
    pub minutes: i64,
    pub seconds: i64,
    pub frames: i64,
    pub drop_frame: bool,
}

pub fn nominal_fps(fps: Rational) -> i64 {
    if fps.num <= 0 || fps.den <= 0 {
        return 30;
    }
    let rounded = (f64::from(fps.num) / f64::from(fps.den)).round() as i64;
    rounded.max(1)
}

pub fn is_drop_frame(fps: Rational) -> bool {
    let nominal = nominal_fps(fps);
    (nominal == 30 || nominal == 60) && fps.den != 1
}

pub fn ticks_to_frames(ticks: i64, fps: Rational) -> i64 {
    if fps.num <= 0 || fps.den <= 0 {
        return 0;
    }
    mul_div(
        i128::from(ticks),
        i128::from(fps.num),
        i128::from(TICKS_PER_SECOND) * i128::from(fps.den),
    )
}

pub fn frames_to_ticks(frames: i64, fps: Rational) -> i64 {
    videocut_model::ticks_from_frames(frames, fps).unwrap_or(0)
}

pub fn format_timecode(frame_count: i64, fps: Rational, drop_frame: bool) -> String {
    let nominal = nominal_fps(fps);
    let (hours, minutes, seconds, frames) = if drop_frame && (nominal == 30 || nominal == 60) {
        frames_to_drop_parts(frame_count, nominal)
    } else {
        frames_to_ndf_parts(frame_count, nominal)
    };
    let sep = if drop_frame { ';' } else { ':' };
    format!("{hours:02}:{minutes:02}:{seconds:02}{sep}{frames:02}")
}

pub fn parse_timecode(text: &str) -> Option<Timecode> {
    let text = text.trim();
    let bytes = text.as_bytes();
    let sep_at = bytes.iter().rposition(|b| *b == b':' || *b == b';')?;
    let drop_frame = bytes[sep_at] == b';';
    let frames: i64 = text[sep_at + 1..].parse().ok()?;
    let hms = &text[..sep_at];
    let mut parts = hms.split(':');
    let hours: i64 = parts.next()?.parse().ok()?;
    let minutes: i64 = parts.next()?.parse().ok()?;
    let seconds: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(Timecode {
        hours,
        minutes,
        seconds,
        frames,
        drop_frame,
    })
}

pub fn timecode_to_frames(tc: Timecode, fps: Rational, force_drop: bool) -> i64 {
    let nominal = nominal_fps(fps);
    let drop = (tc.drop_frame || force_drop) && (nominal == 30 || nominal == 60);
    if drop {
        drop_parts_to_frames(tc.hours, tc.minutes, tc.seconds, tc.frames, nominal)
    } else {
        ((tc.hours * 60 + tc.minutes) * 60 + tc.seconds) * nominal + tc.frames
    }
}

fn frames_to_ndf_parts(frame_count: i64, nominal: i64) -> (i64, i64, i64, i64) {
    let nominal = nominal.max(1);
    let neg = frame_count < 0;
    let mut frames = frame_count.abs();
    let f = frames % nominal;
    frames /= nominal;
    let seconds = frames % 60;
    frames /= 60;
    let minutes = frames % 60;
    let hours = frames / 60;
    if neg {
        (-hours, -minutes, -seconds, -f)
    } else {
        (hours, minutes, seconds, f)
    }
}

fn frames_to_drop_parts(frame_count: i64, nominal: i64) -> (i64, i64, i64, i64) {
    let drop = drop_frames(nominal);
    if drop == 0 {
        return frames_to_ndf_parts(frame_count, nominal);
    }
    let frames_per_minute = nominal * 60 - drop;
    let frames_per_10 = nominal * 60 * 10 - drop * 9;
    let d = frame_count / frames_per_10;
    let m = frame_count % frames_per_10;
    let adjusted = if m > drop {
        frame_count + drop * 9 * d + drop * ((m - drop) / frames_per_minute)
    } else {
        frame_count + drop * 9 * d
    };
    frames_to_ndf_parts(adjusted, nominal)
}

fn drop_parts_to_frames(hours: i64, minutes: i64, seconds: i64, frames: i64, nominal: i64) -> i64 {
    let drop = drop_frames(nominal);
    let total_minutes = hours * 60 + minutes;
    ((hours * 3600 + minutes * 60 + seconds) * nominal + frames)
        - drop * (total_minutes - total_minutes / 10)
}

fn drop_frames(nominal: i64) -> i64 {
    match nominal {
        30 => 2,
        60 => 4,
        _ => 0,
    }
}

pub fn scale_ticks(ticks: i64, speed: f64) -> i64 {
    if (speed - 1.0).abs() < 1e-12 {
        ticks
    } else if speed == 0.0 {
        0
    } else {
        (ticks as f64 * speed).round() as i64
    }
}

pub fn parse_fps_token(text: &str) -> Option<Rational> {
    let text = text.trim();
    if let Some((n, d)) = text.split_once('/') {
        let num = n.trim().parse().ok()?;
        let den = d.trim().parse().ok()?;
        return Rational::fps(num, den).ok();
    }
    let num = text.parse().ok()?;
    Rational::fps(num, 1).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fcpx_time_roundtrip_keeps_half_seconds() {
        let half = TICKS_PER_SECOND / 2;
        assert_eq!(format_fcpx_time(0), "0s");
        assert_eq!(format_fcpx_time(TICKS_PER_SECOND), "1s");
        assert_eq!(format_fcpx_time(half), "1/2s");
        assert_eq!(parse_fcpx_time("1/2s").unwrap(), half);
        assert_eq!(parse_fcpx_time("0s").unwrap(), 0);
        assert_eq!(parse_fcpx_time("-1s").unwrap(), -TICKS_PER_SECOND);
    }

    #[test]
    fn thirty_fps_timecode_and_drop_frame() {
        let fps = Rational { num: 30, den: 1 };
        assert_eq!(format_timecode(30, fps, false), "00:00:01:00");
        assert_eq!(format_timecode(165, fps, false), "00:00:05:15");
        let parsed = parse_timecode("00:00:05:15").unwrap();
        assert_eq!(timecode_to_frames(parsed, fps, false), 165);

        let df = Rational {
            num: 30000,
            den: 1001,
        };
        assert_eq!(format_timecode(1800, df, true), "00:01:00;02");
        let parsed = parse_timecode("00:01:00;02").unwrap();
        assert!(parsed.drop_frame);
        assert_eq!(timecode_to_frames(parsed, df, true), 1800);
    }

    #[test]
    fn rational_time_is_exact_at_30fps() {
        let fps = Rational { num: 30, den: 1 };
        let ticks = 5 * TICKS_PER_SECOND + TICKS_PER_SECOND / 2;
        let (value, rate) = ticks_to_rational(ticks, fps);
        assert_eq!(rate, 30);
        assert_eq!(
            rational_to_ticks(i128::from(value), i128::from(rate)),
            ticks
        );
    }
}
