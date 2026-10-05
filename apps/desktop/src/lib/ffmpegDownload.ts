/**
 * Shared shape of the FFmpeg download progress event from Rust.
 *
 * Keep in step with `DownloadProgress` in `src-tauri/src/ffmpeg.rs`.
 */
/**
 * Shape of the FFmpeg download progress event from Rust.
 *
 * Keep in step with `DownloadProgress` in `src-tauri/src/ffmpeg.rs`. The fields
 * arrive camelCase; reading the snake_case Rust names yields undefined and a
 * progress bar that never moves.
 */
export interface FfmpegDownloadProgress {
  /** Bytes written to disk so far. */
  received: number;
  /** Full download size, or an estimate when the server sends none. */
  total: number;
  /** 0-1, or null when the total is unknown. */
  fraction: number | null;
  /** Bytes per second, averaged over a short window. */
  bytesPerSec: number;
  /** Seconds remaining, or null before there is enough data to estimate. */
  etaSecs: number | null;
}

/** Human byte size, e.g. `84.3 MB`. Binary units, matching what Windows shows. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";

  const units = ["B", "KB", "MB", "GB"];
  const exp = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** exp;

  // Whole bytes never need a decimal; larger units get one.
  return `${exp === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[exp]}`;
}

/** Download speed, e.g. `4.2 MB/s`. */
export function formatSpeed(bytesPerSec: number): string {
  if (!Number.isFinite(bytesPerSec) || bytesPerSec <= 0) return "—";
  return `${formatBytes(bytesPerSec)}/s`;
}

/** Percentage complete, clamped to 0-100. Handles a null or over-full fraction. */
export function formatPercent(fraction: number | null | undefined): number {
  if (fraction == null || !Number.isFinite(fraction)) return 0;
  return Math.max(0, Math.min(100, Math.round(fraction * 100)));
}

/** Rough time remaining, e.g. `about 25 seconds left`. */
export function formatEta(etaSecs: number | null | undefined): string | null {
  if (etaSecs == null || !Number.isFinite(etaSecs) || etaSecs < 0) return null;
  if (etaSecs < 1) return "almost done";

  const rounded = Math.round(etaSecs);
  if (rounded < 60) return `about ${rounded} second${rounded === 1 ? "" : "s"} left`;

  const minutes = Math.floor(rounded / 60);
  const seconds = rounded % 60;
  if (minutes < 60) {
    return seconds === 0
      ? `about ${minutes} minute${minutes === 1 ? "" : "s"} left`
      : `about ${minutes}m ${seconds}s left`;
  }

  const hours = Math.floor(minutes / 60);
  return `about ${hours}h ${minutes % 60}m left`;
}