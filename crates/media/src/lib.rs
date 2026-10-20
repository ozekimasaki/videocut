//! FFmpeg sidecar for VideoCut.
//!
//! The crate imports media into [`videocut_model::MediaRef`], builds half-resolution
//! proxies, poster frames, and waveform peaks, then exports a sequence. On Windows
//! the binaries are `ffmpeg.exe` and `ffprobe.exe`. Search order:
//!
//! 1. `VIDEOCUT_FFMPEG` and `VIDEOCUT_FFPROBE`
//! 2. `VIDEOCUT_FFMPEG_DIR`
//! 3. The application directory, then `ffmpeg/`, `ffmpeg/bin/`, and `resources/ffmpeg/`
//! 4. `PATH`
//!
//! Arguments are passed directly to the process. Paths with spaces stay one argument.
//! The filter graph for cuts and dissolves is documented in `FILTER_GRAPH.md` and
//! built by [`build_timeline_graph`].

mod error;
mod export;
mod filters;
mod graph;
mod probe;
mod proxy;
mod sidecar;
mod spans;
mod thumbnail;
mod time;
mod waveform;

pub use error::MediaError;
pub use export::{
    export_sequence, plan_export, select_prores_encoder, ExportPlan, ExportPreset, ExportRange,
    ExportRequest, ExportResult, ProResProfile, PRORES_ENCODERS,
};
pub use graph::{build_timeline_graph, sequence_end_ticks, FilterGraph, GraphInput, GraphOptions};
pub use probe::{
    import_media, is_supported_extension, media_ref_from_probe_json, supported_extensions,
    SUPPORTED_EXTENSIONS,
};
pub use proxy::{generate_proxy, plan_proxy, FfmpegPlan, ProxyOptions};
pub use sidecar::{binary_file_name, FfmpegSidecar, LocateOptions};
pub use thumbnail::{poster_ticks, write_poster, write_thumbnail, DEFAULT_THUMB_WIDTH};
pub use waveform::{
    waveform_from_mono_samples, waveform_peaks, WaveformOptions, WaveformPeaks,
    DEFAULT_SAMPLES_PER_PEAK, DEFAULT_WAVEFORM_RATE,
};
