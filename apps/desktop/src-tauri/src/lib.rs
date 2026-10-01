use std::sync::Arc;
use tauri::{AppHandle, Manager, Emitter};
use tokio::sync::Mutex;
use tracing::{error, info};
use wheel_core::{ActionRegistry, JobQueue, WheelSettings, action::default_actions};

mod commands;
mod overlay;
mod tray;
pub mod sound;

/// The name of the app — single source of truth.
pub const APP_NAME: &str = wheel_core::action::APP_NAME;

/// Global application state accessible from Tauri commands.
pub struct AppState {
    pub settings: Arc<Mutex<WheelSettings>>,
    pub registry: Arc<ActionRegistry>,
    pub job_queue: Arc<JobQueue>,
    pub history: Arc<wheel_core::HistoryDb>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Set up structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wheel=debug,tauri=info".parse().unwrap()),
        )
        .init();

    info!("{} starting up", APP_NAME);

    // Build the action registry
    let mut registry = ActionRegistry::new();
    for manifest in default_actions() {
        registry.register(manifest);
    }
    let registry = Arc::new(registry);

    // Create the job queue (max 4 concurrent jobs)
    let (job_queue, _event_rx) = JobQueue::new(4);
    let job_queue = Arc::new(job_queue);

    // Set up persistent SQLite history and settings
    let local_app_data = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("APPDATA"))
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let local_path = std::path::PathBuf::from(local_app_data);
    let lime_dir = local_path.join("Lime");
    let old_wheel_dir = local_path.join("Wheel");
    let app_dir = if !lime_dir.exists() && old_wheel_dir.exists() {
        let _ = std::fs::rename(&old_wheel_dir, &lime_dir);
        if lime_dir.exists() { lime_dir } else { old_wheel_dir }
    } else {
        lime_dir
    };
    let db_path = app_dir.join("history.db");
    let history = wheel_core::HistoryDb::open(&db_path).unwrap_or_else(|e| {
        tracing::warn!("Failed to open persistent history at {:?}: {}, falling back to in-memory", db_path, e);
        wheel_core::HistoryDb::open_in_memory().expect("in-memory db must succeed")
    });
    let history = Arc::new(history);

    let settings_path = app_dir.join("settings.json");
    let mut initial_settings = WheelSettings::load_or_default(&settings_path);
    initial_settings.general.launch_at_login = wheel_win::shell::is_launch_at_login_registered();
    initial_settings.general.explorer_context_menu = wheel_win::shell::is_context_menu_registered();
    sound::set_sound_enabled(initial_settings.wheel_ui.sound_enabled);
    let settings = Arc::new(Mutex::new(initial_settings));

    let state = AppState {
        settings: Arc::clone(&settings),
        registry: Arc::clone(&registry),
        job_queue: Arc::clone(&job_queue),
        history: Arc::clone(&history),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance::init(|_app, args, _cwd| {
            info!("Second instance launched: {:?}", args);
            // Bring the existing instance to front or open a tool from args
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_denylist(&["overlay", "main"])
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::all()
                        & !tauri_plugin_window_state::StateFlags::VISIBLE,
                )
                .build(),
        )
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_actions,
            commands::get_settings,
            commands::save_settings,
            commands::show_overlay,
            commands::hide_overlay,
            commands::dispatch_action,
            commands::get_jobs,
            commands::get_history,
            commands::clear_history,
            commands::delete_history_item,
            commands::open_in_folder,
            commands::open_file,
            commands::trim_image_file,
            commands::convert_media_file,
            commands::get_ffmpeg_status,
            commands::open_settings_window,
            commands::is_explorer_context_menu_enabled,
            commands::set_explorer_context_menu,
            commands::run_preset,
            commands::pick_folder,
            commands::open_data_folder,
            commands::check_for_update,
            commands::download_and_install_update,
            commands::restart_app,
            commands::get_app_version,
            commands::play_hover_sound,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // Create the overlay window (pre-created, hidden)
            overlay::create_overlay_window(&handle)?;

            // Set up the system tray
            tray::setup_tray(&handle)?;

            // Start the Win32 hook thread
            let settings_clone = Arc::clone(&settings);
            let handle_clone = handle.clone();
            tauri::async_runtime::spawn(async move {
                start_hook_listener(handle_clone, settings_clone).await;
            });

            // Auto-updater: check for updates on launch in the background
            let updater_handle = handle.clone();
            let settings_for_updater = Arc::clone(&settings);
            tauri::async_runtime::spawn(async move {
                // Delay 5 seconds so startup remains instant
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                info!("Auto-updater: checking for updates on launch...");

                let (auto_update, include_prereleases) = {
                    let lock = settings_for_updater.lock().await;
                    (lock.general.auto_update, lock.general.include_prereleases)
                };

                if !auto_update {
                    info!("Auto-updater: auto-updates disabled in settings");
                    return;
                }

                match commands::resolve_update(&updater_handle, include_prereleases).await {
                    Ok(Some(update)) => {
                        let new_ver = update.version.clone();
                        info!("Auto-updater: update v{} available. Downloading and staging...", new_ver);
                            let progress_handle = updater_handle.clone();
                            let install_res = update
                                .download_and_install(
                                    move |chunk, total| {
                                        let _ = progress_handle.emit(
                                            "update-download-progress",
                                            serde_json::json!({
                                                "chunk_length": chunk,
                                                "content_length": total,
                                            }),
                                        );
                                    },
                                    || {
                                        info!("Auto-updater: installation staged");
                                    },
                                )
                                .await;

                            if install_res.is_ok() {
                                info!("Auto-updater: successfully staged Lime v{}", new_ver);
                                use tauri_plugin_notification::NotificationExt;
                                let _ = updater_handle
                                    .notification()
                                    .builder()
                                    .title("Lime — Update Ready")
                                    .body(format!("Lime v{} is ready! Click restart in Settings or relaunch to apply.", new_ver))
                                    .show();

                                let _ = updater_handle.emit(
                                    "update-downloaded",
                                    serde_json::json!({ "version": new_ver }),
                                );
                            } else if let Err(e) = install_res {
                                tracing::warn!("Auto-updater: download/install failed: {}", e);
                            }
                        }
                        Ok(None) => {
                            info!("Auto-updater: Lime is up to date");
                        }
                        Err(e) => {
                            tracing::debug!("Auto-updater: update check skipped: {}", e);
                        }
                    }
            });

            info!("{} setup complete", APP_NAME);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let app = window.app_handle();
                let state = app.state::<AppState>();
                let minimize_to_tray = {
                    if let Ok(guard) = state.settings.try_lock() {
                        guard.general.minimize_to_tray
                    } else {
                        true
                    }
                };

                if !minimize_to_tray {
                    let other_visible = app.webview_windows().into_iter().any(|(label, w)| {
                        label != window.label() && label != "overlay" && w.is_visible().unwrap_or(false)
                    });
                    if !other_visible {
                        tracing::info!("Closing last window with minimize_to_tray=false, exiting Lime");
                        app.exit(0);
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Starts the Win32 hook listener that drives the overlay.
async fn start_hook_listener(app: AppHandle, settings: Arc<Mutex<WheelSettings>>) {
    use wheel_win::WinEvent;

    let (threshold, always_show, modifier, paused) = {
        let s = settings.lock().await;
        (
            s.trigger.movement_threshold_px as i32,
            s.trigger.always_show,
            s.trigger.modifier.clone(),
            s.trigger.paused,
        )
    };
    wheel_win::hooks::set_threshold(threshold);
    wheel_win::hooks::set_always_show(always_show);
    wheel_win::hooks::set_modifier(modifier);
    wheel_win::hooks::set_paused(paused);

    let (mut rx, _hook_thread) = wheel_win::hooks::start_hooks(threshold);
    info!("Hook listener started");

    let mut current_overlay_rect: Option<(i32, i32, i32, i32)> = None;

    while let Some(event) = rx.recv().await {
        match event {
            WinEvent::DragArmed { x, y } => {
                let paused = {
                    let s = settings.lock().await;
                    s.trigger.paused
                };

                if !paused {
                    info!("Drag armed at ({}, {}), showing overlay", x, y);
                    wheel_win::hooks::set_drag_armed(true);
                    let _ = app.emit("drag-armed", serde_json::json!({ "x": x, "y": y }));

                    // Position and show the overlay centered on cursor
                    if let Some(overlay) = app.get_webview_window("overlay") {
                        let _ = overlay.emit("drag-armed", serde_json::json!({ "x": x, "y": y }));

                        // Center the overlay on cursor, clamped to work area
                        let overlay_size = 400i32;
                        let (ox, oy) = wheel_win::dpi::clamp_to_work_area(x, y, overlay_size, overlay_size);
                        current_overlay_rect = Some((ox, oy, overlay_size, overlay_size));
                        let _ = overlay.set_position(tauri::Position::Physical(
                            tauri::PhysicalPosition { x: ox, y: oy },
                        ));
                        let _ = overlay.show();
                    }
                }
            }
            WinEvent::DragCancelled | WinEvent::EscapePressed => {
                wheel_win::hooks::set_drag_armed(false);
                current_overlay_rect = None;
                let _ = app.emit("drag-cancelled", ());
                if let Some(overlay) = app.get_webview_window("overlay") {
                    let _ = overlay.emit("drag-cancelled", ());
                    let _ = overlay.hide();
                }
            }
            WinEvent::MouseMove { .. } => {
                // Pointer moves are handled inside the overlay window
                // via Chromium dragover and OLE drop target, avoiding IPC saturation.
            }
            WinEvent::TogglePage => {
                info!("Toggling wheel page from WinEvent");
                let _ = app.emit("toggle-page", ());
                if let Some(overlay) = app.get_webview_window("overlay") {
                    let _ = overlay.emit("toggle-page", ());
                }
            }
            WinEvent::LButtonChanged { pressed: false, x, y } => {
                let is_inside_overlay = if let Some((ox, oy, w, h)) = current_overlay_rect {
                    x >= ox && x <= ox + w && y >= oy && y <= oy + h
                } else {
                    false
                };

                current_overlay_rect = None;

                if !is_inside_overlay {
                    wheel_win::hooks::set_drag_armed(false);
                    let _ = app.emit("drag-cancelled", ());
                    if let Some(overlay) = app.get_webview_window("overlay") {
                        let _ = overlay.emit("drag-cancelled", ());
                        let _ = overlay.hide();
                    }
                } else {
                    // Released inside overlay: OLE drop will handle action dispatch.
                    // Keep overlay visible for confirmation timeout so the drop completes,
                    // then auto-hide.
                    let overlay_clone = app.get_webview_window("overlay");
                    let settings_clone = Arc::clone(&settings);
                    tauri::async_runtime::spawn(async move {
                        let timeout = {
                            let s = settings_clone.lock().await;
                            s.trigger.confirm_timeout_ms.max(50) as u64
                        };
                        tokio::time::sleep(std::time::Duration::from_millis(timeout)).await;
                        wheel_win::hooks::set_drag_armed(false);
                        if let Some(overlay) = overlay_clone {
                            let _ = overlay.hide();
                        }
                    });
                }
            }
            _ => {}
        }
    }

    error!("Hook listener channel closed unexpectedly");
}
