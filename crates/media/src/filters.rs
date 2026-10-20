use videocut_model::Rational;

use crate::error::MediaError;
use crate::spans::{ClipSlice, InputTable, Span, SpanBody};
use crate::time::{format_rational, format_ticks_as_seconds};

#[derive(Debug, Clone)]
pub(crate) struct PictureFormat {
    pub width: u32,
    pub height: u32,
    pub fps: Rational,
    pub pixel_format: String,
    pub frame_ticks: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct SoundFormat {
    pub sample_rate: u32,
    pub layout: String,
    pub channels: u16,
}

pub(crate) fn channel_layout(channels: u16) -> (&'static str, u16, bool) {
    match channels {
        1 => ("mono", 1, false),
        2 => ("stereo", 2, false),
        6 => ("5.1", 6, false),
        _ => ("stereo", 2, true),
    }
}

struct Labels {
    next: u32,
    prefix: &'static str,
}

impl Labels {
    fn video() -> Self {
        Self {
            next: 0,
            prefix: "v",
        }
    }

    fn allocate(&mut self) -> String {
        let label = format!("{}{}", self.prefix, self.next);
        self.next += 1;
        label
    }
}

struct Render<'a> {
    video: bool,
    picture: Option<&'a PictureFormat>,
    sound: Option<&'a SoundFormat>,
    labels: &'a mut Labels,
    inputs: &'a mut InputTable,
    warnings: &'a mut Vec<String>,
}

pub(crate) fn render_picture(
    spans: &[Span],
    picture: &PictureFormat,
    inputs: &mut InputTable,
    warnings: &mut Vec<String>,
) -> Result<Vec<String>, MediaError> {
    let mut labels = Labels::video();
    Render {
        video: true,
        picture: Some(picture),
        sound: None,
        labels: &mut labels,
        inputs,
        warnings,
    }
    .list(spans, "vout")
}

pub(crate) fn render_sound(
    spans: &[Span],
    sound: &SoundFormat,
    final_label: &str,
    labels_already: &mut u32,
    inputs: &mut InputTable,
    warnings: &mut Vec<String>,
) -> Result<Vec<String>, MediaError> {
    let mut labels = Labels {
        next: *labels_already,
        prefix: "a",
    };
    let filters = Render {
        video: false,
        picture: None,
        sound: Some(sound),
        labels: &mut labels,
        inputs,
        warnings,
    }
    .list(spans, final_label)?;
    *labels_already = labels.next;
    Ok(filters)
}

pub(crate) fn amix(labels: &[String]) -> String {
    let heads: String = labels.iter().map(|label| format!("[{label}]")).collect();
    format!(
        "{heads}amix=inputs={}:duration=longest:dropout_transition=0:normalize=0[aout]",
        labels.len()
    )
}

impl Render<'_> {
    fn list(&mut self, spans: &[Span], final_label: &str) -> Result<Vec<String>, MediaError> {
        if spans.is_empty() {
            return Ok(Vec::new());
        }
        if spans.len() == 1 {
            return self.span(&spans[0], final_label);
        }
        let mut filters = Vec::new();
        let mut outputs = Vec::new();
        for span in spans {
            let label = self.labels.allocate();
            filters.extend(self.span(span, &label)?);
            outputs.push(label);
        }
        filters.push(concat(&outputs, self.video, final_label));
        Ok(filters)
    }

    fn span(&mut self, span: &Span, out: &str) -> Result<Vec<String>, MediaError> {
        let duration = span.timeline_out.saturating_sub(span.timeline_in).max(1);
        match &span.body {
            SpanBody::Gap => Ok(vec![self.gap(duration, out)?]),
            SpanBody::Clip(slice) => Ok(vec![self.clip(slice, duration, out)?]),
            SpanBody::Dissolve { left, right } => {
                let left_label = self.labels.allocate();
                let right_label = self.labels.allocate();
                let mut filters = vec![
                    self.clip(left, duration, &left_label)?,
                    self.clip(right, duration, &right_label)?,
                ];
                filters.push(dissolve_filter(
                    &left_label,
                    &right_label,
                    duration,
                    self.video,
                    out,
                ));
                Ok(filters)
            }
        }
    }

    fn gap(&self, duration: i64, out: &str) -> Result<String, MediaError> {
        let seconds = format_ticks_as_seconds(duration);
        if self.video {
            let picture = self
                .picture
                .ok_or_else(|| MediaError::Invalid("missing picture format".into()))?;
            let fps = format_rational(picture.fps)?;
            Ok(format!(
                "color=c=black:size={}x{}:rate={fps}:duration={seconds},format={},setsar=1[{out}]",
                picture.width, picture.height, picture.pixel_format
            ))
        } else {
            let sound = self
                .sound
                .ok_or_else(|| MediaError::Invalid("missing audio format".into()))?;
            Ok(format!(
                "anullsrc=channel_layout={layout}:sample_rate={rate},atrim=duration={seconds},asetpts=PTS-STARTPTS,aformat=sample_fmts=fltp:sample_rates={rate}:channel_layouts={layout}[{out}]",
                layout = sound.layout,
                rate = sound.sample_rate
            ))
        }
    }

    fn clip(&mut self, slice: &ClipSlice, duration: i64, out: &str) -> Result<String, MediaError> {
        if self.video {
            let picture = self
                .picture
                .ok_or_else(|| MediaError::Invalid("missing picture format".into()))?;
            if slice.still {
                note_still(
                    self.inputs,
                    slice.input_index,
                    duration,
                    picture.frame_ticks,
                );
                return still_picture(slice, duration, picture, out);
            }
            if !slice.has_picture {
                warn_once(
                    self.warnings,
                    format!(
                        "media input {} has no picture and is exported as black",
                        slice.input_index
                    ),
                );
                return self.gap(duration, out);
            }
            return timed_picture(slice, picture, out);
        }
        let sound = self
            .sound
            .ok_or_else(|| MediaError::Invalid("missing audio format".into()))?;
        if slice.still || !slice.has_sound {
            warn_once(
                self.warnings,
                format!(
                    "media input {} has no audio and is exported as silence",
                    slice.input_index
                ),
            );
            return self.gap(duration, out);
        }
        timed_sound(slice, sound, out)
    }
}

fn note_still(inputs: &mut InputTable, index: usize, duration: i64, frame_ticks: i64) {
    let Some(input) = inputs.inputs.get_mut(index) else {
        return;
    };
    let needed = duration.saturating_add(frame_ticks.max(1));
    if needed > input.still_duration_ticks {
        input.still_duration_ticks = needed;
    }
}

fn still_picture(
    slice: &ClipSlice,
    duration: i64,
    picture: &PictureFormat,
    out: &str,
) -> Result<String, MediaError> {
    let fps = format_rational(picture.fps)?;
    Ok(format!(
        "[{}:v]trim=duration={},setpts=PTS-STARTPTS,{}[{out}]",
        slice.input_index,
        format_ticks_as_seconds(duration),
        picture_tail(picture, &fps)
    ))
}

fn timed_picture(
    slice: &ClipSlice,
    picture: &PictureFormat,
    out: &str,
) -> Result<String, MediaError> {
    let fps = format_rational(picture.fps)?;
    Ok(format!(
        "[{}:v]trim=start={}:end={},{},{}[{out}]",
        slice.input_index,
        format_ticks_as_seconds(slice.source_in),
        format_ticks_as_seconds(slice.source_out),
        video_time_filters(slice.speed, slice.reversed),
        picture_tail(picture, &fps)
    ))
}

fn picture_tail(picture: &PictureFormat, fps: &str) -> String {
    format!(
        "scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:black,fps={fps},setsar=1,format={pix}",
        w = picture.width,
        h = picture.height,
        pix = picture.pixel_format
    )
}

fn timed_sound(slice: &ClipSlice, sound: &SoundFormat, out: &str) -> Result<String, MediaError> {
    Ok(format!(
        "[{}:a]atrim=start={}:end={},{},aformat=sample_fmts=fltp:sample_rates={}:channel_layouts={}[{out}]",
        slice.input_index,
        format_ticks_as_seconds(slice.source_in),
        format_ticks_as_seconds(slice.source_out),
        audio_time_filters(slice.speed, slice.reversed)?,
        sound.sample_rate,
        sound.layout
    ))
}

fn video_time_filters(speed: f64, reversed: bool) -> String {
    let unity = (speed - 1.0).abs() < 1e-6;
    if unity && !reversed {
        return "setpts=PTS-STARTPTS".to_string();
    }
    if reversed && unity {
        return "setpts=PTS-STARTPTS,reverse".to_string();
    }
    if !reversed {
        return format!("setpts=(PTS-STARTPTS)/{}", format_factor(speed));
    }
    format!(
        "setpts=PTS-STARTPTS,reverse,setpts=PTS/{}",
        format_factor(speed)
    )
}

fn audio_time_filters(speed: f64, reversed: bool) -> Result<String, MediaError> {
    let mut parts = vec!["asetpts=PTS-STARTPTS".to_string()];
    if reversed {
        parts.push("areverse".to_string());
    }
    parts.extend(atempo_filters(speed)?);
    Ok(parts.join(","))
}

fn atempo_filters(speed: f64) -> Result<Vec<String>, MediaError> {
    if !speed.is_finite() || speed <= 0.0 {
        return Err(MediaError::Invalid("speed must be positive".into()));
    }
    if (speed - 1.0).abs() < 1e-6 {
        return Ok(Vec::new());
    }
    let mut factors = Vec::new();
    let mut remaining = speed;
    let mut guard = 0;
    while remaining > 2.0 + 1e-6 {
        factors.push("atempo=2".to_string());
        remaining /= 2.0;
        guard += 1;
        if guard > 16 {
            return Err(MediaError::Invalid("speed is too extreme to export".into()));
        }
    }
    while remaining < 0.5 - 1e-6 {
        factors.push("atempo=0.5".to_string());
        remaining /= 0.5;
        guard += 1;
        if guard > 16 {
            return Err(MediaError::Invalid("speed is too extreme to export".into()));
        }
    }
    if (remaining - 1.0).abs() > 1e-4 {
        factors.push(format!("atempo={}", format_factor(remaining)));
    }
    Ok(factors)
}

fn dissolve_filter(left: &str, right: &str, duration: i64, video: bool, out: &str) -> String {
    let seconds = format_ticks_as_seconds(duration);
    if video {
        format!("[{left}][{right}]xfade=transition=fade:duration={seconds}:offset=0[{out}]")
    } else {
        format!("[{left}][{right}]acrossfade=d={seconds}[{out}]")
    }
}

fn concat(labels: &[String], video: bool, out: &str) -> String {
    let heads: String = labels.iter().map(|label| format!("[{label}]")).collect();
    if video {
        format!("{heads}concat=n={}:v=1:a=0[{out}]", labels.len())
    } else {
        format!("{heads}concat=n={}:v=0:a=1[{out}]", labels.len())
    }
}

fn format_factor(value: f64) -> String {
    let rounded = (value * 1_000_000.0).round() / 1_000_000.0;
    let text = format!("{rounded:.6}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

fn warn_once(warnings: &mut Vec<String>, message: String) {
    if !warnings.iter().any(|existing| existing == &message) {
        warnings.push(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atempo_splits_rates_outside_the_native_window() {
        assert!(atempo_filters(1.0).unwrap().is_empty());
        assert_eq!(atempo_filters(2.0).unwrap(), vec!["atempo=2".to_string()]);
        assert_eq!(
            atempo_filters(4.0).unwrap(),
            vec!["atempo=2".to_string(), "atempo=2".to_string()]
        );
        assert_eq!(
            atempo_filters(0.25).unwrap(),
            vec!["atempo=0.5".to_string(), "atempo=0.5".to_string()]
        );
    }
}
