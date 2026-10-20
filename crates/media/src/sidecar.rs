use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::error::MediaError;

/// `CREATE_NO_WINDOW` so a Windows desktop app does not flash a console.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Directories, relative to the application, that may hold the sidecar.
const INSTALL_CANDIDATES: &[&str] = &["", "ffmpeg", "ffmpeg/bin", "resources/ffmpeg"];

/// Located `ffmpeg` and `ffprobe` binaries.
///
/// On Windows the file names are `ffmpeg.exe` and `ffprobe.exe`. Arguments are
/// passed as a vector, not through `cmd.exe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfmpegSidecar {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

/// Explicit search inputs. [`FfmpegSidecar::locate`] fills this from the process.
#[derive(Debug, Clone, Default)]
pub struct LocateOptions {
    pub ffmpeg_env: Option<PathBuf>,
    pub ffprobe_env: Option<PathBuf>,
    pub dir_env: Option<PathBuf>,
    pub install_root: Option<PathBuf>,
    pub search_path: bool,
}

impl LocateOptions {
    pub fn from_process() -> Self {
        Self {
            ffmpeg_env: env_path("VIDEOCUT_FFMPEG"),
            ffprobe_env: env_path("VIDEOCUT_FFPROBE"),
            dir_env: env_path("VIDEOCUT_FFMPEG_DIR"),
            install_root: std::env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(Path::to_path_buf)),
            search_path: true,
        }
    }
}

impl FfmpegSidecar {
    /// Resolve the sidecar using environment variables, the install tree, then `PATH`.
    pub fn locate() -> Result<Self, MediaError> {
        Self::locate_with(&LocateOptions::from_process())
    }

    pub fn locate_with(options: &LocateOptions) -> Result<Self, MediaError> {
        if let (Some(ffmpeg), Some(ffprobe)) = (&options.ffmpeg_env, &options.ffprobe_env) {
            if ffmpeg.is_file() && ffprobe.is_file() {
                return Ok(Self {
                    ffmpeg: ffmpeg.clone(),
                    ffprobe: ffprobe.clone(),
                });
            }
        }
        if let Some(dir) = &options.dir_env {
            if let Ok(found) = Self::from_dir(dir) {
                return Ok(found);
            }
        }
        if let Some(root) = &options.install_root {
            if let Ok(found) = Self::from_install_tree(root) {
                return Ok(found);
            }
        }
        if options.search_path {
            if let Ok(found) = Self::from_path() {
                return Ok(found);
            }
        }
        Err(MediaError::SidecarNotFound)
    }

    /// Both binaries live directly in `dir`.
    pub fn from_dir(dir: &Path) -> Result<Self, MediaError> {
        let ffmpeg = dir.join(binary_file_name("ffmpeg"));
        let ffprobe = dir.join(binary_file_name("ffprobe"));
        if ffmpeg.is_file() && ffprobe.is_file() {
            Ok(Self { ffmpeg, ffprobe })
        } else {
            Err(MediaError::SidecarNotFound)
        }
    }

    /// Search `root`, `root/ffmpeg`, `root/ffmpeg/bin`, and `root/resources/ffmpeg`.
    pub fn from_install_tree(root: &Path) -> Result<Self, MediaError> {
        for relative in INSTALL_CANDIDATES {
            let dir = if relative.is_empty() {
                root.to_path_buf()
            } else {
                root.join(relative)
            };
            if let Ok(found) = Self::from_dir(&dir) {
                return Ok(found);
            }
        }
        Err(MediaError::SidecarNotFound)
    }

    pub fn from_path() -> Result<Self, MediaError> {
        let ffmpeg = PathBuf::from(binary_file_name("ffmpeg"));
        let ffprobe = PathBuf::from(binary_file_name("ffprobe"));
        if command_succeeds(&ffmpeg, &["-version"]) && command_succeeds(&ffprobe, &["-version"]) {
            Ok(Self { ffmpeg, ffprobe })
        } else {
            Err(MediaError::SidecarNotFound)
        }
    }

    pub fn run(&self, args: &[String]) -> Result<(), MediaError> {
        let output = run_command(&self.ffmpeg, args, false)?;
        status_error(&output.status, &output.stderr)?;
        Ok(())
    }

    pub fn run_capture(&self, args: &[String]) -> Result<Vec<u8>, MediaError> {
        let output = run_command(&self.ffmpeg, args, true)?;
        status_error(&output.status, &output.stderr)?;
        Ok(output.stdout)
    }

    pub fn probe_json(&self, path: &Path) -> Result<String, MediaError> {
        let args = vec![
            "-hide_banner".to_string(),
            "-loglevel".to_string(),
            "error".to_string(),
            "-print_format".to_string(),
            "json".to_string(),
            "-show_format".to_string(),
            "-show_streams".to_string(),
            path_arg(path),
        ];
        let output = run_command(&self.ffprobe, &args, true)?;
        status_error(&output.status, &output.stderr).map_err(|error| match error {
            MediaError::FfmpegFailed { message } => MediaError::ProbeFailed { message },
            other => other,
        })?;
        String::from_utf8(output.stdout).map_err(|_| MediaError::ProbeFailed {
            message: "ffprobe returned non-utf8 json".into(),
        })
    }

    pub fn encoder_listing(&self) -> Result<String, MediaError> {
        let args = vec!["-hide_banner".to_string(), "-encoders".to_string()];
        let stdout = self.run_capture(&args)?;
        String::from_utf8(stdout).map_err(|_| MediaError::FfmpegFailed {
            message: "encoder list was not utf8".into(),
        })
    }

    pub fn has_encoder(&self, name: &str) -> Result<bool, MediaError> {
        let listing = self.encoder_listing()?;
        Ok(encoder_listed(&listing, name))
    }
}

pub fn binary_file_name(stem: &str) -> String {
    binary_file_name_for(stem, cfg!(windows))
}

pub fn binary_file_name_for(stem: &str, windows: bool) -> String {
    if windows {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    }
}

/// True when `name` appears as its own token in `ffmpeg -encoders` output.
pub fn encoder_listed(listing: &str, name: &str) -> bool {
    listing
        .lines()
        .any(|line| line.split_whitespace().any(|word| word == name))
}

pub fn path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).map(PathBuf::from)
}

fn command_succeeds(program: &Path, args: &[&str]) -> bool {
    let mut command = Command::new(program);
    command.args(args);
    configure_command(&mut command);
    command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn run_command(
    program: &Path,
    args: &[String],
    capture_stdout: bool,
) -> Result<Output, MediaError> {
    let mut command = Command::new(program);
    command.args(args.iter().map(OsStr::new));
    configure_command(&mut command);
    command.stderr(Stdio::piped());
    if capture_stdout {
        command.stdout(Stdio::piped());
    } else {
        command.stdout(Stdio::null());
    }
    command.output().map_err(MediaError::from)
}

fn configure_command(command: &mut Command) {
    command.stdin(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
}

fn status_error(status: &ExitStatus, stderr: &[u8]) -> Result<(), MediaError> {
    if status.success() {
        return Ok(());
    }
    let code = status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "signal".to_string());
    let text = String::from_utf8_lossy(stderr);
    let trimmed = text.trim();
    let start = trimmed.len().saturating_sub(2_000);
    let start = trimmed.floor_char_boundary(start);
    Err(MediaError::FfmpegFailed {
        message: format!("exit {code}: {}", &trimmed[start..]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_sidecar_names_use_exe() {
        assert_eq!(binary_file_name_for("ffmpeg", true), "ffmpeg.exe");
        assert_eq!(binary_file_name_for("ffprobe", true), "ffprobe.exe");
        assert_eq!(binary_file_name_for("ffmpeg", false), "ffmpeg");
    }

    #[test]
    fn encoder_token_does_not_match_a_longer_name() {
        let listing = " VFS... prores_ks            Apple ProRes\n VF...D prores               Apple ProRes\n";
        assert!(encoder_listed(listing, "prores_ks"));
        assert!(encoder_listed(listing, "prores"));
        assert!(!encoder_listed(listing, "prores_aw"));
    }

    #[test]
    fn install_tree_finds_ffmpeg_subdirectory() {
        let root = std::env::temp_dir().join(format!(
            "videocut-sidecar-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let dir = root.join("ffmpeg");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(binary_file_name("ffmpeg")), b"").unwrap();
        std::fs::write(dir.join(binary_file_name("ffprobe")), b"").unwrap();
        let found = FfmpegSidecar::from_install_tree(&root).unwrap();
        assert_eq!(found.ffmpeg, dir.join(binary_file_name("ffmpeg")));
        assert_eq!(found.ffprobe, dir.join(binary_file_name("ffprobe")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_directory_is_not_a_sidecar() {
        let missing = std::env::temp_dir().join(format!(
            "videocut-sidecar-missing-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let error = FfmpegSidecar::from_dir(&missing).unwrap_err();
        assert!(matches!(error, MediaError::SidecarNotFound));
    }

    #[test]
    fn env_paths_win_over_the_install_tree() {
        let root = std::env::temp_dir().join(format!(
            "videocut-sidecar-order-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let bundled = root.join("ffmpeg");
        let env_dir = root.join("env");
        std::fs::create_dir_all(&bundled).unwrap();
        std::fs::create_dir_all(&env_dir).unwrap();
        for dir in [&bundled, &env_dir] {
            std::fs::write(dir.join(binary_file_name("ffmpeg")), b"").unwrap();
            std::fs::write(dir.join(binary_file_name("ffprobe")), b"").unwrap();
        }
        let options = LocateOptions {
            ffmpeg_env: Some(env_dir.join(binary_file_name("ffmpeg"))),
            ffprobe_env: Some(env_dir.join(binary_file_name("ffprobe"))),
            dir_env: None,
            install_root: Some(root.clone()),
            search_path: false,
        };
        let found = FfmpegSidecar::locate_with(&options).unwrap();
        assert_eq!(found.ffmpeg, env_dir.join(binary_file_name("ffmpeg")));
        let _ = std::fs::remove_dir_all(&root);
    }

    fn unique_suffix() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(0)
    }
}
