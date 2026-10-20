use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use videocut_interchange::{
    export_edl, export_edl_sequence, export_fcpxml, export_otio, import_edl, import_fcpxml,
    import_otio,
};
use videocut_media::{
    export_sequence, import_media, ExportPreset, ExportRange, ExportRequest, FfmpegSidecar,
    ProResProfile,
};
use videocut_model::Project;
use videocut_prproj::PrprojDocument;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedProject {
    pub project_json: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOutcome {
    pub output_path: String,
    pub warnings: Vec<String>,
}

enum ExchangeFormat {
    Otio,
    Fcpxml,
    Edl,
}

pub fn read_prproj(path: &str) -> Result<LoadedProject, String> {
    let document = PrprojDocument::read_path(Path::new(path)).map_err(|error| error.to_string())?;
    let project_json =
        serde_json::to_string(document.project()).map_err(|error| error.to_string())?;
    Ok(LoadedProject {
        project_json,
        warnings: warning_lines(document.warnings()),
    })
}

pub fn write_prproj(path: &str, project_json: &str) -> Result<Vec<String>, String> {
    let project = parse_project(project_json)?;
    let document = PrprojDocument::from_project(&project).map_err(|error| error.to_string())?;
    document
        .write_path(Path::new(path))
        .map_err(|error| error.to_string())?;
    Ok(warning_lines(document.warnings()))
}

pub fn import_interchange(path: &str, format: &str) -> Result<LoadedProject, String> {
    let parsed = parse_format(format)?;
    let text = fs::read_to_string(path).map_err(|error| format!("読めません: {error}"))?;
    let imported = match parsed {
        ExchangeFormat::Otio => import_otio(&text),
        ExchangeFormat::Fcpxml => import_fcpxml(&text),
        ExchangeFormat::Edl => import_edl(&text),
    }
    .map_err(|error| error.to_string())?;
    Ok(LoadedProject {
        project_json: serde_json::to_string(&imported.project).map_err(|error| error.to_string())?,
        warnings: imported.warnings,
    })
}

pub fn export_interchange(
    path: &str,
    project_json: &str,
    format: &str,
    sequence_id: Option<&str>,
) -> Result<Vec<String>, String> {
    let project = parse_project(project_json)?;
    let exported = match parse_format(format)? {
        ExchangeFormat::Otio => export_otio(&project),
        ExchangeFormat::Fcpxml => export_fcpxml(&project),
        ExchangeFormat::Edl => match sequence_id.filter(|id| !id.is_empty()) {
            Some(id) => export_edl_sequence(&project, id),
            None => export_edl(&project),
        },
    }
    .map_err(|error| error.to_string())?;
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| format!("書けません: {error}"))?;
        }
    }
    fs::write(path, exported.text).map_err(|error| format!("書けません: {error}"))?;
    Ok(exported.warnings)
}

pub fn probe_media(path: &str) -> Result<String, String> {
    let sidecar = FfmpegSidecar::locate().map_err(|error| error.to_string())?;
    let media = import_media(&sidecar, Path::new(path)).map_err(|error| error.to_string())?;
    serde_json::to_string(&media).map_err(|error| error.to_string())
}

pub fn export_media(project_json: &str, request_json: &str) -> Result<ExportOutcome, String> {
    let project = parse_project(project_json)?;
    let request = media_request(request_json)?;
    let sidecar = FfmpegSidecar::locate().unwrap_or_else(|_| FfmpegSidecar {
        ffmpeg: PathBuf::from("ffmpeg"),
        ffprobe: PathBuf::from("ffprobe"),
    });
    let result =
        export_sequence(&sidecar, &project, &request).map_err(|error| error.to_string())?;
    Ok(ExportOutcome {
        output_path: result.output_path.display().to_string(),
        warnings: result.warnings,
    })
}

fn warning_lines(warnings: &[videocut_prproj::PrprojWarning]) -> Vec<String> {
    warnings
        .iter()
        .map(|warning| format!("{}: {}", warning.tag, warning.message))
        .collect()
}

fn parse_project(project_json: &str) -> Result<Project, String> {
    serde_json::from_str(project_json).map_err(|error| format!("プロジェクトの形式が違います: {error}"))
}

fn parse_format(format: &str) -> Result<ExchangeFormat, String> {
    match format {
        "otio" => Ok(ExchangeFormat::Otio),
        "fcpxml" => Ok(ExchangeFormat::Fcpxml),
        "edl" => Ok(ExchangeFormat::Edl),
        other => Err(format!("未対応の形式です: {other}")),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsExportRequest {
    sequence_id: String,
    preset: String,
    range: JsExportRange,
    output_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsExportRange {
    mode: String,
    #[serde(default)]
    in_ticks: i64,
    #[serde(default)]
    out_ticks: i64,
}

fn media_request(request_json: &str) -> Result<ExportRequest, String> {
    let body: JsExportRequest = serde_json::from_str(request_json)
        .map_err(|error| format!("書き出し設定の形式が違います: {error}"))?;
    let preset = match body.preset.as_str() {
        "h264" => ExportPreset::h264_mp4(),
        "prores" => ExportPreset::ProResMov {
            profile: ProResProfile::Hq,
        },
        "wav" => ExportPreset::Wav,
        other => return Err(format!("未対応のプリセットです: {other}")),
    };
    let range = match body.range.mode.as_str() {
        "entire" => ExportRange::Entire,
        "inOut" => ExportRange::InOut {
            in_ticks: body.range.in_ticks,
            out_ticks: body.range.out_ticks,
        },
        other => return Err(format!("未対応の範囲です: {other}")),
    };
    Ok(ExportRequest {
        sequence_id: body.sequence_id,
        preset,
        output_path: PathBuf::from(body.output_path),
        range,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use videocut_model::{Sequence, SequenceSettings, Track, TrackKind};

    fn project_with_sequence(name: &str) -> Project {
        let mut project = Project::new("bridge");
        let mut sequence = Sequence::new(name, SequenceSettings::hd1080p30());
        sequence.video_tracks.push(Track::new(0, TrackKind::Video));
        sequence.audio_tracks.push(Track::new(0, TrackKind::Audio));
        project.sequences.push(sequence);
        project
    }

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "videocut-bridge-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn prproj_roundtrip_keeps_the_project_name() {
        let dir = temp_dir("prproj");
        let path = dir.join("demo.prproj");
        let project = project_with_sequence("main");
        let json = serde_json::to_string(&project).unwrap();
        write_prproj(path.to_str().unwrap(), &json).unwrap();
        let loaded = read_prproj(path.to_str().unwrap()).unwrap();
        let parsed: Project = serde_json::from_str(&loaded.project_json).unwrap();
        assert_eq!(parsed.name, "bridge");
        assert_eq!(parsed.sequences[0].name, "main");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn interchange_roundtrip_keeps_the_sequence_name() {
        let project = project_with_sequence("main");
        let json = serde_json::to_string(&project).unwrap();
        for format in ["otio", "fcpxml", "edl"] {
            let dir = temp_dir(format);
            let path = dir.join(format!("cut.{format}"));
            export_interchange(path.to_str().unwrap(), &json, format, Some(&project.sequences[0].id))
                .unwrap();
            let loaded = import_interchange(path.to_str().unwrap(), format).unwrap();
            let parsed: Project = serde_json::from_str(&loaded.project_json).unwrap();
            assert_eq!(parsed.sequences[0].name, "main", "{format}");
            let _ = fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn unknown_exchange_format_is_rejected() {
        let error = import_interchange("missing.otio", "unknown-format").unwrap_err();
        assert!(error.contains("未対応"));
    }

    #[test]
    fn export_media_reaches_the_exporter() {
        let project = project_with_sequence("main");
        let json = serde_json::to_string(&project).unwrap();
        let request = format!(
            r#"{{"sequenceId":"{}","preset":"h264","range":{{"mode":"entire","inTicks":0,"outTicks":0}},"outputPath":"out.mp4"}}"#,
            project.sequences[0].id
        );
        let error = export_media(&json, &request).unwrap_err();
        assert!(
            error.contains("timeline is empty"),
            "{error}"
        );
    }

    #[test]
    fn probe_reports_a_media_error_for_a_missing_file() {
        let error = probe_media("missing-clip.mp4").unwrap_err();
        let lowered = error.to_lowercase();
        assert!(
            lowered.contains("ffmpeg")
                || lowered.contains("does not exist")
                || lowered.contains("not found")
                || error.contains("存在"),
            "{error}"
        );
    }
}
