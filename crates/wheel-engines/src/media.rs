use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FfmpegStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

pub fn no_window_command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

pub fn get_ffmpeg_status() -> FfmpegStatus {
    match find_ffmpeg_path() {
        Some(path) => {
            let version = no_window_command(&path)
                .arg("-version")
                .output()
                .ok()
                .and_then(|out| {
                    let text = String::from_utf8_lossy(&out.stdout).to_string();
                    text.lines().next().map(|l| l.to_string())
                });
            FfmpegStatus {
                installed: true,
                path: Some(path.to_string_lossy().to_string()),
                version,
            }
        }
        None => FfmpegStatus {
            installed: false,
            path: None,
            version: None,
        },
    }
}

pub fn find_ffmpeg_path() -> Option<PathBuf> {
    // 1. App-specific sidecar directory: %LOCALAPPDATA%\Lime\bin\ffmpeg.exe (fallback to legacy Wheel)
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let sidecar = PathBuf::from(&local_app_data).join("Lime").join("bin").join("ffmpeg.exe");
        if sidecar.exists() {
            return Some(sidecar);
        }
        let legacy_sidecar = PathBuf::from(&local_app_data).join("Wheel").join("bin").join("ffmpeg.exe");
        if legacy_sidecar.exists() {
            return Some(legacy_sidecar);
        }
    }

    // 2. Next to the current running binary
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            let next_to = dir.join("ffmpeg.exe");
            if next_to.exists() {
                return Some(next_to);
            }
        }
    }

    // 3. System PATH (check if ffmpeg runs successfully)
    if no_window_command("ffmpeg")
        .arg("-version")
        .output()
        .is_ok()
    {
        return Some(PathBuf::from("ffmpeg"));
    }

    None
}

/// Convert video or audio file using FFmpeg with safe argument arrays.
pub fn convert_media(input: &Path, output: &Path, target_format: &str) -> Result<PathBuf> {
    info!(
        "Converting media {:?} -> {:?} (target_format: {})",
        input, output, target_format
    );

    let ffmpeg = find_ffmpeg_path().ok_or_else(|| {
        anyhow::anyhow!("FFmpeg is not installed or not found in PATH or Lime sidecar directory")
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut cmd = no_window_command(ffmpeg);
    cmd.arg("-y").arg("-i").arg(input);

    match target_format.to_lowercase().as_str() {
        // High quality animated GIF with optimized palette
        "gif" => {
            cmd.arg("-vf")
                .arg("fps=15,scale=540:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse");
        }
        // MP4 with H.264 & AAC
        "mp4" => {
            cmd.arg("-c:v").arg("libx264").arg("-pix_fmt").arg("yuv420p").arg("-c:a").arg("aac");
        }
        // WebM with VP9 & Opus
        "webm" => {
            cmd.arg("-c:v").arg("libvpx-vp9").arg("-crf").arg("30").arg("-b:v").arg("0").arg("-c:a").arg("libopus");
        }
        // Audio extraction to MP3
        "mp3" => {
            cmd.arg("-vn").arg("-c:a").arg("libmp3lame").arg("-q:a").arg("2");
        }
        // Audio extraction to WAV
        "wav" => {
            cmd.arg("-vn").arg("-c:a").arg("pcm_s16le");
        }
        // Audio extraction to FLAC
        "flac" => {
            cmd.arg("-vn").arg("-c:a").arg("flac");
        }
        // Audio extraction to M4A / AAC
        "m4a" => {
            cmd.arg("-vn").arg("-c:a").arg("aac").arg("-b:a").arg("192k");
        }
        _ => {
            // Default copy/transcode based on extension
        }
    }

    cmd.arg(output);

    let status = cmd
        .status()
        .with_context(|| "Failed to execute FFmpeg command")?;

    if !status.success() {
        anyhow::bail!("FFmpeg process exited with error code: {:?}", status.code());
    }

    info!("Media conversion completed: {:?}", output);
    Ok(output.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffmpeg_detection_logic() {
        // Test that find_ffmpeg_path returns Option<PathBuf> cleanly without panic
        let path = find_ffmpeg_path();
        // Just verify function runs safely
        let _ = path;
    }
}
