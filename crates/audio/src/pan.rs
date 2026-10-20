use std::f64::consts::FRAC_PI_2;

use crate::units::clamp_pan;

/// Unity-center sine balance. `-1` is full left, `0` leaves both channels
/// untouched, and `+1` is full right.
pub fn balance_gains(pan: f64) -> (f64, f64) {
    let pan = clamp_pan(pan);
    let (mut left, mut right) = if pan < 0.0 {
        (1.0, (-pan * FRAC_PI_2).cos())
    } else if pan > 0.0 {
        ((pan * FRAC_PI_2).cos(), 1.0)
    } else {
        (1.0, 1.0)
    };
    left = snap_unity(left);
    right = snap_unity(right);
    (left, right)
}

fn snap_unity(value: f64) -> f64 {
    if value.abs() < 1e-10 {
        0.0
    } else if (value - 1.0).abs() < 1e-10 {
        1.0
    } else {
        value
    }
}

pub fn apply_balance(interleaved: &mut [f32], channels: usize, pan: f64) {
    if channels < 2 || interleaved.len() < channels {
        return;
    }
    let (left, right) = balance_gains(pan);
    let left = left as f32;
    let right = right as f32;
    let frames = interleaved.len() / channels;
    for frame in 0..frames {
        let base = frame * channels;
        interleaved[base] *= left;
        interleaved[base + 1] *= right;
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_1_SQRT_2;

    use super::*;

    #[test]
    fn endpoints_and_half_pan() {
        assert_eq!(balance_gains(0.0), (1.0, 1.0));
        assert_eq!(balance_gains(-1.0), (1.0, 0.0));
        assert_eq!(balance_gains(1.0), (0.0, 1.0));
        let (left, right) = balance_gains(-0.5);
        assert!((left - 1.0).abs() < 1e-12);
        assert!((right - FRAC_1_SQRT_2).abs() < 1e-12);
    }

    #[test]
    fn hard_left_clears_the_right_channel() {
        let mut samples = [1.0, 1.0];
        apply_balance(&mut samples, 2, -1.0);
        assert_eq!(samples, [1.0, 0.0]);
    }
}
