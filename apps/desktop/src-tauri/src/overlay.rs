use tauri::{AppHandle, WebviewWindowBuilder, WebviewUrl, Manager};
use tracing::info;

/// Create the radial wheel overlay window.
/// 
/// The overlay is:
/// - Pre-created and hidden at startup (fast show time)
/// - Transparent, borderless, always-on-top
/// - No taskbar entry
/// - Does not steal focus
/// - dragDropEnabled: false (we use our own IDropTarget)
pub fn create_overlay_window(app: &AppHandle) -> anyhow::Result<()> {
    info!("Creating overlay window");

    let overlay = WebviewWindowBuilder::new(
        app,
        "overlay",
        WebviewUrl::App("index.html?window=overlay".into()),
    )
    .title("Wheel Overlay")
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .inner_size(400.0, 400.0)
    .visible(false)
    .build()?;

    // On Windows, apply additional extended window styles for no-activate
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE,
            WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        };
        use windows::Win32::Foundation::HWND;

        if let Ok(hwnd) = overlay.hwnd() {
            unsafe {
                let hwnd = HWND(hwnd.0);
                let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(
                    hwnd,
                    GWL_EXSTYLE,
                    ex_style
                        | WS_EX_NOACTIVATE.0 as isize
                        | WS_EX_TOOLWINDOW.0 as isize,
                );
            }

            // Remove all Windows 11 DWM window frame artifacts (rounded box, border, backdrop material)
            wheel_win::vibrancy::make_overlay_transparent_frameless(hwnd.0 as isize);

            // Register our OLE drop target on the overlay HWND
            register_overlay_drop_target(app.clone(), hwnd.0 as isize);
        }
    }

    info!("Overlay window created (hidden)");
    Ok(())
}

#[cfg(target_os = "windows")]
fn register_overlay_drop_target(app: AppHandle, hwnd: isize) {
    use wheel_win::drop_target::{register_drop_target, DropEvent};
    use tauri::Emitter;

    let callback = Box::new(move |event: DropEvent| {
        if let Some(overlay) = app.get_webview_window("overlay") {
            match event {
                DropEvent::Enter { files, x, y } => {
                    let paths: Vec<String> = files
                        .iter()
                        .filter_map(|p| p.to_str().map(String::from))
                        .collect();
                    // Inspect extensions for context-aware filtering
                    let extensions: Vec<String> = files
                        .iter()
                        .filter_map(|p| {
                            p.extension()
                                .and_then(|e| e.to_str())
                                .map(|e| e.to_lowercase())
                        })
                        .collect();
                    let _ = overlay.emit(
                        "drop-enter",
                        serde_json::json!({
                            "files": paths,
                            "extensions": extensions,
                            "x": x,
                            "y": y,
                        }),
                    );
                    tracing::info!("OLE DragEnter: {:?}", paths);
                }
                DropEvent::Over { x, y } => {
                    use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
                    static LAST_X: AtomicI32 = AtomicI32::new(-9999);
                    static LAST_Y: AtomicI32 = AtomicI32::new(-9999);
                    static LAST_TIME: AtomicU64 = AtomicU64::new(0);

                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;

                    let last_x = LAST_X.load(Ordering::Relaxed);
                    let last_y = LAST_Y.load(Ordering::Relaxed);
                    let last_time = LAST_TIME.load(Ordering::Relaxed);

                    // Throttle to max 60Hz and only when position changes
                    if (x != last_x || y != last_y) && (now.saturating_sub(last_time) >= 16) {
                        LAST_X.store(x, Ordering::Relaxed);
                        LAST_Y.store(y, Ordering::Relaxed);
                        LAST_TIME.store(now, Ordering::Relaxed);
                        let _ = overlay.emit("drop-over", serde_json::json!({ "x": x, "y": y }));
                    }
                }
                DropEvent::Leave => {
                    let _ = overlay.emit("drop-leave", ());
                }
                DropEvent::Drop { files, x, y } => {
                    let paths: Vec<String> = files
                        .iter()
                        .filter_map(|p| p.to_str().map(String::from))
                        .collect();
                    let _ = overlay.emit(
                        "drop-files",
                        serde_json::json!({ "files": paths, "x": x, "y": y }),
                    );
                    tracing::info!("OLE Drop: {:?} at ({}, {})", paths, x, y);
                }
            }
        }
    });

    match register_drop_target(hwnd, callback) {
        Ok(handle) => {
            // Leak the handle intentionally — it lives for the lifetime of the app.
            // A proper solution stores it in AppState and drops it on cleanup.
            std::mem::forget(handle);
            tracing::info!("OLE drop target registered on overlay HWND {:x}", hwnd);
        }
        Err(e) => {
            tracing::error!("Failed to register drop target: {:?}", e);
        }
    }
}
