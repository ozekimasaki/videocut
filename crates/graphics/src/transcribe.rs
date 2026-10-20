use std::path::{Path, PathBuf};
use std::process::Command;

use videocut_model::Sequence;

use crate::caption::{self, ImportMode};
use crate::error::GraphicsError;
use crate::subtitle::{self, SubtitleCue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptSegment {
    pub start_ticks: i64,
    pub end_ticks: i64,
    pub text: String,
}

/// Turns a media file into timed text.
///
/// Implementations must not fetch model weights. The caller provides whatever
/// program and model files the adapter needs.
pub trait Transcriber {
    fn transcribe(&self, media_path: &Path) -> Result<Vec<TranscriptSegment>, GraphicsError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptFormat {
    Srt,
    Vtt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

pub trait CommandRunner {
    fn run(&self, program: &Path, args: &[String]) -> Result<CommandOutput, GraphicsError>;
}

/// Spawns `program` directly. Arguments are not passed through a shell.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessCommandRunner;

impl<R: CommandRunner + ?Sized> CommandRunner for &R {
    fn run(&self, program: &Path, args: &[String]) -> Result<CommandOutput, GraphicsError> {
        (**self).run(program, args)
    }
}

impl CommandRunner for ProcessCommandRunner {
    fn run(&self, program: &Path, args: &[String]) -> Result<CommandOutput, GraphicsError> {
        let output = Command::new(program).args(args).output().map_err(|err| {
            GraphicsError::Transcription(format!("failed to start {}: {err}", program.display()))
        })?;
        let stdout = String::from_utf8(output.stdout)
            .map_err(|_| GraphicsError::Transcription("command stdout was not UTF-8".into()))?;
        let stderr = String::from_utf8(output.stderr)
            .map_err(|_| GraphicsError::Transcription("command stderr was not UTF-8".into()))?;
        let status = output.status.code().unwrap_or(1);
        Ok(CommandOutput {
            status,
            stdout,
            stderr,
        })
    }
}

/// Runs an external transcription command and parses SRT or VTT from stdout.
///
/// The caller chooses the program and its arguments, including any model file
/// that program needs. `{input}` in an argument is replaced with the media
/// path. When no argument contains `{input}`, the media path is appended.
pub struct CommandLineTranscriber<R> {
    program: PathBuf,
    args: Vec<String>,
    format: TranscriptFormat,
    runner: R,
}

impl<R> CommandLineTranscriber<R> {
    pub fn new(
        program: impl Into<PathBuf>,
        args: impl Into<Vec<String>>,
        format: TranscriptFormat,
        runner: R,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into(),
            format,
            runner,
        }
    }
}

impl<R: CommandRunner> Transcriber for CommandLineTranscriber<R> {
    fn transcribe(&self, media_path: &Path) -> Result<Vec<TranscriptSegment>, GraphicsError> {
        let input = path_to_string(media_path);
        let mut replaced = false;
        let mut args = Vec::with_capacity(self.args.len() + 1);
        for arg in &self.args {
            if arg.contains("{input}") {
                replaced = true;
                args.push(arg.replace("{input}", &input));
            } else {
                args.push(arg.clone());
            }
        }
        if !replaced {
            args.push(input);
        }
        let output = self.runner.run(&self.program, &args)?;
        if output.status != 0 {
            let detail = if output.stderr.trim().is_empty() {
                format!("exit status {}", output.status)
            } else {
                output.stderr.trim().to_string()
            };
            return Err(GraphicsError::Transcription(detail));
        }
        let cues = match self.format {
            TranscriptFormat::Srt => subtitle::parse_srt(&output.stdout)?,
            TranscriptFormat::Vtt => subtitle::parse_vtt(&output.stdout)?,
        };
        Ok(cues.into_iter().map(segment_from_cue).collect())
    }
}

pub fn apply_transcript(
    sequence: &mut Sequence,
    segments: &[TranscriptSegment],
    mode: ImportMode,
) -> Result<usize, GraphicsError> {
    let mut cues = Vec::with_capacity(segments.len());
    for segment in segments {
        if segment.start_ticks < 0 || segment.end_ticks <= segment.start_ticks {
            return Err(GraphicsError::InvalidRange);
        }
        cues.push(SubtitleCue {
            start_ticks: segment.start_ticks,
            end_ticks: segment.end_ticks,
            text: segment.text.clone(),
        });
    }
    caption::apply_cues(sequence, &cues, mode)
}

fn segment_from_cue(cue: SubtitleCue) -> TranscriptSegment {
    TranscriptSegment {
        start_ticks: cue.start_ticks,
        end_ticks: cue.end_ticks,
        text: cue.text,
    }
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use videocut_model::{SequenceSettings, TICKS_PER_SECOND};

    struct ScriptedRunner {
        status: i32,
        stdout: String,
        stderr: String,
        calls: RefCell<Vec<(PathBuf, Vec<String>)>>,
    }

    impl CommandRunner for ScriptedRunner {
        fn run(&self, program: &Path, args: &[String]) -> Result<CommandOutput, GraphicsError> {
            self.calls
                .borrow_mut()
                .push((program.to_path_buf(), args.to_vec()));
            Ok(CommandOutput {
                status: self.status,
                stdout: self.stdout.clone(),
                stderr: self.stderr.clone(),
            })
        }
    }

    #[test]
    fn command_adapter_parses_stdout_and_substitutes_the_media_path() {
        let runner = ScriptedRunner {
            status: 0,
            stdout: "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n".into(),
            stderr: String::new(),
            calls: RefCell::new(Vec::new()),
        };
        let transcriber = CommandLineTranscriber::new(
            "transcribe-bin",
            vec![
                "--model".into(),
                "/opt/models/ggml.bin".into(),
                "{input}".into(),
            ],
            TranscriptFormat::Srt,
            &runner,
        );
        let as_trait: &dyn Transcriber = &transcriber;
        let segments = as_trait
            .transcribe(Path::new("/media/interview.wav"))
            .unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Hello");
        assert_eq!(segments[0].start_ticks, TICKS_PER_SECOND);
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, PathBuf::from("transcribe-bin"));
        assert_eq!(
            calls[0].1,
            vec!["--model", "/opt/models/ggml.bin", "/media/interview.wav",]
        );
    }

    #[test]
    fn missing_placeholder_appends_the_path_and_errors_surface_stderr() {
        let runner = ScriptedRunner {
            status: 2,
            stdout: String::new(),
            stderr: "model path missing".into(),
            calls: RefCell::new(Vec::new()),
        };
        let transcriber = CommandLineTranscriber::new(
            "transcribe-bin",
            vec!["--output-srt".into()],
            TranscriptFormat::Srt,
            &runner,
        );
        let err = transcriber.transcribe(Path::new("take.wav")).unwrap_err();
        assert_eq!(
            err,
            GraphicsError::Transcription("model path missing".into())
        );
        assert_eq!(runner.calls.borrow()[0].1, vec!["--output-srt", "take.wav"]);
    }

    #[test]
    fn apply_transcript_validates_before_replacing_captions() {
        let mut sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
        sequence.captions.push(videocut_model::Caption {
            id: "cap-keep".into(),
            start_ticks: 0,
            end_ticks: 10,
            text: "keep".into(),
        });
        let bad = [TranscriptSegment {
            start_ticks: 5,
            end_ticks: 5,
            text: "nope".into(),
        }];
        assert_eq!(
            apply_transcript(&mut sequence, &bad, ImportMode::Replace).unwrap_err(),
            GraphicsError::InvalidRange
        );
        assert_eq!(sequence.captions[0].text, "keep");

        let good = [TranscriptSegment {
            start_ticks: TICKS_PER_SECOND,
            end_ticks: TICKS_PER_SECOND * 2,
            text: "from speech".into(),
        }];
        apply_transcript(&mut sequence, &good, ImportMode::Replace).unwrap();
        assert_eq!(sequence.captions.len(), 1);
        assert_eq!(sequence.captions[0].text, "from speech");
    }

    #[test]
    fn process_runner_reports_a_missing_program() {
        let err = ProcessCommandRunner
            .run(Path::new("videocut-missing-transcriber"), &[])
            .unwrap_err();
        assert!(matches!(err, GraphicsError::Transcription(_)));
    }

    #[cfg(unix)]
    #[test]
    fn process_runner_captures_stdout_without_a_shell() {
        let output = ProcessCommandRunner
            .run(Path::new("/bin/echo"), &["ok".into()])
            .unwrap();
        assert_eq!(output.status, 0);
        assert_eq!(output.stdout.trim(), "ok");
    }
}
