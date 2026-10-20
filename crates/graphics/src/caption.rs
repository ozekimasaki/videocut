use videocut_model::{new_id, Caption, Sequence};

use crate::error::GraphicsError;
use crate::subtitle::{self, SubtitleCue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    Replace,
    Append,
}

pub fn add_caption(
    sequence: &mut Sequence,
    start_ticks: i64,
    end_ticks: i64,
    text: impl Into<String>,
) -> Result<String, GraphicsError> {
    validate_range(start_ticks, end_ticks)?;
    let caption = Caption {
        id: new_id("cap"),
        start_ticks,
        end_ticks,
        text: text.into(),
    };
    let id = caption.id.clone();
    sequence.captions.push(caption);
    Ok(id)
}

pub fn set_caption_text(
    sequence: &mut Sequence,
    id: &str,
    text: impl Into<String>,
) -> Result<(), GraphicsError> {
    let caption = find_mut(sequence, id)?;
    caption.text = text.into();
    Ok(())
}

pub fn set_caption_timing(
    sequence: &mut Sequence,
    id: &str,
    start_ticks: i64,
    end_ticks: i64,
) -> Result<(), GraphicsError> {
    validate_range(start_ticks, end_ticks)?;
    let caption = find_mut(sequence, id)?;
    caption.start_ticks = start_ticks;
    caption.end_ticks = end_ticks;
    Ok(())
}

pub fn remove_caption(sequence: &mut Sequence, id: &str) -> Result<Caption, GraphicsError> {
    let index = sequence
        .captions
        .iter()
        .position(|caption| caption.id == id)
        .ok_or_else(|| GraphicsError::CaptionNotFound(id.to_string()))?;
    Ok(sequence.captions.remove(index))
}

/// Captions active at `tick`, including the start and excluding the end.
pub fn captions_at(sequence: &Sequence, tick: i64) -> Vec<&Caption> {
    sequence
        .captions
        .iter()
        .filter(|caption| caption.start_ticks <= tick && tick < caption.end_ticks)
        .collect()
}

pub fn import_srt(
    sequence: &mut Sequence,
    source: &str,
    mode: ImportMode,
) -> Result<usize, GraphicsError> {
    let cues = subtitle::parse_srt(source)?;
    apply_cues(sequence, &cues, mode)
}

pub fn import_vtt(
    sequence: &mut Sequence,
    source: &str,
    mode: ImportMode,
) -> Result<usize, GraphicsError> {
    let cues = subtitle::parse_vtt(source)?;
    apply_cues(sequence, &cues, mode)
}

pub fn export_srt(sequence: &Sequence) -> Result<String, GraphicsError> {
    Ok(subtitle::write_srt(&cues_from_sequence(sequence)?))
}

pub fn export_vtt(sequence: &Sequence) -> Result<String, GraphicsError> {
    Ok(subtitle::write_vtt(&cues_from_sequence(sequence)?))
}

pub(crate) fn apply_cues(
    sequence: &mut Sequence,
    cues: &[SubtitleCue],
    mode: ImportMode,
) -> Result<usize, GraphicsError> {
    let captions = cues
        .iter()
        .map(|cue| {
            validate_range(cue.start_ticks, cue.end_ticks)?;
            Ok(Caption {
                id: new_id("cap"),
                start_ticks: cue.start_ticks,
                end_ticks: cue.end_ticks,
                text: cue.text.clone(),
            })
        })
        .collect::<Result<Vec<_>, GraphicsError>>()?;
    let count = captions.len();
    match mode {
        ImportMode::Replace => sequence.captions = captions,
        ImportMode::Append => sequence.captions.extend(captions),
    }
    Ok(count)
}

fn cues_from_sequence(sequence: &Sequence) -> Result<Vec<SubtitleCue>, GraphicsError> {
    let mut captions = sequence.captions.clone();
    captions.sort_by(|left, right| {
        left.start_ticks
            .cmp(&right.start_ticks)
            .then(left.end_ticks.cmp(&right.end_ticks))
            .then(left.id.cmp(&right.id))
    });
    captions
        .into_iter()
        .map(|caption| {
            validate_range(caption.start_ticks, caption.end_ticks)?;
            Ok(SubtitleCue {
                start_ticks: caption.start_ticks,
                end_ticks: caption.end_ticks,
                text: caption.text,
            })
        })
        .collect()
}

fn validate_range(start_ticks: i64, end_ticks: i64) -> Result<(), GraphicsError> {
    if start_ticks < 0 || end_ticks <= start_ticks {
        return Err(GraphicsError::InvalidRange);
    }
    Ok(())
}

fn find_mut<'a>(sequence: &'a mut Sequence, id: &str) -> Result<&'a mut Caption, GraphicsError> {
    sequence
        .captions
        .iter_mut()
        .find(|caption| caption.id == id)
        .ok_or_else(|| GraphicsError::CaptionNotFound(id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use videocut_model::{SequenceSettings, TICKS_PER_SECOND};

    fn empty_sequence() -> Sequence {
        Sequence::new("seq", SequenceSettings::hd1080p30())
    }

    #[test]
    fn caption_edit_and_active_range() {
        let mut sequence = empty_sequence();
        let id = add_caption(&mut sequence, 0, TICKS_PER_SECOND, "Hello").unwrap();
        assert_eq!(captions_at(&sequence, 0).len(), 1);
        assert!(captions_at(&sequence, TICKS_PER_SECOND).is_empty());
        set_caption_text(&mut sequence, &id, "Updated").unwrap();
        set_caption_timing(&mut sequence, &id, 10, 20).unwrap();
        assert_eq!(sequence.captions[0].text, "Updated");
        assert_eq!(sequence.captions[0].start_ticks, 10);
        assert_eq!(
            set_caption_timing(&mut sequence, &id, 20, 20).unwrap_err(),
            GraphicsError::InvalidRange
        );
        assert_eq!(sequence.captions[0].start_ticks, 10);
        assert_eq!(
            set_caption_text(&mut sequence, "missing", "x").unwrap_err(),
            GraphicsError::CaptionNotFound("missing".into())
        );
        let removed = remove_caption(&mut sequence, &id).unwrap();
        assert_eq!(removed.text, "Updated");
        assert!(sequence.captions.is_empty());
    }

    #[test]
    fn srt_and_vtt_import_export_round_trip() {
        let mut sequence = empty_sequence();
        add_caption(
            &mut sequence,
            ms(5_500),
            ms(7_000),
            "Second line\nthat wraps",
        )
        .unwrap();
        add_caption(&mut sequence, ms(1_000), ms(4_000), "Hello world").unwrap();
        let srt = export_srt(&sequence).unwrap();
        assert!(srt.starts_with("1\n00:00:01,000 --> 00:00:04,000\nHello world\n"));
        let vtt = export_vtt(&sequence).unwrap();
        assert!(vtt.starts_with("WEBVTT\n"));
        assert!(vtt.contains("00:00:01.000 --> 00:00:04.000"));

        let mut restored = empty_sequence();
        let count = import_srt(&mut restored, &srt, ImportMode::Replace).unwrap();
        assert_eq!(count, 2);
        assert_eq!(restored.captions[0].text, "Hello world");
        assert_eq!(restored.captions[0].start_ticks, ms(1_000));
        assert_eq!(restored.captions[1].text, "Second line\nthat wraps");
        assert_eq!(export_srt(&restored).unwrap(), srt);

        let vtt_count = import_vtt(&mut restored, &vtt, ImportMode::Replace).unwrap();
        assert_eq!(vtt_count, 2);
        assert_eq!(export_vtt(&restored).unwrap(), vtt);
    }

    #[test]
    fn replace_and_append_and_failed_import_keep_existing_captions() {
        let mut sequence = empty_sequence();
        add_caption(&mut sequence, ms(1_000), ms(2_000), "Keep").unwrap();
        let err = import_vtt(&mut sequence, "not vtt", ImportMode::Replace).unwrap_err();
        assert!(matches!(err, GraphicsError::Parse { .. }));
        assert_eq!(sequence.captions.len(), 1);
        assert_eq!(sequence.captions[0].text, "Keep");

        let extra = "1\n00:00:03,000 --> 00:00:04,000\nAdded\n\n";
        import_srt(&mut sequence, extra, ImportMode::Append).unwrap();
        assert_eq!(sequence.captions.len(), 2);
        import_srt(&mut sequence, "", ImportMode::Replace).unwrap();
        assert!(sequence.captions.is_empty());
        import_vtt(&mut sequence, "WEBVTT\n\n", ImportMode::Replace).unwrap();
        assert!(sequence.captions.is_empty());
    }

    #[test]
    fn export_rejects_an_inverted_caption_without_writing() {
        let mut sequence = empty_sequence();
        sequence.captions.push(Caption {
            id: "cap-bad".into(),
            start_ticks: 20,
            end_ticks: 10,
            text: "bad".into(),
        });
        assert_eq!(
            export_srt(&sequence).unwrap_err(),
            GraphicsError::InvalidRange
        );
    }

    #[test]
    fn millisecond_quantization_survives_export() {
        let mut sequence = empty_sequence();
        add_caption(&mut sequence, ms(1_500) + 10, ms(2_000), "Near").unwrap();
        let srt = export_srt(&sequence).unwrap();
        assert!(srt.contains("00:00:01,500 --> 00:00:02,000"));
        import_srt(&mut sequence, &srt, ImportMode::Replace).unwrap();
        assert_eq!(sequence.captions[0].start_ticks, ms(1_500));
    }

    fn ms(value: i64) -> i64 {
        value * TICKS_PER_SECOND / 1000
    }
}
