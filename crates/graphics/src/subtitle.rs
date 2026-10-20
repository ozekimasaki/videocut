use videocut_model::TICKS_PER_SECOND;

use crate::error::GraphicsError;

/// One subtitle cue in sequence ticks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleCue {
    pub start_ticks: i64,
    pub end_ticks: i64,
    pub text: String,
}

#[derive(Clone, Copy)]
enum SubtitleFormat {
    Srt,
    Vtt,
}

struct Line {
    number: usize,
    text: String,
}

pub fn parse_srt(source: &str) -> Result<Vec<SubtitleCue>, GraphicsError> {
    parse_cues(source, SubtitleFormat::Srt)
}

pub fn parse_vtt(source: &str) -> Result<Vec<SubtitleCue>, GraphicsError> {
    parse_cues(source, SubtitleFormat::Vtt)
}

pub fn write_srt(cues: &[SubtitleCue]) -> String {
    write_cues(cues, SubtitleFormat::Srt)
}

pub fn write_vtt(cues: &[SubtitleCue]) -> String {
    write_cues(cues, SubtitleFormat::Vtt)
}

pub(crate) fn ticks_to_ms(ticks: i64) -> i64 {
    if ticks <= 0 {
        return 0;
    }
    let ticks = ticks as i128;
    let tps = TICKS_PER_SECOND as i128;
    let ms = (ticks * 1000 + tps / 2) / tps;
    i64::try_from(ms).unwrap_or(i64::MAX)
}

pub(crate) fn ms_to_ticks(ms: i64) -> i64 {
    if ms <= 0 {
        return 0;
    }
    let ticks = (ms as i128) * (TICKS_PER_SECOND as i128) / 1000;
    i64::try_from(ticks).unwrap_or(i64::MAX)
}

fn parse_cues(source: &str, format: SubtitleFormat) -> Result<Vec<SubtitleCue>, GraphicsError> {
    let blocks = split_blocks(source_lines(source));
    let mut blocks = blocks.into_iter();
    if matches!(format, SubtitleFormat::Vtt) {
        let header = blocks.next().ok_or_else(|| GraphicsError::Parse {
            line: 1,
            message: "VTT file must start with WEBVTT".into(),
        })?;
        let first = header.first().map(|line| line.text.trim()).unwrap_or("");
        let line = header.first().map(|line| line.number).unwrap_or(1);
        if !first.starts_with("WEBVTT") {
            return Err(GraphicsError::Parse {
                line,
                message: "VTT file must start with WEBVTT".into(),
            });
        }
        if header.iter().any(|line| line.text.contains("-->")) {
            return Err(GraphicsError::Parse {
                line,
                message: "expected a blank line after the WEBVTT header".into(),
            });
        }
    }

    let mut cues = Vec::new();
    for block in blocks {
        if is_metadata_block(&block) {
            continue;
        }
        cues.push(parse_cue_block(&block)?);
    }
    Ok(cues)
}

fn source_lines(source: &str) -> Vec<Line> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let source = source.replace("\r\n", "\n").replace('\r', "\n");
    source
        .lines()
        .enumerate()
        .map(|(index, text)| Line {
            number: index + 1,
            text: text.to_string(),
        })
        .collect()
}

fn split_blocks(lines: Vec<Line>) -> Vec<Vec<Line>> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    for line in lines {
        if line.text.trim().is_empty() {
            if !current.is_empty() {
                blocks.push(std::mem::take(&mut current));
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

fn is_metadata_block(block: &[Line]) -> bool {
    let first = block.first().map(|line| line.text.trim()).unwrap_or("");
    matches_keyword(first, "NOTE")
        || matches_keyword(first, "STYLE")
        || matches_keyword(first, "REGION")
}

fn matches_keyword(line: &str, keyword: &str) -> bool {
    line == keyword
        || line
            .strip_prefix(keyword)
            .is_some_and(|rest| rest.starts_with(' '))
}

fn parse_cue_block(block: &[Line]) -> Result<SubtitleCue, GraphicsError> {
    let timing_at = block.iter().position(|line| line.text.contains("-->"));
    let Some(timing_at) = timing_at else {
        let line = block.first().map(|line| line.number).unwrap_or(1);
        return Err(GraphicsError::Parse {
            line,
            message: "expected a cue timing line".into(),
        });
    };
    if timing_at > 1 {
        return Err(GraphicsError::Parse {
            line: block[0].number,
            message: "cue has more than one identifier line".into(),
        });
    }
    let (start_ticks, end_ticks) = parse_timing_line(&block[timing_at])?;
    let text = block[timing_at + 1..]
        .iter()
        .map(|line| line.text.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(SubtitleCue {
        start_ticks,
        end_ticks,
        text,
    })
}

fn parse_timing_line(line: &Line) -> Result<(i64, i64), GraphicsError> {
    let fail = |message: &str| GraphicsError::Parse {
        line: line.number,
        message: message.into(),
    };
    let Some((start_raw, end_raw)) = line.text.split_once("-->") else {
        return Err(fail("expected a cue timing line"));
    };
    let start_ticks = parse_timestamp(start_raw.trim(), line.number)?;
    let end_token = end_raw.split_whitespace().next().unwrap_or("");
    let end_ticks = parse_timestamp(end_token, line.number)?;
    if end_ticks <= start_ticks {
        return Err(fail("cue end must be after start"));
    }
    Ok((start_ticks, end_ticks))
}

fn parse_timestamp(raw: &str, line: usize) -> Result<i64, GraphicsError> {
    let fail = |message: &str| GraphicsError::Parse {
        line,
        message: message.into(),
    };
    let parts: Vec<&str> = raw.split(':').collect();
    let (hour_str, minute_str, second_str) = match parts.as_slice() {
        [hours, minutes, seconds] => (*hours, *minutes, *seconds),
        [minutes, seconds] => ("0", *minutes, *seconds),
        _ => return Err(fail("invalid timestamp")),
    };
    let hours: i64 = hour_str.parse().map_err(|_| fail("invalid timestamp"))?;
    let minutes: i64 = minute_str.parse().map_err(|_| fail("invalid timestamp"))?;
    if !(0..60).contains(&minutes) {
        return Err(fail("minutes out of range"));
    }
    let (second_str, fraction) = second_str
        .split_once(['.', ','])
        .ok_or_else(|| fail("timestamp needs fractional seconds"))?;
    let seconds: i64 = second_str.parse().map_err(|_| fail("invalid timestamp"))?;
    if !(0..60).contains(&seconds) {
        return Err(fail("seconds out of range"));
    }
    if hours < 0
        || fraction.is_empty()
        || fraction.len() > 3
        || !fraction.chars().all(|ch| ch.is_ascii_digit())
    {
        return Err(fail("invalid fractional seconds"));
    }
    let fraction_value: i64 = fraction
        .parse()
        .map_err(|_| fail("invalid fractional seconds"))?;
    let millis = match fraction.len() {
        1 => fraction_value * 100,
        2 => fraction_value * 10,
        3 => fraction_value,
        _ => return Err(fail("invalid fractional seconds")),
    };
    let total_ms = hours
        .saturating_mul(3_600_000)
        .saturating_add(minutes.saturating_mul(60_000))
        .saturating_add(seconds.saturating_mul(1_000))
        .saturating_add(millis);
    Ok(ms_to_ticks(total_ms))
}

fn write_cues(cues: &[SubtitleCue], format: SubtitleFormat) -> String {
    let separator = match format {
        SubtitleFormat::Srt => ',',
        SubtitleFormat::Vtt => '.',
    };
    let mut out = match format {
        SubtitleFormat::Srt => String::new(),
        SubtitleFormat::Vtt => "WEBVTT\n\n".to_string(),
    };
    for (index, cue) in cues.iter().enumerate() {
        let text = normalize_cue_text(&cue.text);
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            index + 1,
            format_timestamp(ticks_to_ms(cue.start_ticks), separator),
            format_timestamp(ticks_to_ms(cue.end_ticks), separator),
            text
        ));
    }
    out
}

fn format_timestamp(total_ms: i64, separator: char) -> String {
    let total_ms = total_ms.max(0);
    let hours = total_ms / 3_600_000;
    let minutes = (total_ms % 3_600_000) / 60_000;
    let seconds = (total_ms % 60_000) / 1_000;
    let millis = total_ms % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02}{separator}{millis:03}")
}

/// Drops blank lines so an exported cue stays a single block.
fn normalize_cue_text(text: &str) -> String {
    text.replace('\r', "")
        .split('\n')
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello_cues() -> Vec<SubtitleCue> {
        vec![
            SubtitleCue {
                start_ticks: ms_to_ticks(1_000),
                end_ticks: ms_to_ticks(4_000),
                text: "Hello world".into(),
            },
            SubtitleCue {
                start_ticks: ms_to_ticks(5_500),
                end_ticks: ms_to_ticks(7_000),
                text: "Second line\nthat wraps".into(),
            },
        ]
    }

    #[test]
    fn srt_round_trip() {
        let written = write_srt(&hello_cues());
        let expected = "\
1
00:00:01,000 --> 00:00:04,000
Hello world

2
00:00:05,500 --> 00:00:07,000
Second line
that wraps

";
        assert_eq!(written, expected);
        assert_eq!(parse_srt(&written).unwrap(), hello_cues());
    }

    #[test]
    fn vtt_round_trip() {
        let written = write_vtt(&hello_cues());
        let expected = "\
WEBVTT

1
00:00:01.000 --> 00:00:04.000
Hello world

2
00:00:05.500 --> 00:00:07.000
Second line
that wraps

";
        assert_eq!(written, expected);
        assert_eq!(parse_vtt(&written).unwrap(), hello_cues());
    }

    #[test]
    fn vtt_skips_header_metadata_note_style_and_region() {
        let source = "\
WEBVTT
Kind: captions
Language: en

NOTE
commentator

NOTE same line

STYLE
::cue { color: white; }

REGION
id:top

00:00:01.000 --> 00:00:02.000 align:start position:10%
Visible

";
        let cues = parse_vtt(source).unwrap();
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "Visible");
        assert_eq!(cues[0].start_ticks, ms_to_ticks(1_000));
        assert_eq!(cues[0].end_ticks, ms_to_ticks(2_000));
    }

    #[test]
    fn vtt_accepts_short_timestamps_and_fraction_widths() {
        let source = "\
WEBVTT

01:02.500 --> 01:03.000
Short

00:00:01.5 --> 00:00:02.0
Fraction

";
        let cues = parse_vtt(source).unwrap();
        assert_eq!(cues[0].start_ticks, ms_to_ticks(62_500));
        assert_eq!(cues[0].end_ticks, ms_to_ticks(63_000));
        assert_eq!(cues[1].start_ticks, ms_to_ticks(1_500));
        assert_eq!(cues[1].end_ticks, ms_to_ticks(2_000));
    }

    #[test]
    fn srt_accepts_dot_separator_optional_index_bom_and_crlf() {
        let source = "\u{feff}1\r\n00:00:01.000 --> 00:00:02.000\r\nHello\r\n\r\n00:00:03,000 --> 00:00:04,000\r\nNext\r\n";
        let cues = parse_srt(source).unwrap();
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].text, "Hello");
        assert_eq!(cues[1].text, "Next");
        assert_eq!(cues[1].start_ticks, ms_to_ticks(3_000));
    }

    #[test]
    fn malformed_timestamps_report_the_line() {
        let err = parse_srt("1\n00:61:00,000 --> 00:62:00,000\nNope\n").unwrap_err();
        assert_eq!(
            err,
            GraphicsError::Parse {
                line: 2,
                message: "minutes out of range".into(),
            }
        );
        let err = parse_srt("1\n00:00:02,000 --> 00:00:01,000\nBackwards\n").unwrap_err();
        assert!(matches!(
            err,
            GraphicsError::Parse {
                line: 2,
                message,
            } if message == "cue end must be after start"
        ));
        let err = parse_vtt("not a header\n").unwrap_err();
        assert!(matches!(err, GraphicsError::Parse { line: 1, .. }));
        let err = parse_vtt("WEBVTT\n00:00:01.000 --> 00:00:02.000\nHi\n").unwrap_err();
        assert!(matches!(
            err,
            GraphicsError::Parse { message, .. } if message.contains("blank line")
        ));
    }

    #[test]
    fn export_collapses_blank_lines_inside_a_cue() {
        let cues = vec![SubtitleCue {
            start_ticks: ms_to_ticks(1_000),
            end_ticks: ms_to_ticks(2_000),
            text: "a\n\nb".into(),
        }];
        let written = write_srt(&cues);
        assert!(written.contains("a\nb\n"));
        assert_eq!(parse_srt(&written).unwrap()[0].text, "a\nb");
    }

    #[test]
    fn empty_files_have_no_cues() {
        assert!(parse_srt("").unwrap().is_empty());
        assert!(parse_vtt("WEBVTT\n\n").unwrap().is_empty());
        assert_eq!(write_srt(&[]), "");
        assert_eq!(write_vtt(&[]), "WEBVTT\n\n");
    }

    #[test]
    fn millisecond_rounding_matches_half_up() {
        let half_ms = TICKS_PER_SECOND / 1000 / 2;
        assert_eq!(ticks_to_ms(half_ms), 1);
        assert_eq!(ticks_to_ms(half_ms - 1), 0);
        assert_eq!(ms_to_ticks(1_500), TICKS_PER_SECOND * 3 / 2);
    }
}
