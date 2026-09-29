use std::path::PathBuf;
use tauri::{Manager, State, Emitter};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::AppState;
use wheel_core::{ActionManifest, WheelSettings};

/// Return all registered actions (filtered by enabled status)
#[tauri::command]
pub async fn get_actions(state: State<'_, AppState>) -> Result<Vec<ActionManifest>, String> {
    let actions: Vec<ActionManifest> = state
        .registry
        .all()
        .iter()
        .filter(|a| a.enabled)
        .cloned()
        .collect();
    Ok(actions)
}

/// Return the current settings
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<WheelSettings, String> {
    let mut settings = state.settings.lock().await.clone();
    settings.general.launch_at_login = wheel_win::shell::is_launch_at_login_registered();
    settings.general.explorer_context_menu = wheel_win::shell::is_context_menu_registered();
    Ok(settings)
}

/// Save updated settings
#[tauri::command]
pub async fn save_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings: WheelSettings,
) -> Result<(), String> {
    let migrated = settings.migrate();
    *state.settings.lock().await = migrated.clone();

    // Immediately update live Win32 hooks at runtime without requiring an app relaunch
    wheel_win::hooks::set_threshold(migrated.trigger.movement_threshold_px as i32);
    wheel_win::hooks::set_always_show(migrated.trigger.always_show);
    wheel_win::hooks::set_modifier(migrated.trigger.modifier.clone());
    wheel_win::hooks::set_paused(migrated.trigger.paused);

    // Synchronize Windows startup registration (Launch at Windows Login)
    if let Err(e) = wheel_win::shell::set_launch_at_login(migrated.general.launch_at_login) {
        tracing::warn!("Failed to synchronize launch at login: {}", e);
    }

    // Synchronize Windows Explorer context menu registration
    if migrated.general.explorer_context_menu {
        if let Err(e) = wheel_win::shell::register_context_menu(None) {
            tracing::warn!("Failed to register context menu: {}", e);
        }
    } else {
        if let Err(e) = wheel_win::shell::unregister_context_menu() {
            tracing::warn!("Failed to unregister context menu: {}", e);
        }
    }

    // Broadcast settings update to frontend windows
    let _ = app.emit("settings-updated", &migrated);

    let local_app_data = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("APPDATA"))
        .unwrap_or_else(|_| ".".to_string());
    let settings_path = std::path::PathBuf::from(local_app_data).join("Wheel").join("settings.json");
    if let Err(e) = migrated.save_to_path(&settings_path) {
        tracing::warn!("Failed to persist settings to disk at {:?}: {}", settings_path, e);
    } else {
        info!("Settings saved and persisted to disk at {:?}", settings_path);
    }

    Ok(())
}

/// Show the overlay at the specified cursor position
#[tauri::command]
pub async fn show_overlay(
    app: tauri::AppHandle,
    x: i32,
    y: i32,
) -> Result<(), String> {
    wheel_win::hooks::set_drag_armed(true);
    let _ = app.emit("drag-armed", serde_json::json!({ "x": x, "y": y }));
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.emit("drag-armed", serde_json::json!({ "x": x, "y": y }));
        let overlay_size = 400i32;
        let (ox, oy) = wheel_win::dpi::clamp_to_work_area(x, y, overlay_size, overlay_size);
        overlay
            .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: ox,
                y: oy,
            }))
            .map_err(|e| e.to_string())?;
        overlay.show().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Hide the overlay
#[tauri::command]
pub async fn hide_overlay(app: tauri::AppHandle) -> Result<(), String> {
    wheel_win::hooks::set_drag_armed(false);
    let _ = app.emit("drag-cancelled", ());
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.emit("drag-cancelled", ());
        overlay.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DispatchRequest {
    pub action_id: String,
    pub files: Vec<String>,
    pub params: serde_json::Value,
}

/// Dispatch an action (instant conversion or open tool window)
#[tauri::command]
pub async fn dispatch_action(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: DispatchRequest,
) -> Result<String, String> {
    let action_id = &request.action_id;

    let manifest = state
        .registry
        .get(action_id)
        .ok_or_else(|| format!("Unknown action: {}", action_id))?
        .clone();

    let files: Vec<PathBuf> = request.files.iter().map(PathBuf::from).collect();

    info!("Dispatching action '{}' on {} files", action_id, files.len());

    // Hide overlay immediately
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.hide();
    }

    use wheel_core::action::ActionKind;
    match manifest.kind {
        ActionKind::Instant => {
            // Queue a background job
            let job = wheel_core::Job::new(action_id.clone(), files, request.params);
            let _ = state.history.insert_job(&job);

            let job_id = state
                .job_queue
                .enqueue(job)
                .await
                .map_err(|e| e.to_string())?;

            // Run the actual conversion in a blocking task
            let state_arc = std::sync::Arc::clone(&state.inner().job_queue);
            let action_id_clone = action_id.clone();
            let app_clone = app.clone();
            tokio::spawn(async move {
                run_instant_action(app_clone, state_arc, job_id, &action_id_clone).await;
            });

            Ok(job_id.to_string())
        }
        ActionKind::Window => {
            // Open the tool window for this action
            open_tool_window(&app, &manifest, &files).map_err(|e| e.to_string())?;
            Ok(format!("window:{}", action_id))
        }
    }
}

fn get_in_flight_tools() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static IN_FLIGHT: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    IN_FLIGHT.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

fn open_tool_window(
    app: &tauri::AppHandle,
    manifest: &ActionManifest,
    files: &[PathBuf],
) -> anyhow::Result<()> {
    use tauri::WebviewUrl;

    let files_json = serde_json::to_string(files)?;
    let encoded = urlencoding::encode(&files_json);
    let url = format!(
        "index.html?window=tool&tool={}&files={}",
        manifest.id, encoded
    );

    let win_cfg = manifest.window.clone().unwrap_or_default();
    let window_id = format!("tool-{}", manifest.id.replace('.', "-"));

    // Reuse existing window if open
    if let Some(win) = app.get_webview_window(&window_id) {
        let _ = win.eval(&format!("window.location.href = '{}'", url));
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        #[cfg(target_os = "windows")]
        if let Ok(hwnd) = win.hwnd() {
            wheel_win::force_focus_window(hwnd.0 as isize);
        }
        return Ok(());
    }

    let in_flight = get_in_flight_tools();
    {
        let mut guard = in_flight.lock().unwrap();
        if guard.contains(&window_id) {
            info!("Tool window {} creation already in-flight, skipping duplicate", window_id);
            return Ok(());
        }
        guard.insert(window_id.clone());
    }

    let win_res = tauri::WebviewWindowBuilder::new(app, &window_id, WebviewUrl::App(url.into()))
        .title(&manifest.title)
        .inner_size(win_cfg.width as f64, win_cfg.height as f64)
        .decorations(false)
        .transparent(true)
        .resizable(win_cfg.resizable)
        .maximizable(false)
        .center()
        .focused(true)
        .build();

    {
        let mut guard = in_flight.lock().unwrap();
        guard.remove(&window_id);
    }

    let win = win_res?;
    let _ = win.unminimize();
    let _ = win.show();
    let _ = win.set_focus();
    #[cfg(target_os = "windows")]
    if let Ok(hwnd) = win.hwnd() {
        wheel_win::force_focus_window(hwnd.0 as isize);
    }

    Ok(())
}

async fn run_instant_action(
    app: tauri::AppHandle,
    queue: std::sync::Arc<wheel_core::JobQueue>,
    job_id: wheel_core::JobId,
    action_id: &str,
) {
    use wheel_core::job::JobStatus;
    use tauri::Emitter;

    // Mark as running
    queue
        .update_job(job_id, |j| j.status = JobStatus::Running)
        .await;

    let job = match queue.get_job(job_id).await {
        Some(j) => j,
        None => return,
    };

    // Update history db
    {
        let state = app.state::<crate::AppState>();
        let _ = state.history.update_job(&job);
    }

    // Determine target format from action_id (e.g. "convert.png" -> "png")
    let target_ext = action_id.split('.').nth(1).unwrap_or("png").to_string();

    let mut outputs = Vec::new();
    let mut error: Option<String> = None;

    let output_settings = {
        let state = app.state::<crate::AppState>();
        let out = state.settings.lock().await.output.clone();
        out
    };

    let chosen_folder: Option<PathBuf> = if output_settings.policy == wheel_core::OutputPolicy::AskEachTime {
        tokio::task::spawn_blocking(wheel_win::shell::pick_folder)
            .await
            .ok()
            .and_then(|r| r.ok())
            .flatten()
    } else {
        None
    };

    let effective_folder = if output_settings.policy == wheel_core::OutputPolicy::AskEachTime {
        chosen_folder.as_deref()
    } else {
        output_settings.fixed_folder.as_ref().map(std::path::Path::new)
    };

    if target_ext == "pdf" {
        if !job.inputs.is_empty() {
            let output_path = wheel_core::output::resolve_output_path(
                &job.inputs[0],
                "pdf",
                &output_settings.suffix,
                &output_settings.policy,
                effective_folder,
                output_settings.overwrite_source,
            );
            let inputs_clone = job.inputs.clone();
            let output_clone = output_path.clone();

            let result = tokio::task::spawn_blocking(move || {
                wheel_engines::pdf::images_to_pdf(&inputs_clone, &output_clone)
            })
            .await;

            match result {
                Ok(Ok(path)) => outputs.push(path),
                Ok(Err(e)) => error = Some(e.to_string()),
                Err(e) => error = Some(e.to_string()),
            }
        }
    } else {
        for input in &job.inputs {
            let input_ext = input
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .trim_start_matches('.')
                .to_lowercase();

            let output_path = wheel_core::output::resolve_output_path(
                input,
                &target_ext,
                &output_settings.suffix,
                &output_settings.policy,
                effective_folder,
                output_settings.overwrite_source,
            );

            // PDF extraction to images (PNG or JPG)
            if input_ext == "pdf" && (target_ext == "png" || target_ext == "jpg" || target_ext == "jpeg") {
                let input_clone = input.clone();
                let output_clone = output_path.clone();
                let fmt_clone = target_ext.clone();

                let result = tokio::task::spawn_blocking(move || {
                    wheel_engines::pdf::pdf_to_images(&input_clone, &output_clone, &fmt_clone)
                })
                .await;

                match result {
                    Ok(Ok(paths)) => outputs.extend(paths),
                    Ok(Err(e)) => {
                        error = Some(e.to_string());
                        break;
                    }
                    Err(e) => {
                        error = Some(e.to_string());
                        break;
                    }
                }
            } else if let Some(fmt) = wheel_engines::image_convert::OutputFormat::from_extension(&target_ext) {
                let input_clone = input.clone();
                let output_clone = output_path.clone();
                let params = wheel_engines::image_convert::ConvertParams {
                    output_format: fmt,
                    output_path: output_clone.clone(),
                    quality: 85,
                };
                let preserve_meta = output_settings.preserve_metadata;

                let result = tokio::task::spawn_blocking(move || -> anyhow::Result<PathBuf> {
                    let out = wheel_engines::image_convert::convert_image(&input_clone, &params)?;
                    if !preserve_meta {
                        let _ = wheel_engines::metadata::strip_metadata(&out, &out, false);
                    }
                    Ok(out)
                })
                .await;

                match result {
                    Ok(Ok(path)) => outputs.push(path),
                    Ok(Err(e)) => {
                        error = Some(e.to_string());
                        break;
                    }
                    Err(e) => {
                        error = Some(e.to_string());
                        break;
                    }
                }
            } else if matches!(target_ext.as_str(), "mp4" | "webm" | "mov" | "mkv" | "gif" | "mp3" | "wav" | "flac" | "m4a") {
                let input_clone = input.clone();
                let output_clone = output_path.clone();
                let fmt_clone = target_ext.clone();

                let result = tokio::task::spawn_blocking(move || {
                    wheel_engines::media::convert_media(&input_clone, &output_clone, &fmt_clone)
                })
                .await;

                match result {
                    Ok(Ok(path)) => outputs.push(path),
                    Ok(Err(e)) => {
                        error = Some(e.to_string());
                        break;
                    }
                    Err(e) => {
                        error = Some(e.to_string());
                        break;
                    }
                }
            } else {
                error = Some(format!("Unsupported format: {}", target_ext));
                break;
            }
        }
    }

    use tauri_plugin_notification::NotificationExt;

    if let Some(err) = error {
        queue
            .update_job(job_id, |j| {
                j.status = JobStatus::Failed;
                j.error = Some(err.clone());
            })
            .await;

        if let Some(final_job) = queue.get_job(job_id).await {
            let state = app.state::<crate::AppState>();
            let _ = state.history.update_job(&final_job);
        }

        let _ = app.emit(
            "job-failed",
            serde_json::json!({ "job_id": job_id, "error": err }),
        );

        let _ = app
            .notification()
            .builder()
            .title("Wheel — Conversion failed")
            .body(&err)
            .show();
    } else {
        queue
            .update_job(job_id, |j| {
                j.status = JobStatus::Completed;
                j.outputs = outputs.clone();
                j.progress = 1.0;
            })
            .await;

        if let Some(final_job) = queue.get_job(job_id).await {
            let state = app.state::<crate::AppState>();
            let _ = state.history.update_job(&final_job);
        }

        // Recycle original input files if configured
        if output_settings.recycle_source {
            for input in &job.inputs {
                if let Err(e) = trash::delete(input) {
                    tracing::warn!("Failed to recycle source file {:?}: {}", input, e);
                }
            }
        }

        // Copy converted outputs to clipboard if policy is set to clipboard
        if output_settings.policy == wheel_core::OutputPolicy::Clipboard {
            let _ = wheel_win::shell::copy_files_to_clipboard(&outputs);
        }

        let output_strs: Vec<String> = outputs
            .iter()
            .filter_map(|p| p.to_str().map(String::from))
            .collect();

        let _ = app.emit(
            "job-completed",
            serde_json::json!({ "job_id": job_id, "outputs": output_strs }),
        );

        let _ = app
            .notification()
            .builder()
            .title(format!("Wheel — {} complete", target_ext.to_uppercase()))
            .body(format!("Converted {} file(s) successfully", outputs.len()))
            .show();

        info!("Job {} completed: {:?}", job_id, outputs);
    }
}

/// Return all active jobs in the queue
#[tauri::command]
pub async fn get_jobs(state: State<'_, AppState>) -> Result<Vec<wheel_core::Job>, String> {
    Ok(state.job_queue.all_jobs().await)
}

/// Return persistent recent jobs from SQLite history
#[tauri::command]
pub async fn get_history(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<wheel_core::Job>, String> {
    state
        .history
        .get_recent_jobs(limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}

/// Clear all job history from SQLite
#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.history.clear_history().map_err(|e| e.to_string())
}

/// Delete a single job from SQLite history
#[tauri::command]
pub async fn delete_history_item(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let uuid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.history.delete_job(uuid).map_err(|e| e.to_string())
}

/// Reveal a file in Windows Explorer
#[tauri::command]
pub async fn open_in_folder(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(format!("File does not exist: {}", path));
    }

    std::process::Command::new("explorer")
        .arg(format!("/select,{}", path))
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Open a file with the default system application
#[tauri::command]
pub async fn open_file(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(format!("File does not exist: {}", path));
    }

    std::process::Command::new("cmd")
        .args(["/C", "start", "", &path])
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Crop an image using specific pixel bounds
#[tauri::command]
pub async fn crop_image_file(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    output_path: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    if !input.exists() {
        return Err(format!("Input file does not exist: {}", input_path));
    }

    let output_settings = {
        let out = state.settings.lock().await.output.clone();
        out
    };

    let target_out = match output_path {
        Some(p) => PathBuf::from(p),
        None => {
            let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("png");
            let fixed_folder = output_settings.fixed_folder.as_ref().map(std::path::Path::new);
            wheel_core::output::resolve_output_path(
                &input,
                ext,
                if output_settings.suffix.is_empty() { ".cropped" } else { &output_settings.suffix },
                &output_settings.policy,
                fixed_folder,
                output_settings.overwrite_source,
            )
        }
    };

    let params = wheel_engines::image_tool::CropParams { x, y, width, height };
    let input_clone = input.clone();
    let out_clone = target_out.clone();

    let res = tokio::task::spawn_blocking(move || {
        wheel_engines::image_tool::crop_image(&input_clone, &out_clone, &params)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    // Record job in history
    let mut job = wheel_core::Job::new("tool.crop", vec![input], serde_json::json!({ "x": x, "y": y, "width": width, "height": height }));
    job.status = wheel_core::job::JobStatus::Completed;
    job.progress = 1.0;
    job.outputs = vec![res.clone()];
    let _ = state.history.insert_job(&job);

    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder()
        .title("Wheel — Crop complete")
        .body(format!("Cropped image saved to {:?}", res.file_name().unwrap_or_default()))
        .show();

    Ok(res.to_string_lossy().to_string())
}

/// Compress an image using balanced or strong presets or target size
#[tauri::command]
pub async fn compress_image_file(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    preset: String,
    target_size_kb: Option<u64>,
    output_path: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    if !input.exists() {
        return Err(format!("Input file does not exist: {}", input_path));
    }

    let output_settings = {
        let out = state.settings.lock().await.output.clone();
        out
    };

    let target_out = match output_path {
        Some(p) => PathBuf::from(p),
        None => {
            let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
            let fixed_folder = output_settings.fixed_folder.as_ref().map(std::path::Path::new);
            wheel_core::output::resolve_output_path(
                &input,
                ext,
                if output_settings.suffix.is_empty() { ".min" } else { &output_settings.suffix },
                &output_settings.policy,
                fixed_folder,
                output_settings.overwrite_source,
            )
        }
    };

    let comp_preset = match preset.to_lowercase().as_str() {
        "strong" => wheel_engines::image_tool::CompressionPreset::Strong,
        _ => wheel_engines::image_tool::CompressionPreset::Balanced,
    };

    let params = wheel_engines::image_tool::CompressParams {
        preset: comp_preset,
        target_size_kb,
    };

    let input_clone = input.clone();
    let out_clone = target_out.clone();

    let res = tokio::task::spawn_blocking(move || {
        wheel_engines::image_tool::compress_image(&input_clone, &out_clone, &params)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    // Record job in history
    let mut job = wheel_core::Job::new("tool.compress", vec![input], serde_json::json!({ "preset": preset, "target_size_kb": target_size_kb }));
    job.status = wheel_core::job::JobStatus::Completed;
    job.progress = 1.0;
    job.outputs = vec![res.clone()];
    let _ = state.history.insert_job(&job);

    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder()
        .title("Wheel — Compression complete")
        .body(format!("Compressed file saved to {:?}", res.file_name().unwrap_or_default()))
        .show();

    Ok(res.to_string_lossy().to_string())
}

/// Retrieve structured metadata for an image/document
#[tauri::command]
pub async fn get_image_metadata(input_path: String) -> Result<wheel_engines::metadata::FileMetadataReport, String> {
    let p = PathBuf::from(&input_path);
    wheel_engines::metadata::read_metadata(&p).map_err(|e| e.to_string())
}

/// Strip metadata or GPS tags from an image
#[tauri::command]
pub async fn strip_image_metadata(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    strip_gps_only: bool,
    output_path: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    if !input.exists() {
        return Err(format!("Input file does not exist: {}", input_path));
    }

    let output_settings = {
        let out = state.settings.lock().await.output.clone();
        out
    };

    let target_out = match output_path {
        Some(p) => PathBuf::from(p),
        None => {
            let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
            let fixed_folder = output_settings.fixed_folder.as_ref().map(std::path::Path::new);
            let suffix = if strip_gps_only { ".nogps" } else { ".clean" };
            wheel_core::output::resolve_output_path(
                &input,
                ext,
                suffix,
                &output_settings.policy,
                fixed_folder,
                output_settings.overwrite_source,
            )
        }
    };

    let input_clone = input.clone();
    let out_clone = target_out.clone();

    let res = tokio::task::spawn_blocking(move || {
        wheel_engines::metadata::strip_metadata(&input_clone, &out_clone, strip_gps_only)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    // Record job in history
    let mut job = wheel_core::Job::new("tool.metadata", vec![input], serde_json::json!({ "strip_gps_only": strip_gps_only }));
    job.status = wheel_core::job::JobStatus::Completed;
    job.progress = 1.0;
    job.outputs = vec![res.clone()];
    let _ = state.history.insert_job(&job);

    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder()
        .title("Wheel — Metadata stripped")
        .body(format!("Cleaned file saved to {:?}", res.file_name().unwrap_or_default()))
        .show();

    Ok(res.to_string_lossy().to_string())
}

/// Add background to image
#[tauri::command]
pub async fn add_background_file(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    padding: u32,
    corner_radius: u32,
    shadow_blur: u32,
    aspect_ratio: String,
    color_start: [u8; 4],
    color_end: [u8; 4],
    format: String,
    output_path: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    if !input.exists() {
        return Err(format!("Input file does not exist: {}", input_path));
    }

    let output_settings = {
        let out = state.settings.lock().await.output.clone();
        out
    };

    let target_out = match output_path {
        Some(p) => PathBuf::from(p),
        None => {
            let ext = if format.is_empty() { "png" } else { &format };
            let fixed_folder = output_settings.fixed_folder.as_ref().map(std::path::Path::new);
            wheel_core::output::resolve_output_path(
                &input,
                ext,
                if output_settings.suffix.is_empty() { ".bg" } else { &output_settings.suffix },
                &output_settings.policy,
                fixed_folder,
                output_settings.overwrite_source,
            )
        }
    };

    let params = wheel_engines::bg::AddBgParams {
        padding,
        corner_radius,
        shadow_blur,
        aspect_ratio,
        color_start,
        color_end,
        format,
    };

    let input_clone = input.clone();
    let out_clone = target_out.clone();

    let res = tokio::task::spawn_blocking(move || {
        wheel_engines::bg::add_background(&input_clone, &out_clone, &params)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    // Record job in history
    let mut job = wheel_core::Job::new("tool.add_bg", vec![input], serde_json::json!({ "padding": padding, "corner_radius": corner_radius }));
    job.status = wheel_core::job::JobStatus::Completed;
    job.progress = 1.0;
    job.outputs = vec![res.clone()];
    let _ = state.history.insert_job(&job);

    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder()
        .title("Wheel — Background added")
        .body(format!("Backdrop image saved to {:?}", res.file_name().unwrap_or_default()))
        .show();

    Ok(res.to_string_lossy().to_string())
}

/// Edit photo adjustments (brightness, contrast, rotation, flip, resize)
#[tauri::command]
pub async fn edit_image_file(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    brightness: i32,
    contrast: f32,
    rotation: u32,
    flip_h: bool,
    flip_v: bool,
    resize_w: Option<u32>,
    resize_h: Option<u32>,
    output_path: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    if !input.exists() {
        return Err(format!("Input file does not exist: {}", input_path));
    }

    let output_settings = {
        let out = state.settings.lock().await.output.clone();
        out
    };

    let target_out = match output_path {
        Some(p) => PathBuf::from(p),
        None => {
            let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("png");
            let fixed_folder = output_settings.fixed_folder.as_ref().map(std::path::Path::new);
            wheel_core::output::resolve_output_path(
                &input,
                ext,
                if output_settings.suffix.is_empty() { ".edited" } else { &output_settings.suffix },
                &output_settings.policy,
                fixed_folder,
                output_settings.overwrite_source,
            )
        }
    };

    let params = wheel_engines::edit::EditParams {
        brightness,
        contrast,
        rotation,
        flip_h,
        flip_v,
        resize_w,
        resize_h,
    };

    let input_clone = input.clone();
    let out_clone = target_out.clone();

    let res = tokio::task::spawn_blocking(move || {
        wheel_engines::edit::edit_image(&input_clone, &out_clone, &params)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    // Record job in history
    let mut job = wheel_core::Job::new("tool.edit", vec![input], serde_json::json!({ "brightness": brightness, "contrast": contrast, "rotation": rotation }));
    job.status = wheel_core::job::JobStatus::Completed;
    job.progress = 1.0;
    job.outputs = vec![res.clone()];
    let _ = state.history.insert_job(&job);

    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder()
        .title("Wheel — Edit complete")
        .body(format!("Edited image saved to {:?}", res.file_name().unwrap_or_default()))
        .show();

    Ok(res.to_string_lossy().to_string())
}

/// Remove background from image
#[tauri::command]
pub async fn remove_background_file(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    feather_radius: u32,
    bg_color: Option<[u8; 4]>,
    format: String,
    output_path: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    if !input.exists() {
        return Err(format!("Input file does not exist: {}", input_path));
    }

    let output_settings = {
        let out = state.settings.lock().await.output.clone();
        out
    };

    let target_out = match output_path {
        Some(p) => PathBuf::from(p),
        None => {
            let ext = if format.is_empty() { "png" } else { &format };
            let fixed_folder = output_settings.fixed_folder.as_ref().map(std::path::Path::new);
            wheel_core::output::resolve_output_path(
                &input,
                ext,
                if output_settings.suffix.is_empty() { ".nobg" } else { &output_settings.suffix },
                &output_settings.policy,
                fixed_folder,
                output_settings.overwrite_source,
            )
        }
    };

    let params = wheel_engines::remove_bg::RemoveBgParams {
        feather_radius,
        bg_color,
        format,
    };

    let input_clone = input.clone();
    let out_clone = target_out.clone();

    let res = tokio::task::spawn_blocking(move || {
        wheel_engines::remove_bg::remove_background(&input_clone, &out_clone, &params)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    // Record job in history
    let mut job = wheel_core::Job::new("tool.remove_bg", vec![input], serde_json::json!({ "feather_radius": feather_radius }));
    job.status = wheel_core::job::JobStatus::Completed;
    job.progress = 1.0;
    job.outputs = vec![res.clone()];
    let _ = state.history.insert_job(&job);

    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder()
        .title("Wheel — Background removed")
        .body(format!("Image saved to {:?}", res.file_name().unwrap_or_default()))
        .show();

    Ok(res.to_string_lossy().to_string())
}

/// Retrieve model installation status
#[tauri::command]
pub async fn get_rmbg_model_status() -> Result<wheel_engines::remove_bg::ModelStatus, String> {
    Ok(wheel_engines::remove_bg::get_model_status())
}

/// Download RMBG-1.4 model with progress notifications
#[tauri::command]
pub async fn download_rmbg_model(app: tauri::AppHandle) -> Result<String, String> {
    use tauri::Emitter;
    let app_clone = app.clone();

    let res = tokio::task::spawn_blocking(move || {
        wheel_engines::remove_bg::download_model(move |percent, downloaded, total| {
            let _ = app_clone.emit(
                "model-download-progress",
                serde_json::json!({
                    "percent": percent,
                    "downloaded": downloaded,
                    "total": total,
                }),
            );
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    Ok(res.to_string_lossy().to_string())
}

/// Delete local RMBG model
#[tauri::command]
pub async fn delete_rmbg_model() -> Result<(), String> {
    wheel_engines::remove_bg::delete_model().map_err(|e| e.to_string())
}

/// Irreversibly redact regions of an image file
#[tauri::command]
pub async fn redact_image_file(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    regions: Vec<wheel_engines::redact::RedactRegion>,
    output_path: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    if !input.exists() {
        return Err(format!("Input file does not exist: {}", input_path));
    }

    let output_settings = {
        let out = state.settings.lock().await.output.clone();
        out
    };

    let target_out = match output_path {
        Some(p) => PathBuf::from(p),
        None => {
            let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("png");
            let fixed_folder = output_settings.fixed_folder.as_ref().map(std::path::Path::new);
            wheel_core::output::resolve_output_path(
                &input,
                ext,
                if output_settings.suffix.is_empty() { ".redacted" } else { &output_settings.suffix },
                &output_settings.policy,
                fixed_folder,
                output_settings.overwrite_source,
            )
        }
    };

    let input_clone = input.clone();
    let out_clone = target_out.clone();
    let regions_count = regions.len();

    let res = tokio::task::spawn_blocking(move || {
        wheel_engines::redact::redact_image(&input_clone, &out_clone, &regions)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    // Record job in history
    let mut job = wheel_core::Job::new("tool.redact", vec![input], serde_json::json!({ "regions_count": regions_count }));
    job.status = wheel_core::job::JobStatus::Completed;
    job.progress = 1.0;
    job.outputs = vec![res.clone()];
    let _ = state.history.insert_job(&job);

    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder()
        .title("Wheel — Redaction complete")
        .body(format!("Redacted image saved to {:?}", res.file_name().unwrap_or_default()))
        .show();

    Ok(res.to_string_lossy().to_string())
}

/// Apply annotation overlay onto an image file
#[tauri::command]
pub async fn annotate_image_file(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    overlay_base64: String,
    output_path: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    if !input.exists() {
        return Err(format!("Input file does not exist: {}", input_path));
    }

    let output_settings = {
        let out = state.settings.lock().await.output.clone();
        out
    };

    let target_out = match output_path {
        Some(p) => PathBuf::from(p),
        None => {
            let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("png");
            let fixed_folder = output_settings.fixed_folder.as_ref().map(std::path::Path::new);
            wheel_core::output::resolve_output_path(
                &input,
                ext,
                if output_settings.suffix.is_empty() { ".annotated" } else { &output_settings.suffix },
                &output_settings.policy,
                fixed_folder,
                output_settings.overwrite_source,
            )
        }
    };

    let input_clone = input.clone();
    let out_clone = target_out.clone();

    let res = tokio::task::spawn_blocking(move || {
        wheel_engines::annotate::apply_annotation_overlay_base64(&input_clone, &out_clone, &overlay_base64)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    // Record job in history
    let mut job = wheel_core::Job::new("tool.annotate", vec![input], serde_json::json!({}));
    job.status = wheel_core::job::JobStatus::Completed;
    job.progress = 1.0;
    job.outputs = vec![res.clone()];
    let _ = state.history.insert_job(&job);

    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder()
        .title("Wheel — Annotation complete")
        .body(format!("Annotated image saved to {:?}", res.file_name().unwrap_or_default()))
        .show();

    Ok(res.to_string_lossy().to_string())
}

/// Convert media file (video/audio) using safe FFmpeg argument arrays
#[tauri::command]
pub async fn convert_media_file(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    target_format: String,
    output_path: Option<String>,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    if !input.exists() {
        return Err(format!("Input file does not exist: {}", input_path));
    }

    let output_settings = {
        let out = state.settings.lock().await.output.clone();
        out
    };

    let target_out = match output_path {
        Some(p) => PathBuf::from(p),
        None => {
            let fixed_folder = output_settings.fixed_folder.as_ref().map(std::path::Path::new);
            wheel_core::output::resolve_output_path(
                &input,
                &target_format,
                &output_settings.suffix,
                &output_settings.policy,
                fixed_folder,
                output_settings.overwrite_source,
            )
        }
    };

    let input_clone = input.clone();
    let out_clone = target_out.clone();
    let fmt_clone = target_format.clone();

    let res = tokio::task::spawn_blocking(move || {
        wheel_engines::media::convert_media(&input_clone, &out_clone, &fmt_clone)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    // Record job in history
    let mut job = wheel_core::Job::new(
        format!("media.{}", target_format),
        vec![input],
        serde_json::json!({ "format": target_format }),
    );
    job.status = wheel_core::job::JobStatus::Completed;
    job.progress = 1.0;
    job.outputs = vec![res.clone()];
    let _ = state.history.insert_job(&job);

    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder()
        .title("Wheel — Media conversion complete")
        .body(format!("Converted media saved to {:?}", res.file_name().unwrap_or_default()))
        .show();

    Ok(res.to_string_lossy().to_string())
}

/// Retrieve FFmpeg detection status
#[tauri::command]
pub async fn get_ffmpeg_status() -> Result<wheel_engines::media::FfmpegStatus, String> {
    Ok(wheel_engines::media::get_ffmpeg_status())
}

/// Open the Settings window
#[tauri::command]
pub async fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::tray::open_settings_window(&app);
    Ok(())
}

/// Open the Command Palette window
#[tauri::command]
pub async fn open_palette_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::tray::open_palette_window(&app);
    Ok(())
}

/// Check if Windows Explorer context menu is active
#[tauri::command]
pub async fn is_explorer_context_menu_enabled() -> Result<bool, String> {
    Ok(wheel_win::shell::is_context_menu_registered())
}

/// Enable or disable Windows Explorer context menu
#[tauri::command]
pub async fn set_explorer_context_menu(enabled: bool) -> Result<(), String> {
    if enabled {
        wheel_win::shell::register_context_menu(None).map_err(|e| e.to_string())
    } else {
        wheel_win::shell::unregister_context_menu().map_err(|e| e.to_string())
    }
}

/// Prompt the user to pick a folder using the native Windows dialog
#[tauri::command]
pub async fn pick_folder() -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(wheel_win::shell::pick_folder)
        .await
        .map_err(|e| e.to_string())?
        .map(|opt| opt.map(|p| p.to_string_lossy().to_string()))
        .map_err(|e| e.to_string())
}

/// Open the %LOCALAPPDATA%\Wheel data directory in Windows Explorer
#[tauri::command]
pub async fn open_data_folder() -> Result<(), String> {
    let local_app_data = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("APPDATA"))
        .unwrap_or_else(|_| ".".to_string());
    let path = std::path::PathBuf::from(local_app_data).join("Wheel");
    let _ = std::fs::create_dir_all(&path);
    std::process::Command::new("explorer")
        .arg(&path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Execute a multi-action preset chain
#[tauri::command]
pub async fn run_preset(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    preset_id: String,
    input_path: String,
) -> Result<Vec<String>, String> {
    let input = PathBuf::from(&input_path);
    if !input.exists() {
        return Err(format!("Input file does not exist: {}", input_path));
    }

    let preset = {
        let s = state.settings.lock().await;
        s.presets.iter().find(|p| p.id == preset_id).cloned()
    }
    .ok_or_else(|| format!("Preset not found: {}", preset_id))?;

    info!("Executing preset '{}' on {:?}", preset.name, input);

    let mut current_input = input.clone();
    let mut intermediate_files: Vec<PathBuf> = Vec::new();

    let output_settings = {
        let out = state.settings.lock().await.output.clone();
        out
    };

    let steps = preset.steps.clone();
    let total_steps = steps.len();

    for (step_idx, step) in steps.into_iter().enumerate() {
        let is_last = step_idx == total_steps - 1;
        let ext = if step.action_id.starts_with("convert.") {
            step.action_id.split('.').nth(1).unwrap_or("png").to_string()
        } else {
            "png".to_string()
        };

        let target_out = if is_last {
            let fixed_folder = output_settings.fixed_folder.as_ref().map(std::path::Path::new);
            wheel_core::output::resolve_output_path(
                &input,
                &ext,
                &output_settings.suffix,
                &output_settings.policy,
                fixed_folder,
                output_settings.overwrite_source,
            )
        } else {
            current_input.with_extension(format!("tmp_{}.{}", step_idx, ext))
        };

        let in_clone = current_input.clone();
        let out_clone = target_out.clone();
        let action_id = step.action_id.clone();
        let params = step.params.clone();
        let ext_clone = ext.clone();

        let step_res = tokio::task::spawn_blocking(move || -> anyhow::Result<PathBuf> {
            if action_id == "tool.removebg" || action_id == "tool.remove_bg" {
                let p = wheel_engines::remove_bg::RemoveBgParams {
                    feather_radius: params.get("feather").and_then(|v| v.as_u64()).unwrap_or(2) as u32,
                    bg_color: None,
                    format: "png".into(),
                };
                wheel_engines::remove_bg::remove_background(&in_clone, &out_clone, &p)
            } else if action_id == "tool.metadata" {
                wheel_engines::metadata::strip_metadata(&in_clone, &out_clone, true)
            } else if action_id == "tool.addbg" || action_id == "tool.add_bg" {
                let p = wheel_engines::bg::AddBgParams {
                    padding: params.get("padding").and_then(|v| v.as_u64()).unwrap_or(40) as u32,
                    corner_radius: 16,
                    shadow_blur: 30,
                    aspect_ratio: "auto".into(),
                    color_start: [249, 115, 22, 255],
                    color_end: [234, 88, 12, 255],
                    format: "png".into(),
                };
                wheel_engines::bg::add_background(&in_clone, &out_clone, &p)
            } else if let Some(fmt) = wheel_engines::image_convert::OutputFormat::from_extension(&ext_clone) {
                let p = wheel_engines::image_convert::ConvertParams {
                    output_format: fmt,
                    output_path: out_clone.clone(),
                    quality: 90,
                };
                wheel_engines::image_convert::convert_image(&in_clone, &p)
            } else {
                anyhow::bail!("Unsupported preset action step: {}", action_id);
            }
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

        if !is_last {
            intermediate_files.push(step_res.clone());
        }
        current_input = step_res;
    }

    // Clean up temporary intermediate files
    for tmp in intermediate_files {
        let _ = std::fs::remove_file(tmp);
    }

    // Record job in history
    let mut job = wheel_core::Job::new(format!("preset.{}", preset.id), vec![input], serde_json::json!({ "name": preset.name }));
    job.status = wheel_core::job::JobStatus::Completed;
    job.progress = 1.0;
    job.outputs = vec![current_input.clone()];
    let _ = state.history.insert_job(&job);

    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder()
        .title("Wheel — Preset Complete")
        .body(format!("{} finished: {:?}", preset.name, current_input.file_name().unwrap_or_default()))
        .show();

    Ok(vec![current_input.to_string_lossy().to_string()])
}


