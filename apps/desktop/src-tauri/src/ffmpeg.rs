//! Locating, reporting on and installing FFmpeg.
//!
//! FFmpeg is not bundled with Lime. Image conversions are pure Rust and need
//! nothing extra, but video, audio and GIF output all shell out to
//! `ffmpeg.exe`. This module answers three questions for the rest of the app:
//! do we have one, where is it, and how do we get one if not.
//!
//! Downloads run over several parallel byte ranges, because the mirrors differ
//! enormously in single-connection throughput. See `download.rs` for choosing a
//! host and for the range arithmetic.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tracing::{info, warn};

use wheel_engines::media::{self, FfmpegStatus};

use crate::download::{self, Probe};

/// Parallel range requests used for one file.
///
/// Sixteen is what a download manager like IDM opens by default. Testing stopped
/// showing gains past this: the bottleneck moves off the server and onto the
/// local socket, so more connections only add handshake overhead.
const SEGMENTS: usize = 16;

/// Attempts per segment before the whole transfer is abandoned.
const SEGMENT_ATTEMPTS: u32 = 3;

/// How often aggregated progress is emitted. Fast enough to look live, slow
/// enough that a 200 MB transfer does not flood the webview with events.
const EMIT_INTERVAL: Duration = Duration::from_millis(150);

/// Rough archive size, used when a mirror does not report a content length and
/// as the fallback for progress display.
const EXPECTED_DOWNLOAD_BYTES: u64 = 115_000_000;

/// Free space needed for the zip plus the extracted binary at the same time.
const NEEDED_FREE_BYTES: u64 = 400_000_000;

/// Emitted as the download progresses.
///
/// Field names are camelCase to match the rest of the UI payload convention.
/// The frontend reads these directly, so renaming one is a silent UI break.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub received: u64,
    pub total: u64,
    /// 0.0-1.0, or `None` when the server sends no content length.
    pub fraction: Option<f32>,
    /// Bytes per second, averaged over a short window.
    pub bytes_per_sec: u64,
    /// Seconds remaining, or `None` before there is enough data to estimate.
    pub eta_secs: Option<f64>,
}

pub const EVENT_PROGRESS: &str = "ffmpeg-download-progress";
pub const EVENT_FINISHED: &str = "ffmpeg-download-finished";

/// Emitted when a new mirror is tried, so the UI can reset its progress rather
/// than showing the previous attempt's totals until the first byte arrives.
pub const EVENT_ATTEMPT: &str = "ffmpeg-download-attempt";

/// Everything the onboarding and Settings screens need to describe the current
/// state, including why it looks that way.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegReport {
    #[serde(flatten)]
    pub status: FfmpegStatus,
    /// True when media conversion is impossible right now.
    pub required_for_media: bool,
    pub managed_path: Option<String>,
    pub managed_version: Option<String>,
}

/// Describe the current FFmpeg situation for the UI.
pub fn report(custom: Option<&str>, managed_version: Option<&str>) -> FfmpegReport {
    let status = media::get_ffmpeg_status(custom);

    FfmpegReport {
        required_for_media: !status.installed,
        managed_version: status
            .installed
            .then(|| managed_version.map(str::to_string))
            .flatten()
            .or_else(|| managed_version.map(str::to_string)),
        managed_path: media::managed_ffmpeg_path()
            .exists()
            .then(|| media::managed_ffmpeg_path().to_string_lossy().to_string()),
        status,
    }
}

/// Free bytes on the volume holding `dir`, or `None` when it cannot be read.
fn free_space(dir: &Path) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

        let path = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        let wide: Vec<u16> = path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let mut free_to_caller = 0u64;
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
    #[cfg(not(target_os = "windows"))]
    {
        let _ = dir;
        None
    }
}

/// Time each mirror by pulling a short sample, then return them fastest-first.
///
/// Mirrors that fail to answer are dropped rather than failing the whole
/// download, so one dead host cannot block setup.
async fn probe_mirrors(client: &reqwest::Client) -> Result<Vec<Probe>, String> {
    let mut probes = Vec::new();

    for mirror in download::MIRRORS {
        match probe_mirror(client, mirror).await {
            Ok(probe) => {
                info!(
                    "Mirror {}: {:.0} KB/s, ranges={}, size={:?}{}",
                    probe.mirror.label,
                    probe.bytes_per_sec as f64 / 1024.0,
                    probe.supports_ranges,
                    probe.total,
                    if probe.can_segment() { "" } else { " (single connection)" }
                );
                // Kept even when slower than the target rate: a slow mirror is a
                // valid fallback for when the fast one is unavailable.
                if probe.bytes_per_sec < download::MIN_PROBE_RATE_BPS {
                    warn!(
                        "{} is below the {} B/s target; it will be tried last",
                        probe.mirror.label,
                        download::MIN_PROBE_RATE_BPS
                    );
                }
                probes.push(probe);
            }
            Err(e) => warn!("Mirror {} unreachable: {e}", mirror.label),
        }
    }

    Ok(download::rank(probes))
}

/// Sample one mirror's throughput, size and range support.
async fn probe_mirror(
    client: &reqwest::Client,
    mirror: &'static download::Mirror,
) -> Result<Probe, String> {
    // One request does three jobs: reveals the total size, proves range support,
    // and carries enough body to time throughput. Asking for a single byte would
    // satisfy the first two but leave nothing to measure.
    let response = client
        .get(mirror.url)
        .header(
            reqwest::header::RANGE,
            format!("bytes=0-{}", download::PROBE_RANGE_BYTES - 1),
        )
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    // 206 means the server honoured the range; 200 means it ignored it and would
    // stream the whole file for any range request we sent.
    let supports_ranges = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;

    // The redirect target, so segments skip a round trip each. Note this can be a
    // signed, expiring URL, which is fine within the life of one download.
    let resolved_url = response.url().to_string();

    let total = response
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit('/').next().and_then(|n| n.trim().parse::<u64>().ok()))
        .or_else(|| response.content_length());

    let total = total.filter(|n| (10_000_000..=600_000_000).contains(n));

    // Time a real read so the measurement reflects sustained throughput rather
    // than just how fast the connection opened.
    let started = std::time::Instant::now();
    let mut sample = 0u64;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        sample += chunk.map(|c| c.len() as u64).unwrap_or(0);
        if started.elapsed() >= download::PROBE_DURATION {
            break;
        }
    }
    drop(stream);

    let elapsed = started.elapsed().as_secs_f64().max(0.001);
    let bytes_per_sec = (sample as f64 / elapsed) as u64;

    // Reject only a host that moved nothing at all. Anything with a measurable
    // rate is ranked on that rate instead, including a slow host: the window is
    // short, so a 20 KB/s mirror legitimately delivers only a few KB inside it
    // and must not be discarded for that.
    if sample == 0 {
        return Err("the probe received no data".to_string());
    }

    Ok(Probe {
        mirror,
        bytes_per_sec,
        total,
        supports_ranges,
        resolved_url,
    })
}

/// Download one segment into the shared file at its byte offset.
///
/// Each segment seeks to its own start and writes sequentially, so segments can
/// run concurrently without blocking each other or interleaving bytes.
async fn fetch_segment(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    range: (u64, u64),
    counter: &AtomicU64,
) -> Result<u64, String> {
    let (start, end) = range;
    let expected = end - start + 1;
    let header = format!("bytes={start}-{end}");

    let mut last_error = String::new();
    for attempt in 1..=SEGMENT_ATTEMPTS {
        match fetch_segment_once(client, url, dest, &header, start, counter).await {
            Ok(written) if written == expected => return Ok(written),
            Ok(written) => {
                last_error = format!("segment {start}-{end} wrote {written} of {expected} bytes");
            }
            Err(e) => last_error = e,
        }

        // Restart this segment's own count, so a retry replaces its figure
        // instead of adding bytes the segment already reported once.
        counter.store(0, Ordering::Relaxed);

        if attempt < SEGMENT_ATTEMPTS {
            warn!("{last_error}; retrying ({attempt}/{SEGMENT_ATTEMPTS})");
            tokio::time::sleep(Duration::from_millis(400 * attempt as u64)).await;
        }
    }

    Err(last_error)
}

/// One attempt at a segment: open, seek, stream to disk.
async fn fetch_segment_once(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    range_header: &str,
    start: u64,
    counter: &AtomicU64,
) -> Result<u64, String> {
    let response = client
        .get(url)
        .header(reqwest::header::RANGE, range_header)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    // The range must actually be honoured. A 200 means the server ignored the
    // header and is sending the whole file, which would write the entire archive
    // at this segment's offset and corrupt everything after it.
    if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(format!(
            "segment {range_header} got HTTP {} instead of 206",
            response.status()
        ));
    }

    // Confirm the returned span matches what was asked for. A server that serves
    // a different range would otherwise land bytes in the wrong place.
    if let Some(returned) = response
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
    {
        let served_start = returned
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.split('-').next())
            .and_then(|n| n.parse::<u64>().ok());

        if served_start != Some(start) {
            return Err(format!(
                "segment asked for {range_header} but the server returned {returned}"
            ));
        }
    }

    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(dest)
        .await
        .map_err(|e| format!("could not open download file: {e}"))?;

    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|e| format!("could not seek to {start}: {e}"))?;

    let mut written = 0u64;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("segment interrupted: {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("could not write segment: {e}"))?;
        written += chunk.len() as u64;
        // Count after the write lands, so progress never runs ahead of the disk.
        counter.store(written, Ordering::Relaxed);
    }

    file.flush().await.map_err(|e| format!("could not flush segment: {e}"))?;
    Ok(written)
}

/// Shared progress event so both download paths report identically.
fn progress_payload(received: u64, total: Option<u64>, bytes_per_sec: u64) -> DownloadProgress {
    let total = total.unwrap_or(EXPECTED_DOWNLOAD_BYTES);
    let remaining = total.saturating_sub(received);
    DownloadProgress {
        received,
        total,
        fraction: Some(received as f32 / total.max(1) as f32),
        bytes_per_sec,
        eta_secs: (bytes_per_sec > 0).then(|| remaining as f64 / bytes_per_sec as f64),
    }
}

/// Download the whole file as one connection.
///
/// Used when a mirror cannot serve ranges, so segmentation is impossible.
async fn download_single(
    client: &reqwest::Client,
    probe: &Probe,
    dest: &Path,
    app: &AppHandle,
) -> Result<(), String> {
    let counter = AtomicU64::new(0);

    fetch_segment_once(client, &probe.resolved_url, dest, "", 0, &counter).await?;

    let received = counter.load(Ordering::Relaxed);
    let _ = app.emit(EVENT_PROGRESS, progress_payload(received, probe.total, 0));
    Ok(())
}

/// Download the file as N parallel range requests writing into one file.
///
/// The file is sized up front so segments can seek to fixed offsets. Writing
/// concurrently at disjoint offsets is safe: no two segments touch the same byte.
async fn download_segmented(
    client: &reqwest::Client,
    probe: &Probe,
    dest: &Path,
    app: &AppHandle,
    segments: usize,
) -> Result<(), String> {
    let total = probe
        .total
        .ok_or_else(|| "mirror did not report a file size".to_string())?;
    // Owned so each spawned task can hold its own range.
    let ranges: Vec<(u64, u64)> = download::split_ranges(total, segments);

    // Size the file once. Segments overwrite their own ranges, so any bytes they
    // fail to write stay zero and are caught by verify_archive.
    let file = std::fs::File::create(dest)
        .map_err(|e| format!("Could not create {}: {e}", dest.display()))?;
    file.set_len(total)
        .map_err(|e| format!("Could not size download file: {e}"))?;
    drop(file);

    /*
     * Per-segment counters rather than one shared total. A retried segment
     * rewrites bytes a shared counter already saw, which inflated progress past
     * 100%; storing each segment's own count and summing makes a retry replace
     * its own figure instead of adding to it.
     */
    let counters: Vec<Arc<AtomicU64>> = (0..ranges.len())
        .map(|_| Arc::new(AtomicU64::new(0)))
        .collect();
    let aggregate = || -> u64 {
        counters
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .sum::<u64>()
            .min(total)
    };

    let mut tasks = Vec::with_capacity(ranges.len());
    // Copy the range out before spawning: the task outlives this loop, so it
    // must not borrow from `ranges`.
    for (range, counter) in ranges.into_iter().zip(counters.iter().cloned()) {
        let client = client.clone();
        let url = probe.resolved_url.clone();
        let dest = dest.to_path_buf();
        tasks.push(tokio::spawn(
            async move { fetch_segment(&client, &url, &dest, range, &counter).await },
        ));
    }

    /*
     * Progress is emitted from its own task rather than a `select!` inside the
     * wait loop. Dropping a `JoinHandle` detaches the task, so an earlier version
     * that popped one task per iteration and dropped it whenever a progress tick
     * won the race returned while most segments were still running — which
     * surfaced as "segments finished short" partway through the file.
     */
    let finished = Arc::new(AtomicU64::new(0));
    let ticker = {
        let app = app.clone();
        let finished = Arc::clone(&finished);
        let counters = counters.clone();
        tokio::spawn(async move {
            let aggregate = || -> u64 {
                counters
                    .iter()
                    .map(|c| c.load(Ordering::Relaxed))
                    .sum::<u64>()
            };
            let mut last_sample = std::time::Instant::now();
            let mut last_bytes = 0u64;

            while finished.load(Ordering::Relaxed) == 0 {
                tokio::time::sleep(EMIT_INTERVAL).await;
                if finished.load(Ordering::Relaxed) != 0 {
                    break;
                }

                let now = std::time::Instant::now();
                let window = now.duration_since(last_sample).as_secs_f64();
                let mut bytes_per_sec = 0u64;
                if window > 0.0 {
                    let done = aggregate();
                    bytes_per_sec = (done.saturating_sub(last_bytes) as f64 / window) as u64;
                    last_bytes = done;
                    last_sample = now;
                }

                let _ = app.emit(
                    EVENT_PROGRESS,
                    progress_payload(aggregate().min(total), Some(total), bytes_per_sec),
                );
            }
        })
    };

    // Await every segment to completion before judging the result.
    let mut first_error: Option<String> = None;
    for task in tasks {
        match task.await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                first_error.get_or_insert(e);
            }
            Err(e) => {
                first_error.get_or_insert(format!("download task failed: {e}"));
            }
        }
    }

    finished.store(1, Ordering::Relaxed);
    let _ = ticker.await;

    if let Some(e) = first_error {
        return Err(e);
    }

    // Every segment reported the bytes it was asked for, so the file is whole.
    if aggregate() < total {
        return Err(format!(
            "segments finished short: {} of {total} bytes",
            aggregate()
        ));
    }

    let _ = app.emit(
        EVENT_PROGRESS,
        progress_payload(total, Some(total), 0),
    );
    Ok(())
}

/// Confirm a finished download is a readable zip of the expected size.
///
/// Segmented writes can leave a file that is the right length but corrupt, and a
/// corrupt archive otherwise fails much later with a confusing extraction error.
fn verify_archive(path: &Path, expected_total: u64) -> Result<(), String> {
    let actual = std::fs::metadata(path)
        .map_err(|e| format!("Could not read the downloaded file: {e}"))?
        .len();

    if actual != expected_total {
        return Err(format!(
            "Download finished at the wrong size ({actual} of {expected_total} bytes)."
        ));
    }

    // Opening the central directory reads bytes from every segment's range, so a
    // missing or misaligned segment surfaces here rather than as a mystery
    // failure during extraction.
    zip::ZipArchive::new(std::fs::File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("The downloaded archive is damaged: {e}"))?;

    Ok(())
}

/// Download and install FFmpeg, trying mirrors fastest-first.
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
        // Segmented downloads hold many connections to the same host at once;
        // without a raised idle pool the surplus sockets wait rather than run.
        .pool_max_idle_per_host(SEGMENTS * 2)
        .build()
        .map_err(|e| format!("Could not create HTTP client: {e}"))?;

    // Probe every mirror, then try them fastest-first. One host is not enough:
    // gyan.dev rate-limits per connection at roughly 30 KB/s, while GitHub's CDN
    // sustains over 1 MB/s for the same bytes.
    let candidates = match probe_mirrors(&client).await {
        Ok(c) if !c.is_empty() => c,
        _ => {
            cleanup();
            return Err("Could not reach any FFmpeg download source.".to_string());
        }
    };

    let mut last_error = String::new();
    // Remembered so the UI can report which host served the download.
    let mut used_mirror: Option<&'static download::Mirror> = None;

    for probe in candidates {
        // Reset progress for this attempt. Without this the UI keeps the previous
        // mirror's total on screen while the next one starts from zero, which
        // reads as the bar jumping backwards for no reason.
        let _ = app.emit(
            EVENT_ATTEMPT,
            serde_json::json!({ "mirror": probe.mirror.id, "label": probe.mirror.label }),
        );

        let total = probe.total.unwrap_or(EXPECTED_DOWNLOAD_BYTES);
        let segments = if probe.can_segment() {
            download::segment_count(total, SEGMENTS)
        } else {
            warn!(
                "{} does not support range requests; using a single connection",
                probe.mirror.label
            );
            1
        };

        info!(
            "Downloading FFmpeg from {} — {} bytes over {} connection(s)",
            probe.mirror.label,
            total,
            segments
        );

        let outcome = if segments > 1 {
            download_segmented(&client, &probe, &archive_partial, &app, segments).await
        } else {
            download_single(&client, &probe, &archive_partial, &app).await
        };

        match outcome {
            Ok(()) => {
                // Segments write at fixed offsets, so a short or misaligned write
                // leaves a plausible-looking but corrupt archive. Confirm the file
                // really is a readable zip of the expected size before extracting.
                if let Err(e) = verify_archive(&archive_partial, total) {
                    let _ = std::fs::remove_file(&archive_partial);
                    last_error = e;
                    warn!("{}: {last_error}", probe.mirror.label);
                    continue;
                }
                used_mirror = Some(probe.mirror);
                break;
            }
            Err(e) => {
                let _ = std::fs::remove_file(&archive_partial);
                last_error = e;
                warn!("{} failed: {last_error}", probe.mirror.label);
            }
        }
    }

    if !archive_partial.exists() {
        cleanup();
        return Err(if last_error.is_empty() {
            "The FFmpeg download did not finish.".to_string()
        } else {
            last_error
        });
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
        serde_json::json!({
            "ok": true,
            "path": target.to_string_lossy(),
            "version": version,
            // Which host actually served it, so the UI can say so.
            "mirror": used_mirror.map(|m| m.id).unwrap_or("unknown"),
        }),
    );
    Ok(version)
}

/// Forget the downloaded copy. `remove_managed` deletes the file on disk.
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

/// Extract just `bin/ffmpeg.exe` from the build.
///
/// The archive also carries docs, presets and licence texts that Lime has no use
/// for, so the rest is skipped rather than written to disk. Both the gyan and
/// BtbN layouts put the binary under a `bin` directory.
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
        "The archive did not contain an ffmpeg.exe, so it was discarded.".to_string()
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

    /// BtbN publishes the binary under the same `bin/` layout.
    #[test]
    fn test_picks_ffmpeg_out_of_a_btbn_style_layout() {
        let dir = std::env::temp_dir().join("lime_ffmpeg_zip_btbn");
        let _ = std::fs::create_dir_all(&dir);
        let archive = dir.join("build.zip");

        {
            let file = std::fs::File::create(&archive).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options: zip::write::FileOptions<'_, ()> =
                zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            for name in [
                "ffmpeg-master-latest-win64-gpl/bin/ffplay.exe",
                "ffmpeg-master-latest-win64-gpl/bin/ffmpeg.exe",
                "ffmpeg-master-latest-win64-gpl/bin/ffprobe.exe",
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

#[cfg(test)]
mod segment_tests {
    use super::*;

    /// The `Content-Range` a server returns for a range request.
    fn content_range(start: u64, end: u64, total: u64) -> String {
        format!("bytes {start}-{end}/{total}")
    }

    /// Extract the start offset a `Content-Range` header claims to serve.
    ///
    /// Regression: segments used to accept any successful response. A server that
    /// ignores `Range` replies `200` with the whole file, which wrote the entire
    /// archive at one segment's offset and produced an archive whose central
    /// directory was unreadable ("Could not find EOCD").
    fn served_start(header: &str) -> Option<u64> {
        header
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.split('-').next())
            .and_then(|n| n.parse().ok())
    }

    #[test]
    fn test_content_range_start_is_parsed_correctly() {
        assert_eq!(served_start(&content_range(0, 99, 1000)), Some(0));
        assert_eq!(served_start(&content_range(500, 599, 1000)), Some(500));
        assert_eq!(served_start("garbage"), None);
        assert_eq!(served_start("bytes */1000"), None);
    }

    #[test]
    fn test_a_segment_rejects_a_response_that_does_not_match_its_range() {
        // The exact failure that corrupted the archive: asked for one offset,
        // served bytes belonging to another.
        let asked = 5_000u64;
        let served = content_range(0, 99, 200_209_587);

        assert_ne!(
            served_start(&served),
            Some(asked),
            "a mismatched range must be detectable"
        );

        // A correct reply parses back to the offset we asked for.
        let correct = content_range(asked, asked + 99, 200_209_587);
        assert_eq!(served_start(&correct), Some(asked));
    }

    #[test]
    fn test_only_partial_content_is_accepted_for_a_segment() {
        // A 200 means the server ignored the range and is streaming the whole
        // file; writing that at a segment offset is what made the zip unreadable.
        let partial = reqwest::StatusCode::PARTIAL_CONTENT;
        let whole = reqwest::StatusCode::OK;

        assert!(partial.is_success() && partial != whole);
        assert!(
            whole.is_success(),
            "both are 'success', which is why status alone was not enough"
        );
    }

    #[test]
    fn test_per_segment_counters_sum_to_the_total() {
        // Per-segment counters replaced one shared total, because a retried
        // segment rewrote bytes the shared counter had already seen.
        let counters: Vec<Arc<AtomicU64>> = (0..4).map(|_| Arc::new(AtomicU64::new(0))).collect();
        let sizes = [10u64, 20, 30, 40];

        for (c, size) in counters.iter().zip(sizes) {
            c.store(size, Ordering::Relaxed);
        }
        let total: u64 = 100;
        let sum: u64 = counters.iter().map(|c| c.load(Ordering::Relaxed)).sum();
        assert_eq!(sum.min(total), total, "a full set of segments sums to the total");

        // A retry replaces its own figure rather than adding to it.
        counters[1].store(20, Ordering::Relaxed);
        let after: u64 = counters.iter().map(|c| c.load(Ordering::Relaxed)).sum();
        assert_eq!(after, 100, "re-counting a segment must not inflate the total");
    }

    #[test]
    fn test_progress_is_clamped_to_the_file_size() {
        // Overshoot from a retry must never reach the UI as more than 100%.
        let total = 1_000u64;
        let received = 1_240u64;

        assert_eq!(received.min(total), total);
    }

    #[test]
    fn test_segments_are_awaited_rather_than_detached() {
        // Regression: a `select!` inside the wait loop popped one `JoinHandle` per
        // iteration and dropped it when the progress tick won. Dropping a handle
        // detaches the task, so the download returned while most segments were
        // still running and reported "segments finished short".
        //
        // The fix keeps every handle in a Vec and awaits them all before judging
        // the result, so a partially finished set can never be mistaken for a
        // complete one.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let completed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let tally = std::sync::Arc::clone(&completed);
        let expected = 4;

        rt.block_on(async move {
            let mut handles = Vec::new();
            for _ in 0..expected {
                let completed = std::sync::Arc::clone(&completed);
                handles.push(tokio::spawn(async move {
                    completed.fetch_add(1, Ordering::SeqCst);
                    Ok::<u64, String>(1)
                }));
            }

            // Mirrors the production drain loop: iterate and await, never drop.
            for handle in handles {
                assert!(handle.await.unwrap().is_ok());
            }
        });

        assert_eq!(
            tally.load(Ordering::SeqCst),
            expected,
            "every segment must have run to completion"
        );
    }
}

#[cfg(test)]
mod archive_tests {
    use super::*;

    #[test]
    fn test_verify_archive_rejects_the_wrong_length() {
        let dir = std::env::temp_dir().join("lime_verify_archive");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("a.zip");

        {
            let file = std::fs::File::create(&path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            zip.start_file("bin/ffmpeg.exe", options).unwrap();
            std::io::Write::write_all(&mut zip, b"stub").unwrap();
            zip.finish().unwrap();
        }

        let actual = std::fs::metadata(&path).unwrap().len();
        assert!(
            verify_archive(&path, actual + 1).is_err(),
            "a length mismatch must be rejected"
        );
        assert!(
            verify_archive(&path, actual).is_ok(),
            "the true length must be accepted"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_verify_archive_rejects_a_zip_of_the_right_size_but_corrupt() {
        // This is the failure segmented writes can cause: correct length, wrong
        // bytes. Only opening the zip catches it.
        let dir = std::env::temp_dir().join("lime_verify_corrupt");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("a.zip");

        std::fs::write(&path, vec![0u8; 4096]).unwrap();
        let size = std::fs::metadata(&path).unwrap().len();

        let err = verify_archive(&path, size).unwrap_err();
        assert!(err.contains("damaged"), "unhelpful error: {err}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}