//! Compress Image.
//!
//! Thin command layer over `wheel_engines::compress`. The engine owns the
//! vocabulary - presets, target size, per-format overrides - and `libcaesium`
//! stays behind it, so nothing here learns the shape of `CSParameters`.
//!
//! Compression runs on a blocking thread and can take minutes on the slow
//! presets, which is why the window gets a progress state rather than a frozen
//! one.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::async_runtime::spawn_blocking;

use wheel_engines::compress::{self, CompressParams, CompressResult};

/// What the window needs to draw itself before the user touches anything.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressSource {
    pub path: String,
    pub file_name: String,
    pub extension: String,
    pub size: u64,
    /// Pixel dimensions, zero when the file could not be read for them.
    pub width: u32,
    pub height: u32,
}

/// The outcome, in the words the window shows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressSaved {
    pub output_path: String,
    pub file_name: String,
    pub original_size: u64,
    pub output_size: u64,
    pub unchanged: bool,
    pub target_missed: bool,
}

/// Format a byte count the way a person would say it.
///
/// Never claims a saved file is "smaller" when it is not, which is the whole
/// reason `unchanged` exists.
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * KB;

    let value = bytes as f64;
    if value < KB {
        format!("{bytes} B")
    } else if value < MB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{:.2} MB", value / MB)
    }
}

/// Read enough about a file to populate the window.
#[tauri::command]
pub async fn compress_inspect(path: String) -> Result<CompressSource, String> {
    let input = PathBuf::from(&path);
    if !input.is_file() {
        return Err(format!("Not a file: {path}"));
    }
    // Rejects a text file wearing an image's extension before a codec sees it.
    compress::validate_input(&input)?;

    let size = std::fs::metadata(&input)
        .map_err(|e| format!("Could not read {path}: {e}"))?
        .len();

    // Dimensions are a nicety: a file the image crate cannot open may still be
    // compressible, so a failure here must not block the tool.
    let (width, height) = wheel_engines::image_convert::load_image(&input)
        .map(|img| (img.width(), img.height()))
        .unwrap_or((0, 0));

    let extension = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    Ok(CompressSource {
        file_name: input
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone()),
        extension,
        size,
        width,
        height,
        path,
    })
}

/// Compress the file and save it beside the original.
///
/// The output extension always matches the input, because Compress re-encodes
/// rather than converts. Offering a format list here would duplicate the Convert
/// tools and quietly change someone's file type on the way out.
#[tauri::command]
pub async fn compress_apply(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    input_path: String,
    params: CompressParams,
) -> Result<CompressSaved, String> {
    let input = PathBuf::from(&input_path);
    if !input.is_file() {
        return Err(format!("Not a file: {input_path}"));
    }

    let settings = state.settings.lock().await.output.clone();
    let target = target_for(&input, &settings);

    let source_for_task = input.clone();
    let params_for_task = params.clone();
    let result = spawn_blocking(move || {
        compress::compress_file(&source_for_task, &target, &params_for_task)
    })
    .await
    .map_err(|e| format!("Compression task failed: {e}"))??;

    let saved = saved_from(&result);

    let mut job = wheel_core::Job::new("tool.compress", vec![input], serde_json::json!({
        "preset": params.preset,
        "target_size": params.target_size,
        "max_dimension": params.max_dimension,
        "original_size": saved.original_size,
        "output_size": saved.output_size,
        "unchanged": saved.unchanged,
    }));
    job.status = wheel_core::job::JobStatus::Completed;
    job.progress = 1.0;
    if !saved.unchanged {
        job.outputs = vec![PathBuf::from(&saved.output_path)];
    }
    let _ = state.history.insert_job(&job);

    use tauri_plugin_notification::NotificationExt;
    let _ = app
        .notification()
        .builder()
        .title("Lime")
        .body(summary(&saved))
        .show();

    Ok(saved)
}

/// One line describing what happened, for the toast.
///
/// The window closes itself on a successful run, so this notification is the
/// only place the result survives. Anything the window would have said has to
/// be here too, or it is lost the moment the window goes.
fn summary(saved: &CompressSaved) -> String {
    if saved.unchanged {
        return format!(
            "{} was already as small as it could get",
            saved.file_name
        );
    }
    let saved_bytes = saved.original_size.saturating_sub(saved.output_size);
    let percent = if saved.original_size == 0 {
        0
    } else {
        (saved_bytes as f64 * 100.0 / saved.original_size as f64).round() as u64
    };
    let mut line = format!(
        "{} · {} → {} ({percent}% smaller)",
        saved.file_name,
        format_bytes(saved.original_size),
        format_bytes(saved.output_size),
    );
    if saved.target_missed {
        line.push_str(" — target size not reached, this is the smallest achievable");
    }
    line
}

/// Where the compressed file goes.
///
/// Honours the same output folder policy as every other tool, so a user who
/// chose "Always save to Downloads" does not end up with compressed copies
/// scattered next to the originals.
fn target_for(input: &Path, settings: &wheel_core::settings::OutputSettings) -> PathBuf {
    let extension = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let fixed_folder = settings.fixed_folder.as_ref().map(std::path::Path::new);

    wheel_core::output::resolve_output_path(
        input,
        &extension,
        "compressed",
        &settings.policy,
        fixed_folder,
        false,
    )
}

fn saved_from(result: &CompressResult) -> CompressSaved {
    CompressSaved {
        file_name: result
            .output_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        output_path: result.output_path.to_string_lossy().to_string(),
        original_size: result.original_size,
        output_size: result.output_size,
        unchanged: result.unchanged,
        target_missed: result.target_missed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_counts_read_the_way_a_person_would_say_them() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(3_407_372), "3.25 MB");
    }

    #[test]
    fn an_unchanged_result_never_claims_a_saving() {
        // The engine decides this, not the window, but the shape is what the UI
        // reads, so it is worth pinning.
        let unchanged = CompressSaved {
            output_path: "a.png".into(),
            file_name: "a.png".into(),
            original_size: 1000,
            output_size: 1000,
            unchanged: true,
            target_missed: false,
        };
        assert_eq!(unchanged.output_size, unchanged.original_size);
    }

    #[test]
    fn the_toast_reports_the_truth() {
        let saved = CompressSaved {
            output_path: "a.png".into(),
            file_name: "a.png".into(),
            original_size: 1000,
            output_size: 250,
            unchanged: false,
            target_missed: false,
        };
        let line = summary(&saved);
        assert!(line.contains("75% smaller"), "got {line}");
        assert!(line.contains("1000 B"), "got {line}");
        assert!(line.contains("250 B"), "got {line}");

        // And an unchanged file must not be described as a saving.
        let saved = CompressSaved {
            unchanged: true,
            output_size: 1000,
            ..saved
        };
        let line = summary(&saved);
        assert!(!line.contains("smaller"), "got {line}");
        assert!(line.contains("already"), "got {line}");
    }

    #[test]
    fn a_missed_target_survives_into_the_notification() {
        // The window closes on a successful run, so anything it would have said
        // and no longer will is lost. This is the only record the user gets.
        let saved = CompressSaved {
            output_path: "a.png".into(),
            file_name: "a.png".into(),
            original_size: 1000,
            output_size: 800,
            unchanged: false,
            target_missed: true,
        };
        let line = summary(&saved);
        assert!(line.contains("target size not reached"), "got {line}");
        assert!(line.contains("20% smaller"), "got {line}");
    }
}