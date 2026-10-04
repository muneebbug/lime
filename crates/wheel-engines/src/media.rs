use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

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
        // Raw ADTS cannot carry a video stream, so the video must be dropped
        // explicitly or the muxer rejects the whole file.
        "aac" => {
            cmd.arg("-vn").arg("-c:a").arg("aac").arg("-b:a").arg("192k");
        }
        // AVI with MPEG-4 video and MP3 audio: the combination with the widest
        // player support. Left to FFmpeg's defaults the codec choice varies by
        // build, so pin it.
        "avi" => {
            cmd.arg("-c:v").arg("mpeg4").arg("-q:v").arg("4").arg("-c:a").arg("libmp3lame");
        }
        _ => {
            // Default copy/transcode based on extension
        }
    }

    cmd.arg(output);

    let result = cmd
        .output()
        .with_context(|| "Failed to execute FFmpeg command")?;

    let stderr = String::from_utf8_lossy(&result.stderr);

    if result.status.success() {
        debug!("FFmpeg output:\n{stderr}");
        info!("Media conversion completed: {:?}", output);
        return Ok(output.to_path_buf());
    }

    warn!(
        "FFmpeg conversion failed (exit code {:?}):\n{stderr}",
        result.status.code()
    );

    let detail = summarize_ffmpeg_stderr(result.stderr.as_slice());

    if is_audio_only_target(target_format) && indicates_no_encodable_stream(&detail) {
        let name = input
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| input.display().to_string());
        anyhow::bail!(
            "\"{}\" has no audio track, so there is nothing to convert to {}.",
            name,
            target_format.to_uppercase()
        );
    }

    anyhow::bail!(
        "FFmpeg conversion failed: {} (exit code {:?})",
        detail,
        result.status.code()
    );
}

/// Output targets that can only ever carry audio.
///
/// For these, FFmpeg having nothing to encode means the source has no audio
/// track — a dead end the user needs to know about, not a codec problem.
fn is_audio_only_target(target_format: &str) -> bool {
    matches!(
        target_format.to_lowercase().as_str(),
        "mp3" | "wav" | "flac" | "m4a" | "aac" | "opus" | "ogg"
    )
}

/// Whether FFmpeg's complaint was that it had no stream to write.
fn indicates_no_encodable_stream(detail: &str) -> bool {
    detail.contains("does not contain any stream") || detail.contains("matches no streams")
}

/// Extract the useful part of FFmpeg's stderr.
///
/// FFmpeg writes its version banner and full stream dump to stderr before doing
/// any work, so the exit code alone rarely identifies the problem. This keeps
/// only the lines that read like diagnostics; FFmpeg reports the root cause
/// first, followed by generic "Error opening output file" lines.
fn summarize_ffmpeg_stderr(stderr: &[u8]) -> String {
    const DIAGNOSTIC_MARKERS: [&str; 9] = [
        "does not contain",
        "Error",
        "Invalid",
        "No such",
        "Permission denied",
        "Unable",
        "Failed",
        "not supported",
        "Invalid data",
    ];

    let text = String::from_utf8_lossy(stderr);
    let mut kept: Vec<&str> = Vec::new();

    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if DIAGNOSTIC_MARKERS.iter().any(|m| line.contains(m)) {
            kept.push(line);
        }
    }

    match kept.len() {
        0 => "no diagnostic output".to_string(),
        n => kept[..n.min(2)].join(" | "),
    }
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

    /// Real stderr captured from `ffmpeg -i <video with no audio> out.mp3`.
    /// The root cause must survive; the banner and stream dump must not.
    const NO_AUDIO_STDERR: &[u8] = b"ffmpeg version 7.0.2-essentials_build-www.gyan.dev\n  built with gcc 13.2.0\n  configuration: --enable-gpl --enable-libmp3lame\n  libavutil      59.  8.100 / 59.  8.100\nInput #0, mov,mp4,m4a,3gp,3g2,mj2, from 'video.mp4':\n  Duration: 00:00:22.50, start: 0.000000, bitrate: 286 kb/s\n  Stream #0:0[0x1](und): Video: h264 (High), yuv420p(tv, bt709), 876x718, 30 fps\nOutput #0, mp3, to 'video.mp3':\n[out#0/mp3 @ 0000020c3ad82b80] Output file does not contain any stream\nError opening output file video.mp3.\nError opening output files: Invalid argument\n";

    #[test]
    fn test_summarize_ffmpeg_stderr_keeps_root_cause() {
        let summary = summarize_ffmpeg_stderr(NO_AUDIO_STDERR);
        assert!(
            summary.contains("does not contain any stream"),
            "root cause missing from: {summary}"
        );
        assert!(
            !summary.contains("ffmpeg version"),
            "banner leaked into: {summary}"
        );
        assert!(
            !summary.contains("Stream #0"),
            "stream dump leaked into: {summary}"
        );
        assert!(
            !summary.contains("libavutil"),
            "library banner leaked into: {summary}"
        );
    }

    #[test]
    fn test_summarize_ffmpeg_stderr_handles_empty_and_clean() {
        assert_eq!(summarize_ffmpeg_stderr(b""), "no diagnostic output");
        assert_eq!(
            summarize_ffmpeg_stderr(b"all good\nnothing to report\n"),
            "no diagnostic output"
        );
    }

    #[test]
    fn test_convert_audio_target_without_audio_track_reports_reason() {
        let Some(ffmpeg) = find_ffmpeg_path() else {
            return;
        };

        let dir = std::env::temp_dir().join("wheel_tests");
        let _ = std::fs::create_dir_all(&dir);

        // A 1s colour clip with no audio track: exactly the shape that makes
        // `ffmpeg -vn -c:a libmp3lame` fail with "does not contain any stream".
        let src = dir.join("silent_fixture.mp4");
        let out = dir.join("no_audio_track.mp3");
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&out);

        let made = no_window_command(&ffmpeg)
            .arg("-y")
            .arg("-f")
            .arg("lavfi")
            .arg("-i")
            .arg("color=c=red:s=64x64:r=5:d=1")
            .arg("-c:v")
            .arg("libx264")
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg(&src)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !made || !src.exists() {
            return;
        }

        let err = convert_media(&src, &out, "mp3").expect_err("expected failure");
        let msg = format!("{err:#}");

        assert_eq!(
            msg, "\"silent_fixture.mp4\" has no audio track, so there is nothing to convert to MP3.",
            "expected a plain-English dead-end message, got: {msg}"
        );
        assert!(
            !msg.contains("exit code"),
            "raw FFmpeg diagnostics leaked into the user-facing message: {msg}"
        );

        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn test_audio_target_detection_helpers() {
        for fmt in ["mp3", "MP3", "wav", "flac", "m4a"] {
            assert!(is_audio_only_target(fmt), "{fmt} should be audio-only");
        }
        for fmt in ["gif", "mp4", "webm", "png"] {
            assert!(!is_audio_only_target(fmt), "{fmt} should not be audio-only");
        }

        assert!(indicates_no_encodable_stream(
            "[out#0/mp3] Output file does not contain any stream"
        ));
        assert!(indicates_no_encodable_stream(
            "Stream map '0:a' matches no streams."
        ));
        assert!(!indicates_no_encodable_stream("Permission denied"));
    }
}
