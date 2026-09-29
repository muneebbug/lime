use std::sync::Arc;
use tauri::{AppHandle, Manager, Emitter};
use tokio::sync::Mutex;
use tracing::{error, info};
use wheel_core::{ActionRegistry, JobQueue, WheelSettings, action::default_actions};

mod commands;
mod overlay;
mod tray;

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
        .unwrap_or_else(|_| ".".to_string());
    let wheel_dir = std::path::PathBuf::from(local_app_data).join("Wheel");
    let db_path = wheel_dir.join("history.db");
    let history = wheel_core::HistoryDb::open(&db_path).unwrap_or_else(|e| {
        tracing::warn!("Failed to open persistent history at {:?}: {}, falling back to in-memory", db_path, e);
        wheel_core::HistoryDb::open_in_memory().expect("in-memory db must succeed")
    });
    let history = Arc::new(history);

    let settings_path = wheel_dir.join("settings.json");
    let initial_settings = WheelSettings::load_or_default(&settings_path);
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
            commands::crop_image_file,
            commands::compress_image_file,
            commands::get_image_metadata,
            commands::strip_image_metadata,
            commands::add_background_file,
            commands::edit_image_file,
            commands::remove_background_file,
            commands::get_rmbg_model_status,
            commands::download_rmbg_model,
            commands::delete_rmbg_model,
            commands::redact_image_file,
            commands::annotate_image_file,
            commands::convert_media_file,
            commands::get_ffmpeg_status,
            commands::open_settings_window,
            commands::open_palette_window,
            commands::is_explorer_context_menu_enabled,
            commands::set_explorer_context_menu,
            commands::run_preset,
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

            info!("{} setup complete", APP_NAME);
            Ok(())
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
                    // Keep overlay visible for a brief moment so the drop completes,
                    // then auto-hide after 350ms.
                    let overlay_clone = app.get_webview_window("overlay");
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
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
