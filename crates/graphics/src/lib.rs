//! Titles, lower thirds, and captions for VideoCut.
//!
//! Caption text lives on [`videocut_model::Sequence::captions`]. Title and
//! lower-third copy is stored as effect parameters on a
//! [`videocut_model::ClipKind::Title`] clip, so this crate does not extend the
//! editorial model.
//!
//! Timestamps in SRT and VTT are absolute sequence time. One second is
//! [`videocut_model::TICKS_PER_SECOND`] ticks. The sequence frame rate is not
//! used, because ticks are already a time unit.
//!
//! Speech-to-text is the [`Transcriber`] trait. [`CommandLineTranscriber`]
//! runs a program the caller supplies and reads SRT or VTT from its stdout.
//! This crate does not download speech models.
//!
//! ```
//! use videocut_graphics::{export_vtt, import_srt, ImportMode};
//! use videocut_model::{Sequence, SequenceSettings};
//!
//! let mut sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
//! let srt = "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n";
//! import_srt(&mut sequence, srt, ImportMode::Replace).unwrap();
//! let vtt = export_vtt(&sequence).unwrap();
//! assert!(vtt.contains("Hello"));
//! ```

mod caption;
mod error;
mod subtitle;
mod title;
mod transcribe;

pub use caption::{
    add_caption, captions_at, export_srt, export_vtt, import_srt, import_vtt, remove_caption,
    set_caption_text, set_caption_timing, ImportMode,
};
pub use error::GraphicsError;
pub use subtitle::{parse_srt, parse_vtt, write_srt, write_vtt, SubtitleCue};
pub use title::param_id;
pub use title::{
    add_lower_third, add_title, create_lower_third_clip, create_title_clip, read_lower_third,
    read_title, set_lower_third_text, set_title_text, LowerThirdContent, LowerThirdSpec, TextAlign,
    TextStyle, TitleContent, TitleSpec, LOWER_THIRD_EFFECT_ID, TITLE_EFFECT_ID,
};
pub use transcribe::{
    apply_transcript, CommandLineTranscriber, CommandOutput, CommandRunner, ProcessCommandRunner,
    Transcriber, TranscriptFormat, TranscriptSegment,
};
