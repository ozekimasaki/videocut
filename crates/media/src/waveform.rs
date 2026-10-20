use std::io::Read;

use videocut_model::MediaRef;

use crate::error::MediaError;
use crate::proxy::{prelude, FfmpegPlan};
use crate::sidecar::FfmpegSidecar;

pub const DEFAULT_WAVEFORM_RATE: u32 = 8_000;
pub const DEFAULT_SAMPLES_PER_PEAK: u32 = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveformOptions {
    /// Decode rate before peak bucketing. Lower rates make long files cheap.
    pub sample_rate: u32,
    pub samples_per_peak: u32,
}

impl Default for WaveformOptions {
    fn default() -> Self {
        Self {
            sample_rate: DEFAULT_WAVEFORM_RATE,
            samples_per_peak: DEFAULT_SAMPLES_PER_PEAK,
        }
    }
}

/// Mono min/max pairs in `-1.0..=1.0`, suitable for a timeline waveform.
#[derive(Debug, Clone, PartialEq)]
pub struct WaveformPeaks {
    /// Channel count of the source, before the mono downmix used for peaks.
    pub source_channels: u16,
    pub sample_rate: u32,
    pub samples_per_peak: u32,
    /// Interleaved minimum and maximum for each bucket.
    pub min_max: Vec<f32>,
}

pub fn plan_waveform(
    media: &MediaRef,
    options: &WaveformOptions,
) -> Result<FfmpegPlan, MediaError> {
    let audio = media
        .audio
        .as_ref()
        .ok_or_else(|| MediaError::NoAudioStream(media.name.clone()))?;
    if options.sample_rate == 0 || options.samples_per_peak == 0 {
        return Err(MediaError::Invalid(
            "waveform sample rate and samples per peak must be non-zero".into(),
        ));
    }
    let _ = audio;
    let mut args = prelude();
    args.extend([
        "-i".into(),
        media.path.clone(),
        "-vn".into(),
        "-ac".into(),
        "1".into(),
        "-ar".into(),
        options.sample_rate.to_string(),
        "-f".into(),
        "f32le".into(),
        "-acodec".into(),
        "pcm_f32le".into(),
        "pipe:1".into(),
    ]);
    Ok(FfmpegPlan { args })
}

pub fn waveform_peaks(
    sidecar: &FfmpegSidecar,
    media: &MediaRef,
    options: &WaveformOptions,
) -> Result<WaveformPeaks, MediaError> {
    let plan = plan_waveform(media, options)?;
    let pcm = sidecar.run_capture(&plan.args)?;
    let min_max = peaks_from_f32le(&pcm, options.samples_per_peak)?;
    Ok(WaveformPeaks {
        source_channels: media
            .audio
            .as_ref()
            .map(|audio| audio.channels)
            .unwrap_or(1),
        sample_rate: options.sample_rate,
        samples_per_peak: options.samples_per_peak,
        min_max,
    })
}

/// Peak buckets from mono `f32` samples in `-1.0..=1.0`.
pub fn waveform_from_mono_samples(
    samples: &[f32],
    sample_rate: u32,
    samples_per_peak: u32,
    source_channels: u16,
) -> Result<WaveformPeaks, MediaError> {
    if samples_per_peak == 0 || sample_rate == 0 {
        return Err(MediaError::Invalid(
            "waveform sample rate and samples per peak must be non-zero".into(),
        ));
    }
    Ok(WaveformPeaks {
        source_channels,
        sample_rate,
        samples_per_peak,
        min_max: bucket_peaks(samples, samples_per_peak),
    })
}

pub fn peaks_from_f32le(bytes: &[u8], samples_per_peak: u32) -> Result<Vec<f32>, MediaError> {
    peaks_from_reader(bytes, samples_per_peak)
}

pub fn peaks_from_reader<R: Read>(
    mut reader: R,
    samples_per_peak: u32,
) -> Result<Vec<f32>, MediaError> {
    if samples_per_peak == 0 {
        return Err(MediaError::Invalid(
            "samples per peak must be non-zero".into(),
        ));
    }
    let mut peaks = Vec::new();
    let mut pending = Vec::new();
    let mut chunk = [0_u8; 16 * 1024];
    let mut bucket_min = 0.0_f32;
    let mut bucket_max = 0.0_f32;
    let mut bucket_count: u32 = 0;
    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        pending.extend_from_slice(&chunk[..read]);
        let usable = pending.len() - (pending.len() % 4);
        let (samples, _) = pending[..usable].as_chunks::<4>();
        for bytes in samples {
            let sample = f32::from_le_bytes(*bytes);
            let sample = if sample.is_finite() {
                sample.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            if bucket_count == 0 {
                bucket_min = sample;
                bucket_max = sample;
            } else {
                bucket_min = bucket_min.min(sample);
                bucket_max = bucket_max.max(sample);
            }
            bucket_count += 1;
            if bucket_count == samples_per_peak {
                peaks.push(bucket_min);
                peaks.push(bucket_max);
                bucket_count = 0;
            }
        }
        pending.drain(..usable);
    }
    if bucket_count > 0 {
        peaks.push(bucket_min);
        peaks.push(bucket_max);
    }
    Ok(peaks)
}

fn bucket_peaks(samples: &[f32], samples_per_peak: u32) -> Vec<f32> {
    let mut bytes = Vec::with_capacity(samples.len() * 4);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    peaks_from_f32le(&bytes, samples_per_peak).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaks_are_min_max_pairs() {
        let peaks = waveform_from_mono_samples(&[0.0, 0.5, -0.25, 1.0], 8_000, 2, 2).unwrap();
        assert_eq!(peaks.min_max, vec![0.0, 0.5, -0.25, 1.0]);
        assert_eq!(peaks.source_channels, 2);
    }

    #[test]
    fn partial_bucket_is_kept() {
        let peaks = waveform_from_mono_samples(&[0.25, -0.5, 0.75], 8_000, 2, 1).unwrap();
        assert_eq!(peaks.min_max, vec![-0.5, 0.25, 0.75, 0.75]);
    }

    #[test]
    fn streaming_reader_matches_the_slice_helper() {
        let samples = [0.1_f32, -0.2, 0.3, -0.4, 0.5];
        let from_slice = waveform_from_mono_samples(&samples, 8_000, 2, 1).unwrap();
        let mut bytes = Vec::new();
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        let streamed = peaks_from_reader(&bytes[..], 2).unwrap();
        assert_eq!(from_slice.min_max, streamed);
    }
}
