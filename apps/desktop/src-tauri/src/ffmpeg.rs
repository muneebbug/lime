//! Locating, reporting on and installing FFmpeg.
//!
//! FFmpeg is not bundled with Lime. Image conversions are pure Rust and need
//! nothing extra, but video, audio and PDF output all shell out to
//! `ffmpeg.exe`. This module answers three questions for the rest of the app:
//! do we have one, where is it, and how do we get one if not.

use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tracing::{info, warn};

use wheel_engines::media::{self, FfmpegStatus};

/// Rolling build from gyan.dev.
///
/// Deliberately not version-pinned: pinning would need a checksum refreshed by
/// hand, and a stale pin is its own kind of bug. The extracted binary is
/// verified by running it before it is put into place.
const DOWNLOAD_URL: &str = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";

/// Rough size of [`DOWNLOAD_URL`], used to warn before starting and to sanity
/// check the reported content length.
const EXPECTED_DOWNLOAD_BYTES: u64 = 115_000_000;

/// Free space needed for the zip plus the extracted binary at the same time.
const NEEDED_FREE_BYTES: u64 = 400_000_000;

/// Emitted as the download progresses.
#[derive(Clone, Serialize)]
pub struct DownloadProgress {
    pub received: u64,
    pub total: u64,
    /// 0.0–1.0, or `None` when the server sends no content length.
    pub fraction: Option<f32>,
}

pub const EVENT_PROGRESS: &str = "ffmpeg-download-progress";
pub const EVENT_FINISHED: &str = "ffmpeg-download-finished";

/// Everything the onboarding and Settings screens need to describe the current
/// state, including why it looks that way.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegReport {
    #[serde(flatten)]
    pub status: FfmpegStatus,
    /// True when media conversion is impossible right now.
    pub required_for_media: bool,
    /// Absolute path of the copy Lime installs into.
    pub managed_path: String,
    /// Present once a download has succeeded, so the UI can offer a refresh.
    pub managed_version: Option<String>,
}

pub fn report(custom: Option<&str>, managed_version: Option<&str>) -> FfmpegReport {
    FfmpegReport {
        status: media::get_ffmpeg_status(custom),
        required_for_media: media::find_ffmpeg_path().is_none(),
        managed_path: media::managed_ffmpeg_path().to_string_lossy().to_string(),
        managed_version: managed_version.map(str::to_string),
    }
}
/// Free bytes on the volume holding `path`, if discoverable.
fn free_space(path: &Path) -> Option<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
        use windows::core::PCWSTR;
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        wide.push(0);
        let mut free_to_caller: u64 = 0;
        unsafe {
            GetDiskFreeSpaceExW(
                PCWSTR(wide.as_ptr()),
                Some(&mut free_to_caller as *mut u64),
                None,
                None,
            )
            .ok()
            .map(|_| free_to_caller)
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

/// Pull `bin/ffmpeg.exe` out of the archive and install it.
///
/// Everything is written to `.partial` files and renamed into place only after
/// the binary has been executed successfully, so an interrupted download can
/// never leave a broken ffmpeg behind for the resolver to find.
pub async fn download_and_install(app: &AppHandle) -> Result<String, String> {
    let target_dir = media::managed_ffmpeg_dir();
    let target = media::managed_ffmpeg_path();

    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Could not create {}: {e}", target_dir.display()))?;

    if let Some(free) = free_space(&target_dir) {
        if free < NEEDED_FREE_BYTES {
            return Err(format!(
                "Not enough disk space. About {} MB is needed to install FFmpeg.",
                NEEDED_FREE_BYTES / 1_048_576
            ));
        }
    }

    let archive_path = std::env::temp_dir().join(format!(
        "lime-ffmpeg-{}.zip",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    let archive_partial = archive_path.with_extension("zip.partial");

    let cleanup = || {
        let _ = std::fs::remove_file(&archive_partial);
        let _ = std::fs::remove_file(&archive_path);
    };

    let client = reqwest::Client::builder()
        .user_agent(concat!("Lime/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("Could not create HTTP client: {e}"))?;

    let response = client
        .get(DOWNLOAD_URL)
        .send()
        .await
        .map_err(|e| format!("Could not reach the FFmpeg download server: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "FFmpeg download failed with HTTP {}",
            response.status()
        ));
    }

    // The release archive is served with Content-Length, but treat a missing or
    // wildly wrong value as unknown rather than reporting nonsense progress.
    let advertised = response.content_length();
    let total = match advertised {
        Some(n) if (10_000_000..=400_000_000).contains(&n) => n,
        _ => {
            warn!("Unexpected Content-Length: {advertised:?}, progress will be approximate");
            EXPECTED_DOWNLOAD_BYTES
        }
    };
    info!("Downloading FFmpeg from {DOWNLOAD_URL} ({total} bytes)");

    let mut file = std::fs::File::create(&archive_partial)
        .map_err(|e| format!("Could not create {}: {e}", archive_partial.display()))?;

    let mut stream = response.bytes_stream();
    let mut received: u64 = 0;
    let mut last_emit = std::time::Instant::now();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                cleanup();
                return Err(format!("Download interrupted: {e}"));
            }
        };
        use std::io::Write;
        if let Err(e) = file.write_all(&chunk) {
            cleanup();
            return Err(format!("Could not write download to disk: {e}"));
        }
        received += chunk.len() as u64;

        // Throttled so a 110 MB download does not flood the webview with events.
        if last_emit.elapsed() >= std::time::Duration::from_millis(100) {
            let _ = app.emit(
                EVENT_PROGRESS,
                DownloadProgress {
                    received,
                    total,
                    fraction: Some(received as f32 / total.max(1) as f32),
                },
            );
            last_emit = std::time::Instant::now();
        }
    }

    if let Err(e) = file.sync_all() {
        cleanup();
        return Err(format!("Could not finish writing download: {e}"));
    }
    drop(file);

    // A truncated stream can otherwise leave a corrupt but large file that only
    // fails much later during extraction, which reads as a broken download.
    if received < 1_000_000 {
        cleanup();
        return Err("Download finished but the file looks incomplete.".to_string());
    }
    if let Some(expected) = advertised {
        if received != expected {
            cleanup();
            return Err(format!(
                "Download was cut short ({received} of {expected} bytes). Try again."
            ));
        }
    }

    std::fs::rename(&archive_partial, &archive_path)
        .map_err(|e| format!("Could not stage download: {e}"))?;

    // Extract to a staging name first, verify, then swap in.
    let staged = target.with_extension("exe.partial");
    let extract_result = extract_ffmpeg(&archive_path, &staged);
    let _ = std::fs::remove_file(&archive_path);
    if let Err(e) = extract_result {
        let _ = std::fs::remove_file(&staged);
        return Err(e);
    }

    if media::probe_ffmpeg(&staged).is_none() {
        let _ = std::fs::remove_file(&staged);
        return Err("The downloaded file did not run as FFmpeg, so it was discarded.".to_string());
    }

    // Windows will not rename onto a running executable, so clear the old one first.
    if target.exists() {
        let _ = std::fs::remove_file(&target);
    }
    std::fs::rename(&staged, &target).map_err(|e| {
        let _ = std::fs::remove_file(&staged);
        format!("Could not install FFmpeg: {e}")
    })?;

    let version = media::probe_ffmpeg(&target).unwrap_or_default();
    info!("FFmpeg installed to {} ({version})", target.display());
    let _ = app.emit(
        EVENT_FINISHED,
        serde_json::json!({ "ok": true, "path": target.to_string_lossy(), "version": version }),
    );
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_picks_ffmpeg_out_of_a_gyan_style_layout() {
        let dir = std::env::temp_dir().join("lime_ffmpeg_zip");
        let _ = std::fs::create_dir_all(&dir);
        let archive = dir.join("build.zip");

        {
            let file = std::fs::File::create(&archive).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options: zip::write::FileOptions<'_, ()> =
                zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

            for name in [
                "ffmpeg-release-essentials/README.txt",
                "ffmpeg-release-essentials/bin/ffprobe.exe",
                "ffmpeg-release-essentials/bin/ffmpeg.exe",
            ] {
                zip.start_file(name, options).unwrap();
                std::io::Write::write_all(&mut zip, b"stub").unwrap();
            }
            zip.finish().unwrap();
        }

        let dest = dir.join("ffmpeg.exe");
        extract_ffmpeg(&archive, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"stub");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_archive_without_ffmpeg_is_an_error() {
        let dir = std::env::temp_dir().join("lime_ffmpeg_zip_empty");
        let _ = std::fs::create_dir_all(&dir);
        let archive = dir.join("build.zip");

        {
            let file = std::fs::File::create(&archive).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options: zip::write::FileOptions<'_, ()> =
                zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("readme/README.txt", options).unwrap();
            std::io::Write::write_all(&mut zip, b"nothing here").unwrap();
            zip.finish().unwrap();
        }

        let dest = dir.join("ffmpeg.exe");
        assert!(extract_ffmpeg(&archive, &dest).is_err());
        assert!(!dest.exists(), "no file should be written on failure");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_corrupt_archive_is_rejected_cleanly() {
        let dir = std::env::temp_dir().join("lime_ffmpeg_zip_corrupt");
        let _ = std::fs::create_dir_all(&dir);
        let archive = dir.join("build.zip");
        std::fs::write(&archive, b"this is not a zip file").unwrap();

        let dest = dir.join("ffmpeg.exe");
        let err = extract_ffmpeg(&archive, &dest).unwrap_err();
        assert!(err.contains("not a valid archive"), "unhelpful: {err}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Extract just `bin/ffmpeg.exe` from a gyan build.
///
/// The archive also carries docs, presets and licence texts that Lime has no use
/// for, so the rest is skipped rather than written to disk.
fn extract_ffmpeg(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive)
        .map_err(|e| format!("Could not open the downloaded archive: {e}"))?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| format!("The download was not a valid archive: {e}"))?;

    let wanted_suffix = format!("{}ffmpeg.exe", std::path::MAIN_SEPARATOR);
    let mut found = None;
    for i in 0..zip.len() {
        let entry = zip
            .by_index(i)
            .map_err(|e| format!("Could not read the archive: {e}"))?;
        let name = entry.name().replace('/', &std::path::MAIN_SEPARATOR.to_string());
        if name.to_lowercase().ends_with(&wanted_suffix.to_lowercase()) {
            found = Some(i);
            break;
        }
    }

    let index = found.ok_or_else(|| {
        "The archive did not contain ffmpeg.exe. It may be a build without it.".to_string()
    })?;

    let mut entry = zip
        .by_index(index)
        .map_err(|e| format!("Could not read ffmpeg.exe from the archive: {e}"))?;
    let mut out = std::fs::File::create(dest)
        .map_err(|e| format!("Could not create {}: {e}", dest.display()))?;
    std::io::copy(&mut entry, &mut out)
        .map_err(|e| format!("Could not write ffmpeg.exe: {e}"))?;
    Ok(())
}

/// Remove a custom path and Lime's own download.
pub fn reset(custom: Option<&str>, remove_managed: bool) {
    if custom.is_some() {
        media::set_custom_ffmpeg_path(None);
    }
    if remove_managed {
        let target = media::managed_ffmpeg_path();
        if let Err(e) = std::fs::remove_file(&target) {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!("Could not remove {}: {e}", target.display());
            }
        }
        let _ = std::fs::remove_dir(media::managed_ffmpeg_dir());
    }
}

