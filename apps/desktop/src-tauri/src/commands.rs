use std::path::PathBuf;
use tauri::{Manager, State};
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
    let settings = state.settings.lock().await.clone();
    Ok(settings)
}

/// Save updated settings
#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    settings: WheelSettings,
) -> Result<(), String> {
    let migrated = settings.migrate();
    *state.settings.lock().await = migrated;
    // TODO: persist to %APPDATA%\Wheel\settings.json via tauri-plugin-store
    info!("Settings saved");
    Ok(())
}

/// Show the overlay at the specified cursor position
#[tauri::command]
pub async fn show_overlay(
    app: tauri::AppHandle,
    x: i32,
    y: i32,
) -> Result<(), String> {
    if let Some(overlay) = app.get_webview_window("overlay") {
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
    if let Some(overlay) = app.get_webview_window("overlay") {
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
    if app.get_webview_window(&window_id).is_some() {
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(app, &window_id, WebviewUrl::App(url.into()))
        .title(&manifest.title)
        .inner_size(win_cfg.width as f64, win_cfg.height as f64)
        .decorations(false)
        .transparent(true)
        .resizable(win_cfg.resizable)
        .center()
        .build()?;

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

    if target_ext == "pdf" {
        if !job.inputs.is_empty() {
            let fixed_folder = output_settings.fixed_folder.as_ref().map(std::path::Path::new);
            let output_path = wheel_core::output::resolve_output_path(
                &job.inputs[0],
                "pdf",
                &output_settings.suffix,
                &output_settings.policy,
                fixed_folder,
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

            let fixed_folder = output_settings.fixed_folder.as_ref().map(std::path::Path::new);
            let output_path = wheel_core::output::resolve_output_path(
                input,
                &target_ext,
                &output_settings.suffix,
                &output_settings.policy,
                fixed_folder,
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

                let result = tokio::task::spawn_blocking(move || {
                    wheel_engines::image_convert::convert_image(&input_clone, &params)
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
