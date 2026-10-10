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
    let mut lock = state.settings.lock().await;
    lock.general.launch_at_login = wheel_win::shell::is_launch_at_login_registered();
    Ok(lock.clone())
}

/// Save updated settings
#[tauri::command]
pub async fn save_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings: WheelSettings,
) -> Result<(), String> {
    let updated = settings;

    let old_settings = {
        let mut lock = state.settings.lock().await;
        if *lock == updated {
            return Ok(());
        }
        let old = lock.clone();
        *lock = updated.clone();
        old
    };

    // Immediately update live Win32 hooks at runtime without requiring an app relaunch
    if updated.trigger.movement_threshold_px != old_settings.trigger.movement_threshold_px {
        wheel_win::hooks::set_threshold(updated.trigger.movement_threshold_px as i32);
    }
    if updated.trigger.always_show != old_settings.trigger.always_show {
        wheel_win::hooks::set_always_show(updated.trigger.always_show);
    }
    if updated.trigger.modifier != old_settings.trigger.modifier {
        wheel_win::hooks::set_modifier(updated.trigger.modifier.clone());
    }
    if updated.trigger.paused != old_settings.trigger.paused {
        wheel_win::hooks::set_paused(updated.trigger.paused);
    }

    // Synchronize Windows startup registration (Launch at Windows Login) ONLY IF CHANGED
    if updated.general.launch_at_login != old_settings.general.launch_at_login {
        if let Err(e) = wheel_win::shell::set_launch_at_login(updated.general.launch_at_login) {
            tracing::warn!("Failed to synchronize launch at login: {}", e);
        }
    }

    // Synchronize live audio effects toggle
    crate::sound::set_sound_enabled(updated.wheel_ui.sound_enabled);

    // Republish the FFmpeg override so conversions in flight pick up a path the
    // user just chose or cleared, without needing a relaunch.
    wheel_engines::media::set_custom_ffmpeg_path(
        updated.ffmpeg.custom_path.as_ref().map(std::path::PathBuf::from),
    );

    // Broadcast settings update to frontend windows
    let _ = app.emit("settings-updated", &updated);

    let settings_path = crate::settings_file_path();
    if let Err(e) = updated.save_to_path(&settings_path) {
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
            // Let the detached HUD show progress before any work begins.
            emit_job_started(&app, action_id, &files);

            // Queue a background job
            let job = wheel_core::Job::new(action_id.clone(), files, request.params);
            let _ = state.history.insert_job(&job);

            let job_id = state
                .job_queue
                .enqueue(job)
                .await
                .map_err(|e| e.to_string())?;

            crate::status::show(&app);

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

/// Announce a job to the detached status HUD (and any other listener).
///
/// Emitted before the job is queued, so the HUD appears the instant the user
/// lets go of a petal rather than after engine startup.
fn emit_job_started(app: &tauri::AppHandle, action_id: &str, files: &[PathBuf]) {
    let names: Vec<String> = files
        .iter()
        .filter_map(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .collect();

    let _ = app.emit(
        "job-started",
        serde_json::json!({
            "action_id": action_id,
            "files": names,
        }),
    );
}

/// Whether an output extension carries audio only.
///
/// Selects which of the per-media-type metadata settings applies to a job.
fn is_audio_target(ext: &str) -> bool {
    matches!(
        ext,
        "mp3" | "wav" | "flac" | "m4a" | "aac" | "ogg" | "opus" | "wma"
    )
}

fn get_in_flight_tools() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static IN_FLIGHT: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    IN_FLIGHT.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

/// Where a tool window should open.
///
/// `WindowBuilder::center()` centres on the *primary* monitor, so on a multi-
/// monitor setup a tool opened from a secondary display appeared somewhere the
/// user was not looking, and often behind the window they had just dragged from.
///
/// Centring on the anchor rather than the primary display is the whole fix;
/// `clamp_to_work_area` additionally keeps the window inside the work area so it
/// cannot open half off a screen or under a taskbar.
/// The overlay is the fallback; the cursor is the truth.
///
/// The live cursor wins because it is literally where the user is, and it still
/// moves after the wheel appears. The overlay is second: `show_overlay` puts it
/// at the cursor and clamps it into that monitor's work area, so its centre is
/// a good answer when the cursor cannot be read.
fn tool_window_anchor(app: &tauri::AppHandle) -> (i32, i32) {
    if let Ok(cursor) = app.cursor_position() {
        return (cursor.x as i32, cursor.y as i32);
    }
    if let Some(overlay) = app.get_webview_window("overlay") {
        if let (Ok(position), Ok(size)) = (overlay.outer_position(), overlay.outer_size()) {
            return (
                position.x + size.width as i32 / 2,
                position.y + size.height as i32 / 2,
            );
        }
    }
    (0, 0)
}

/// Top-left for a window of this size, centred on the active monitor.
fn tool_window_position(app: &tauri::AppHandle, width: i32, height: i32) -> (i32, i32) {
    let (anchor_x, anchor_y) = tool_window_anchor(app);
    wheel_win::dpi::clamp_to_work_area(anchor_x, anchor_y, width, height)
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
        // Already-open windows still follow the user. A tool left on the left
        // monitor should come to the one they are now standing at.
        if let Ok(size) = win.outer_size() {
            let (x, y) = tool_window_position(&app, size.width as i32, size.height as i32);
            let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
        }
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

    let (window_x, window_y) =
        tool_window_position(&app, win_cfg.width as i32, win_cfg.height as i32);

    let win_res = tauri::WebviewWindowBuilder::new(app, &window_id, WebviewUrl::App(url.into()))
        .title(&manifest.title)
        .inner_size(win_cfg.width as f64, win_cfg.height as f64)
        .position(window_x as f64, window_y as f64)
        .decorations(false)
        .transparent(true)
        .resizable(win_cfg.resizable)
        .maximizable(false)
        .focused(true)
        .build();

    {
        let mut guard = in_flight.lock().unwrap();
        guard.remove(&window_id);
    }

    let win = win_res?;

    // Settings and onboarding both clear the DWM border and let the frame helper
    // finish the transparent-window treatment, so their CSS corner radius is the
    // only edge the user sees. A tool window that skipped it showed the Windows
    // corner rounding sitting on top of the app's own radius.
    #[cfg(target_os = "windows")]
    if let Ok(hwnd) = win.hwnd() {
        wheel_win::vibrancy::make_overlay_transparent_frameless(hwnd.0 as isize);
    }

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
    let is_trim = action_id == "tool.trim";

    let mut outputs = Vec::new();
    let mut error: Option<String> = None;

    let output_settings = {
        let state = app.state::<crate::AppState>();
        let out = state.settings.lock().await.output.clone();
        out
    };

    let fixed_folder = output_settings
        .fixed_folder
        .as_ref()
        .map(std::path::Path::new);

    // The one destination for every output below. "Ask every time" was removed
    // because a dialog raised from this background job could not be trusted to
    // come forward or to return a value, so the folder is now always a setting.
    let effective_folder = fixed_folder;

    if is_trim {
        for input in &job.inputs {
            let input_ext = input
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png")
                .trim_start_matches('.')
                .to_lowercase();

            let output_path = wheel_core::output::resolve_output_path(
                input,
                &input_ext,
                if output_settings.suffix.is_empty() { ".trimmed" } else { &output_settings.suffix },
                &output_settings.policy,
                effective_folder,
                output_settings.overwrite_source,
            );

            let in_clone = input.clone();
            let out_clone = output_path.clone();

            let result = tokio::task::spawn_blocking(move || {
                wheel_engines::trim::trim_image(&in_clone, &out_clone)
            })
            .await;

            match result {
                Ok(Ok(res)) => outputs.push(res.output_path),
                Ok(Err(e)) => {
                    error = Some(e.to_string());
                    break;
                }
                Err(e) => {
                    error = Some(e.to_string());
                    break;
                }
            }
        }
    } else if target_ext == "pdf" {
        for input in &job.inputs {
            let output_path = wheel_core::output::resolve_output_path(
                input,
                "pdf",
                &output_settings.suffix,
                &output_settings.policy,
                effective_folder,
                output_settings.overwrite_source,
            );
            let in_clone = vec![input.clone()];
            let out_clone = output_path.clone();

            let result = tokio::task::spawn_blocking(move || {
                wheel_engines::pdf::images_to_pdf(&in_clone, &out_clone)
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
        }
    } else {
        for input in &job.inputs {
            let output_path = wheel_core::output::resolve_output_path(
                input,
                &target_ext,
                &output_settings.suffix,
                &output_settings.policy,
                effective_folder,
                output_settings.overwrite_source,
            );

            let input_ext = input
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .trim_start_matches('.')
                .to_lowercase();
            let input_is_media = matches!(
                input_ext.as_str(),
                "mp4" | "webm"
                    | "mov"
                    | "mkv"
                    | "avi"
                    | "m4v"
                    | "wmv"
                    | "flv"
                    | "mp3"
                    | "wav"
                    | "flac"
                    | "m4a"
                    | "aac"
                    | "ogg"
                    | "opus"
                    | "wma"
            );
            let target_is_media = matches!(
                target_ext.as_str(),
                "mp4" | "webm"
                    | "mov"
                    | "mkv"
                    | "avi"
                    | "gif"
                    | "mp3"
                    | "wav"
                    | "m4a"
                    | "flac"
                    | "aac"
                    | "ogg"
                    | "opus"
                    | "wma"
            );

            let image_fmt = if input_is_media {
                None
            } else {
                wheel_engines::image_convert::OutputFormat::from_extension(&target_ext)
            };

            if let Some(fmt) = image_fmt {
                let input_clone = input.clone();
                let output_clone = output_path.clone();
                let params = wheel_engines::image_convert::ConvertParams {
                    output_format: fmt,
                    output_path: output_clone.clone(),
                    preserve_metadata: output_settings.metadata.images,
                };
                let result = tokio::task::spawn_blocking(move || -> anyhow::Result<PathBuf> {
                    let out = wheel_engines::image_convert::convert_image(&input_clone, &params)?;
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
            } else if target_is_media {
                let input_clone = input.clone();
                let output_clone = output_path.clone();
                let fmt_clone = target_ext.clone();
                let preserve_metadata = if is_audio_target(&target_ext) {
                    output_settings.metadata.audio
                } else {
                    output_settings.metadata.video
                };

                let result = tokio::task::spawn_blocking(move || {
                    wheel_engines::media::convert_media_with(
                        &input_clone,
                        &output_clone,
                        &fmt_clone,
                        wheel_engines::media::MediaOptions {
                            preserve_metadata,
                        },
                    )
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
            .title("Lime — Conversion failed")
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

        let notif_title = if is_trim {
            "Lime — Trim complete".to_string()
        } else {
            format!("Lime — {} complete", target_ext.to_uppercase())
        };
        let notif_body = if is_trim {
            format!("Trimmed blank pixels from {} file(s) successfully", outputs.len())
        } else {
            format!("Converted {} file(s) successfully", outputs.len())
        };

        let _ = app
            .notification()
            .builder()
            .title(notif_title)
            .body(notif_body)
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

    let mut cmd = std::process::Command::new("cmd");
    cmd.args(["/C", "start", "", &path]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.spawn().map_err(|e| e.to_string())?;

    Ok(())
}

/// Trim all blank (transparent) pixels from an image file (matches Photoshop Trim)
#[tauri::command]
pub async fn trim_image_file(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input_path: String,
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
                if output_settings.suffix.is_empty() { ".trimmed" } else { &output_settings.suffix },
                &output_settings.policy,
                fixed_folder,
                output_settings.overwrite_source,
            )
        }
    };

    let in_clone = input.clone();
    let out_clone = target_out.clone();

    let res = tokio::task::spawn_blocking(move || {
        wheel_engines::trim::trim_image(&in_clone, &out_clone)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    // Record job in history
    let mut job = wheel_core::Job::new("tool.trim", vec![input], serde_json::json!({
        "trimmed": res.bounds.needs_trim,
        "original_width": res.bounds.original_width,
        "original_height": res.bounds.original_height,
        "trimmed_width": res.bounds.trimmed_width,
        "trimmed_height": res.bounds.trimmed_height,
    }));
    job.status = wheel_core::job::JobStatus::Completed;
    job.progress = 1.0;
    job.outputs = vec![res.output_path.clone()];
    let _ = state.history.insert_job(&job);

    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder()
        .title("Lime — Trim complete")
        .body(format!("Trimmed image saved to {:?}", res.output_path.file_name().unwrap_or_default()))
        .show();

    Ok(res.output_path.to_string_lossy().to_string())
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
    let preserve_metadata = if is_audio_target(&target_format) {
        output_settings.metadata.audio
    } else {
        output_settings.metadata.video
    };

    let res = tokio::task::spawn_blocking(move || {
        wheel_engines::media::convert_media_with(
            &input_clone,
            &out_clone,
            &fmt_clone,
            wheel_engines::media::MediaOptions {
                preserve_metadata,
            },
        )
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
        .title("Lime — Media conversion complete")
        .body(format!("Converted media saved to {:?}", res.file_name().unwrap_or_default()))
        .show();

    Ok(res.to_string_lossy().to_string())
}

/// Everything the UI needs to describe FFmpeg: where it is, whether media
/// conversion works, and where Lime would install its own copy.
#[tauri::command]
pub async fn get_ffmpeg_report(state: State<'_, AppState>) -> Result<crate::ffmpeg::FfmpegReport, String> {
    let lock = state.settings.lock().await;
    Ok(crate::ffmpeg::report(
        lock.ffmpeg.custom_path.as_deref(),
        lock.ffmpeg.managed_version.as_deref(),
    ))
}

/// Download and install FFmpeg into `%LOCALAPPDATA%\Lime\bin`.
///
/// Runs in the background and reports through `ffmpeg-download-progress` and
/// `ffmpeg-download-finished`, so a slow link never blocks the UI thread.
#[tauri::command]
pub async fn download_ffmpeg(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::ffmpeg::download_and_install(&app).await {
            tracing::warn!("FFmpeg download failed: {e}");
            let _ = app.emit(
                crate::ffmpeg::EVENT_FINISHED,
                serde_json::json!({ "ok": false, "error": e }),
            );
        }
    });
    Ok(())
}

/// Confirm a user-chosen path really is a working `ffmpeg.exe` before saving it.
///
/// Saves the user from pointing Lime at a folder or a build that will not run.
#[tauri::command]
pub async fn validate_ffmpeg_path(path: String) -> Result<serde_json::Value, String> {
    let p = std::path::PathBuf::from(&path);
    if !p.is_file() {
        return Ok(serde_json::json!({ "ok": false, "reason": "That file does not exist." }));
    }
    match wheel_engines::media::probe_ffmpeg(&p) {
        Some(version) => Ok(serde_json::json!({ "ok": true, "version": version })),
        None => Ok(serde_json::json!({
            "ok": false,
            "reason": "That file did not run as FFmpeg."
        })),
    }
}

/// Delete Lime's downloaded copy of FFmpeg, e.g. to reclaim disk space.
#[tauri::command]
pub async fn remove_managed_ffmpeg() -> Result<(), String> {
    crate::ffmpeg::reset(None, true);
    Ok(())
}

/// Replay onboarding on demand, from Settings.
///
/// Does not reset the flag first; the window closing normally marks it complete,
/// so reopening and closing is harmless.
#[tauri::command]
pub async fn open_onboarding(app: tauri::AppHandle) -> Result<(), String> {
    crate::onboarding::show(&app);
    Ok(())
}

#[tauri::command]
pub async fn hide_onboarding(app: tauri::AppHandle) -> Result<(), String> {
    // Closing counts as finishing: record it so a dismissed window does not
    // reappear on the next launch.
    crate::onboarding::complete(&app);
    Ok(())
}

/// Open the Settings window
#[tauri::command]
pub async fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::tray::open_settings_window(&app);
    Ok(())
}

/// Open Settings directly on a specific page.
///
/// Used by the "FFmpeg required" HUD prompt so the user lands on the fix rather
/// than having to hunt for it.
#[tauri::command]
pub async fn open_settings_page(app: tauri::AppHandle, page: String) -> Result<(), String> {
    use tauri::Manager;

    crate::tray::open_settings_window(&app);

    // Give the window a moment to load its frontend before asking it to
    // navigate; emitting into a page that has not mounted yet is dropped.
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        if let Some(win) = app.get_webview_window("settings") {
            let _ = win.emit("settings-navigate", page);
        }
    });
    Ok(())
}

/// Show the detached status/progress HUD.
#[tauri::command]
pub async fn show_status(app: tauri::AppHandle) -> Result<(), String> {
    crate::status::show(&app);
    Ok(())
}

/// Hide the detached status/progress HUD.
#[tauri::command]
pub async fn hide_status(app: tauri::AppHandle) -> Result<(), String> {
    crate::status::hide(&app);
    Ok(())
}

/// Re-anchor the status/progress HUD after the user changes its position.
#[tauri::command]
pub async fn apply_status_position(
    app: tauri::AppHandle,
    vertical: wheel_core::StatusVertical,
    horizontal: wheel_core::StatusHorizontal,
) -> Result<(), String> {
    crate::status::apply_position(&app, vertical, horizontal).map_err(|e| e.to_string())
}

/// Prompt the user to pick a folder using the native Windows dialog
#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let owner_hwnd = owner_window_handle(&app);
    if let Some(hwnd) = owner_hwnd {
        #[cfg(target_os = "windows")]
        wheel_win::force_focus_window(hwnd);
    }

    tokio::task::spawn_blocking(move || {
        wheel_win::shell::pick_folder_owned(owner_hwnd, "Choose a folder")
    })
    .await
    .map_err(|e| e.to_string())?
    .map(|opt| opt.map(|p| p.to_string_lossy().to_string()))
    .map_err(|e| e.to_string())
}

/// The window a native dialog should belong to.
///
/// Prefers the overlay, falling back to Settings, so the dialog always has an
/// owner. A dialog with no owner is unowned: Windows gives it no z-order
/// relationship to us, which is why it used to open behind the app and steal no
/// focus.
fn owner_window_handle(app: &tauri::AppHandle) -> Option<isize> {
    #[cfg(target_os = "windows")]
    {
        use tauri::Manager;

        let visible = |label: &str| {
            app.get_webview_window(label)
                .map(|w| w.is_visible().unwrap_or(false))
                .unwrap_or(false)
        };

        // The overlay is what the user was just dragging from; Settings is the
        // fallback when the dialog comes from a button in the app itself.
        let label = if visible("overlay") {
            "overlay"
        } else if visible("settings") {
            "settings"
        } else {
            "overlay"
        };

        return app
            .get_webview_window(label)
            .and_then(|w| w.hwnd().ok())
            .map(|hwnd| hwnd.0 as isize);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        None
    }
}

/// Open the %LOCALAPPDATA%\Lime data directory in Windows Explorer
#[tauri::command]
pub async fn open_data_folder() -> Result<(), String> {
    let local_app_data = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("APPDATA"))
        .unwrap_or_else(|_| ".".to_string());
    let path = std::path::PathBuf::from(local_app_data).join("Lime");
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
        let _params = step.params.clone();
        let ext_clone = ext.clone();
        let preserve_metadata = output_settings.metadata.images;

        let step_res = tokio::task::spawn_blocking(move || -> anyhow::Result<PathBuf> {
            if action_id == "tool.trim" {
                let r = wheel_engines::trim::trim_image(&in_clone, &out_clone)?;
                Ok(r.output_path)
            } else if let Some(fmt) = wheel_engines::image_convert::OutputFormat::from_extension(&ext_clone) {
                let p = wheel_engines::image_convert::ConvertParams {
                    output_format: fmt,
                    output_path: out_clone.clone(),
                    preserve_metadata,
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
        .title("Lime — Preset Complete")
        .body(format!("{} finished: {:?}", preset.name, current_input.file_name().unwrap_or_default()))
        .show();

    Ok(vec![current_input.to_string_lossy().to_string()])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub update_available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub body: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    prerelease: bool,
    draft: bool,
    assets: Vec<GithubReleaseAsset>,
}

/// Dynamically resolve the appropriate update release from GitHub API with fallback
pub async fn resolve_update(
    app: &tauri::AppHandle,
    include_prereleases: bool,
) -> Result<Option<tauri_plugin_updater::Update>, String> {
    use tauri_plugin_updater::UpdaterExt;
    use url::Url;

    // 1. Try querying GitHub Releases API to respect release channels (Stable vs Pre-release)
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build();

    if let Ok(client) = client {
        let res = client
            .get("https://api.github.com/repos/muneebbug/lime/releases?per_page=15")
            .header("User-Agent", "Lime-App")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await;

        if let Ok(response) = res {
            if response.status().is_success() {
                if let Ok(releases) = response.json::<Vec<GithubRelease>>().await {
                    for rel in releases {
                        if rel.draft {
                            continue;
                        }
                        // Skip pre-releases if user only wants stable releases
                        if !include_prereleases && rel.prerelease {
                            continue;
                        }

                        info!("Auto-updater checking candidate release: {} (prerelease: {})", rel.tag_name, rel.prerelease);

                        if let Some(asset) = rel.assets.into_iter().find(|a| a.name == "latest.json") {
                            if let Ok(target_url) = Url::parse(&asset.browser_download_url) {
                                if let Ok(builder) = app.updater_builder().endpoints(vec![target_url]) {
                                    if let Ok(updater) = builder.build() {
                                        if let Ok(update_opt) = updater.check().await {
                                            return Ok(update_opt);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Fallback to default configured endpoint in tauri.conf.json
    let updater = app.updater().map_err(|e| e.to_string())?;
    updater.check().await.map_err(|e| e.to_string())
}

/// Check for application updates against the GitHub Releases updater manifest
#[tauri::command]
pub async fn check_for_update(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<UpdateCheckResult, String> {
    let include_prereleases = {
        let lock = state.settings.lock().await;
        lock.general.include_prereleases
    };

    let update = resolve_update(&app, include_prereleases).await?;

    let current_version = app.package_info().version.to_string();
    if let Some(update) = update {
        Ok(UpdateCheckResult {
            update_available: true,
            current_version,
            latest_version: Some(update.version.clone()),
            body: update.body.clone(),
            date: update.date.map(|d| d.to_string()),
        })
    } else {
        Ok(UpdateCheckResult {
            update_available: false,
            current_version,
            latest_version: None,
            body: None,
            date: None,
        })
    }
}

/// Download and install the available update
#[tauri::command]
pub async fn download_and_install_update(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let include_prereleases = {
        let lock = state.settings.lock().await;
        lock.general.include_prereleases
    };

    let update = resolve_update(&app, include_prereleases)
        .await?
        .ok_or_else(|| "No update available".to_string())?;

    let version = update.version.clone();
    let app_clone = app.clone();

    update
        .download_and_install(
            move |chunk, total| {
                let _ = app_clone.emit(
                    "update-download-progress",
                    serde_json::json!({
                        "chunk_length": chunk,
                        "content_length": total,
                    }),
                );
            },
            || {
                info!("Update installation staged; ready to apply on restart");
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    use tauri_plugin_notification::NotificationExt;
    let _ = app
        .notification()
        .builder()
        .title("Lime — Update Ready")
        .body(format!("Lime v{} is ready. Restart to apply!", version))
        .show();

    let _ = app.emit(
        "update-downloaded",
        serde_json::json!({ "version": version }),
    );

    Ok(version)
}

/// Restart the application immediately to apply updates
#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) {
    app.restart();
}

/// Return the application version
#[tauri::command]
pub fn get_app_version(app: tauri::AppHandle) -> String {
    format!("v{}", app.package_info().version)
}

/// Play native hover sound when a tool or extension wedge is hovered
#[tauri::command]
pub async fn play_hover_sound(state: State<'_, AppState>) -> Result<(), ()> {
    let settings = state.settings.lock().await;
    if settings.wheel_ui.sound_enabled {
        crate::sound::play_hover_sound();
    }
    Ok(())
}



