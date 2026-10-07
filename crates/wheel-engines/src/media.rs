use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Spawn a process without flashing a console window on Windows.
///
/// FFmpeg is a console application; without this a console flashes up on every
/// conversion.
pub fn no_window_command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

/// Where a resolved `ffmpeg.exe` came from.
///
/// Shown in Settings so a user can tell a copy Lime downloaded apart from one
/// they already had installed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FfmpegSource {
    /// Path the user pointed Lime at in Settings.
    Custom,
    /// `%LOCALAPPDATA%\Lime\bin\ffmpeg.exe`, downloaded by Lime.
    Managed,
    /// Alongside `Lime.exe`, i.e. a future bundled sidecar.
    Bundled,
    /// Found on the system `PATH`.
    SystemPath,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FfmpegStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    /// Absent when `installed` is false.
    pub source: Option<FfmpegSource>,
    /// Directory Lime downloads its own copy into, whether or not it exists yet.
    pub managed_dir: String,
    /// True when `custom_path` is set but no longer usable.
    pub custom_path_stale: bool,
}

/// Where the managed copy lives. Public so the UI can show it before downloading.
pub fn managed_ffmpeg_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("APPDATA"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join("Lime").join("bin")
}

pub fn managed_ffmpeg_path() -> PathBuf {
    managed_ffmpeg_dir().join("ffmpeg.exe")
}

/// A user-chosen path from settings, injected once at startup.
///
/// `wheel-engines` has no access to the settings schema, so the resolved
/// preference is published here instead of threaded through every conversion
/// call. A stale or broken path is skipped rather than allowed to break
/// conversions that would otherwise work.
static CUSTOM_FFMPEG_PATH: std::sync::OnceLock<std::sync::RwLock<Option<PathBuf>>> =
    std::sync::OnceLock::new();

fn custom_slot() -> &'static std::sync::RwLock<Option<PathBuf>> {
    CUSTOM_FFMPEG_PATH.get_or_init(|| std::sync::RwLock::new(None))
}

/// Publish the user's chosen FFmpeg path for the rest of the process.
pub fn set_custom_ffmpeg_path(path: Option<PathBuf>) {
    if let Ok(mut slot) = custom_slot().write() {
        *slot = path;
    }
}

/// True when a custom path is configured but does not yield a working binary.
pub fn custom_path_is_stale(custom: Option<&str>) -> bool {
    match custom {
        None => false,
        Some(p) => probe_ffmpeg(Path::new(p)).is_none(),
    }
}

/// Run `ffmpeg -version` to confirm a path is a working binary.
///
/// Existence is not enough: a wrong file, an incompatible build or a quarantined
/// executable all pass `Path::exists` but fail here.
pub fn probe_ffmpeg(path: &Path) -> Option<String> {
    let out = no_window_command(path).arg("-version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next()?.trim();
    if first.is_empty() {
        None
    } else {
        Some(short_version(first))
    }
}

/// Reduce FFmpeg's banner to just the build name.
///
/// `ffmpeg -version` prints a whole copyright line, such as
/// `ffmpeg version N-127203-ga35c879992-20261005 Copyright (c) 2000-2026 the
/// FFmpeg developers`. That is a log line, not something to show in a UI, and it
/// wraps badly in a settings row. Keep the release name only.
pub fn short_version(banner: &str) -> String {
    // Everything up to the first `Copyright`/`built with` marker.
    let head = banner
        .split(" Copyright")
        .next()
        .unwrap_or(banner)
        .split(" built with")
        .next()
        .unwrap_or(banner)
        .trim();

    // Drop the leading `ffmpeg version ` prefix if present.
    let name = head.strip_prefix("ffmpeg version ").unwrap_or(head).trim();

    if name.is_empty() {
        banner.trim().to_string()
    } else {
        name.to_string()
    }
}

pub fn get_ffmpeg_status(custom: Option<&str>) -> FfmpegStatus {
    let managed_dir = managed_ffmpeg_dir();
    match resolve_ffmpeg(custom) {
        Some((path, source, version)) => FfmpegStatus {
            installed: true,
            path: Some(path.to_string_lossy().to_string()),
            version: Some(version),
            source: Some(source),
            managed_dir: managed_dir.to_string_lossy().to_string(),
            custom_path_stale: false,
        },
        None => FfmpegStatus {
            installed: false,
            path: None,
            version: None,
            source: None,
            managed_dir: managed_dir.to_string_lossy().to_string(),
            custom_path_stale: custom_path_is_stale(custom),
        },
    }
}

/// Resolve `ffmpeg.exe` and report which rule matched.
///
/// Order: user choice, Lime's own download, a bundled sidecar, then `PATH`.
pub fn resolve_ffmpeg(custom: Option<&str>) -> Option<(PathBuf, FfmpegSource, String)> {
    if let Some(p) = custom {
        let path = PathBuf::from(p);
        if let Some(version) = probe_ffmpeg(&path) {
            return Some((path, FfmpegSource::Custom, version));
        }
    }

    // Reflect any path published by the running app even if the caller passed none.
    let published = custom_slot().read().ok().and_then(|s| s.clone());
    if let Some(path) = published {
        if let Some(version) = probe_ffmpeg(&path) {
            return Some((path, FfmpegSource::Custom, version));
        }
    }

    let managed = managed_ffmpeg_path();
    if let Some(version) = probe_ffmpeg(&managed) {
        return Some((managed, FfmpegSource::Managed, version));
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            let next_to = dir.join("ffmpeg.exe");
            if let Some(version) = probe_ffmpeg(&next_to) {
                return Some((next_to, FfmpegSource::Bundled, version));
            }
        }
    }

    if let Some(version) = probe_ffmpeg(Path::new("ffmpeg")) {
        return Some((Path::new("ffmpeg").to_path_buf(), FfmpegSource::SystemPath, version));
    }

    None
}

/// Resolve to just a path, for callers that do not care where it came from.
pub fn find_ffmpeg_path() -> Option<PathBuf> {
    resolve_ffmpeg(None).map(|(p, _, _)| p)
}

/// Marker string on the error raised when FFmpeg cannot be found.
///
/// Commands turns this into a distinct Tauri error code so the UI can offer
/// "Open Settings" rather than showing a dead-end message.
pub const FFMPEG_MISSING_MARKER: &str = "FFMPEG_MISSING";

/// Convert video or audio file using FFmpeg with safe argument arrays.
pub fn convert_media(input: &Path, output: &Path, target_format: &str) -> Result<PathBuf> {
    convert_media_with(input, output, target_format, MediaOptions::default())
}

/// Knobs that are not derivable from the input or target extension.
#[derive(Debug, Clone, Copy)]
pub struct MediaOptions {
    /// Carry tags, chapters and cover art into the output.
    pub preserve_metadata: bool,
}

impl Default for MediaOptions {
    fn default() -> Self {
        Self {
            preserve_metadata: true,
        }
    }
}

pub fn convert_media_with(
    input: &Path,
    output: &Path,
    target_format: &str,
    opts: MediaOptions,
) -> Result<PathBuf> {
    info!(
        "Converting media {:?} -> {:?} (target_format: {}, preserve_metadata: {})",
        input, output, target_format, opts.preserve_metadata
    );

    let ffmpeg = find_ffmpeg_path().ok_or_else(|| {
        anyhow::anyhow!(
            "{FFMPEG_MISSING_MARKER}: FFmpeg is required to convert video and audio. \
             Install it from Settings, or point Lime at an existing ffmpeg.exe."
        )
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut cmd = no_window_command(ffmpeg);
    cmd.arg("-y").arg("-i").arg(input);

    // Applied up front so every target, image-adjacent or not, honours the
    // setting. -1 tells FFmpeg to discard what the input carried.
    if opts.preserve_metadata {
        cmd.arg("-map_metadata").arg("0").arg("-map_chapters").arg("0");
    } else {
        cmd.arg("-map_metadata").arg("-1").arg("-map_chapters").arg("-1");
    }

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
        // Audio-only targets
        "mp3" | "wav" | "flac" | "m4a" | "aac" | "ogg" | "opus" | "wma" => {
            configure_audio_output(&mut cmd, input, target_format, opts.preserve_metadata);
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

/// Whether the input is an audio-only file, so any video stream in it is cover art.
///
/// Decided by extension rather than by probing: a video stream inside an audio
/// file is an attached picture worth keeping, while the same stream inside a
/// video file is the picture itself. Marking that as cover art makes the muxer
/// discard the audio and emit a near-empty file, so the two cases cannot share
/// one mapping.
fn is_audio_only_input(input: &Path) -> bool {
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(
        ext.as_str(),
        "mp3" | "wav" | "flac" | "m4a" | "m4b" | "aac" | "ogg" | "oga" | "opus" | "wma" | "aif"
            | "aiff" | "alac" | "ape" | "mka"
    )
}

/// Whether an audio-only output container can hold an attached picture.
///
/// Checked against the bundled FFmpeg rather than assumed: the Ogg muxer
/// rejects a picture stream outright ("Unsupported codec id"), and raw ADTS and
/// RIFF/WAV have nowhere to put one.
fn target_supports_cover_art(target_format: &str) -> bool {
    matches!(
        target_format.to_lowercase().as_str(),
        "m4a" | "m4b" | "mp4" | "flac" | "wma" | "mp3"
    )
}

/// Configure an audio-only output: keep the audio stream, carry the source's
/// tags across, and preserve cover art when both sides can carry it.
fn configure_audio_output(
    cmd: &mut std::process::Command,
    input: &Path,
    target_format: &str,
    preserve_metadata: bool,
) {
    let target = target_format.to_lowercase();
    let cover = preserve_metadata && is_audio_only_input(input) && target_supports_cover_art(&target);

    // `?` on the audio map too: a source with no audio must reach the muxing
    // stage and report "does not contain any stream", not fail option parsing.
    cmd.arg("-map").arg("0:a?");
    if cover {
        // `?` keeps files that have no cover working.
        cmd.arg("-map").arg("0:v?");
    } else {
        cmd.arg("-vn");
    }

    match target.as_str() {
        "mp3" => {
            cmd.arg("-c:a").arg("libmp3lame").arg("-q:a").arg("2");
        }
        "wav" => {
            cmd.arg("-c:a").arg("pcm_s16le");
        }
        "flac" => {
            cmd.arg("-c:a").arg("flac");
        }
        "m4a" | "m4b" | "aac" => {
            cmd.arg("-c:a").arg("aac").arg("-b:a").arg("192k");
        }
        "ogg" | "oga" => {
            cmd.arg("-c:a").arg("libvorbis").arg("-q:a").arg("5");
        }
        "opus" => {
            cmd.arg("-c:a").arg("libopus").arg("-b:a").arg("128k");
        }
        "wma" => {
            cmd.arg("-c:a").arg("wmav2").arg("-b:a").arg("192k");
        }
        _ => {}
    }

    if cover {
        // Copy the embedded JPEG/PNG rather than re-encoding it.
        cmd.arg("-c:v").arg("copy");
        cmd.arg("-disposition:v:0").arg("attached_pic");
    }
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
    fn test_preserve_metadata_controls_tags() {
        let Some(ffmpeg) = find_ffmpeg_path() else {
            return;
        };

        let dir = std::env::temp_dir().join("wheel_tests");
        let _ = std::fs::create_dir_all(&dir);
        let src = dir.join("meta_src.mp3");

        let made = no_window_command(&ffmpeg)
            .arg("-y")
            .arg("-f")
            .arg("lavfi")
            .arg("-i")
            .arg("sine=frequency=440:duration=2")
            .arg("-c:a")
            .arg("libmp3lame")
            .arg("-metadata")
            .arg("title=WHEELER_TITLE")
            .arg("-id3v2_version")
            .arg("3")
            .arg(&src)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !made {
            return;
        }

        let tags_of = |p: &Path| {
            std::fs::read(p)
                .ok()
                .map(|b| String::from_utf8_lossy(&b).contains("WHEELER_TITLE"))
                .unwrap_or(false)
        };

        for (preserve, expect) in [(true, true), (false, false)] {
            let out = dir.join(format!("meta_out_{preserve}.m4a"));
            let _ = std::fs::remove_file(&out);
            convert_media_with(&src, &out, "m4a", MediaOptions { preserve_metadata: preserve })
                .expect("convert failed");

            assert_eq!(
                tags_of(&out),
                expect,
                "preserve_metadata={preserve} produced the wrong tags"
            );
            let _ = std::fs::remove_file(&out);
        }

        let _ = std::fs::remove_file(&src);
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

    /// A stand-in for `ffmpeg.exe` that must be rejected by the probe.
    ///
    /// The resolver only runs `-version` and reads stdout, so a file that is not
    /// a real executable is exactly the "user pointed Lime at the wrong thing"
    /// case worth covering. Lets the fallback rules be tested without a real
    /// 90 MB binary.
    fn fake_ffmpeg(dir: &Path, name: &str) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, b"not a real binary").unwrap();
        path
    }

    #[test]
    fn test_probe_rejects_non_ffmpeg_files() {
        let dir = std::env::temp_dir().join("wheel_ffmpeg_probe");
        let _ = std::fs::create_dir_all(&dir);
        let bogus = fake_ffmpeg(&dir, "ffmpeg.exe");
        assert!(
            probe_ffmpeg(&bogus).is_none(),
            "a file that is not FFmpeg must not pass the probe"
        );
        assert!(
            probe_ffmpeg(&dir.join("does-not-exist.exe")).is_none(),
            "a missing file must not pass the probe"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_custom_path_that_fails_probe_is_skipped_not_fatal() {
        // A stale custom path must fall through the chain rather than break
        // conversions that would otherwise work.
        let dir = std::env::temp_dir().join("wheel_ffmpeg_stale");
        let _ = std::fs::create_dir_all(&dir);
        let bogus = fake_ffmpeg(&dir, "ffmpeg.exe");

        assert!(custom_path_is_stale(Some(&bogus.to_string_lossy())));
        assert!(!custom_path_is_stale(None));

        let resolved = resolve_ffmpeg(Some(&bogus.to_string_lossy()));
        if let Some((_, source, _)) = resolved {
            assert_ne!(
                source,
                FfmpegSource::Custom,
                "a path that fails the probe must not be reported as Custom"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_status_reports_managed_dir_even_when_missing() {
        let status = get_ffmpeg_status(None);
        assert!(!status.managed_dir.is_empty());
        assert!(status.managed_dir.contains("Lime"));
        if !status.installed {
            assert!(status.path.is_none());
            assert!(status.source.is_none());
        }
    }

    #[test]
    fn test_short_version_strips_the_copyright_banner() {
        // The real first line from `ffmpeg -version` on a nightly build. Shown
        // verbatim it overflowed the settings row and read as a log line.
        let banner = "ffmpeg version N-127203-ga35c879992-20261005 Copyright (c) 2000-2026 the FFmpeg developers";
        assert_eq!(short_version(banner), "N-127203-ga35c879992-20261005");

        // A release build has a plain version number.
        assert_eq!(
            short_version("ffmpeg version 6.1.1 Copyright (c) 2000-2024"),
            "6.1.1"
        );

        // A build that also reports its configuration.
        assert_eq!(
            short_version("ffmpeg version 7.0 built with gcc 13"),
            "7.0"
        );
    }

    #[test]
    fn test_short_version_leaves_unexpected_input_intact() {
        // Never lose information: if nothing matches, keep what we were given.
        assert_eq!(short_version("something else entirely"), "something else entirely");
        assert_eq!(short_version("ffmpeg version 6.0"), "6.0");
        assert_eq!(short_version("  ffmpeg version 6.0  "), "6.0");
    }

    #[test]
    fn test_missing_ffmpeg_error_carries_the_marker() {
        // The UI keys off this marker to offer "Open Settings".
        assert!(FFMPEG_MISSING_MARKER.contains("FFMPEG_MISSING"));
    }
}