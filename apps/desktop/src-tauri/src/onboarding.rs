//! First-run onboarding window.
//!
//! Shown once, on the first launch after install, and never again once the user
//! reaches the final step. Skipping is allowed at every point, so an interrupted
//! or declined setup never blocks the app.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tracing::info;

const ONBOARDING_WINDOW: &str = "onboarding";

/// Label of the onboarding window, also matched by the global close handler.
pub const WINDOW_LABEL: &str = ONBOARDING_WINDOW;

const WIDTH: f64 = 720.0;
const HEIGHT: f64 = 560.0;

/// Create the window up front, hidden, so showing it costs nothing later.
///
/// Pre-creating matches the overlay and status windows: none of them should make
/// the first frame of a conversion slower.
pub fn create_window(app: &AppHandle) -> anyhow::Result<()> {
    if app.get_webview_window(ONBOARDING_WINDOW).is_some() {
        return Ok(());
    }

    WebviewWindowBuilder::new(
        app,
        ONBOARDING_WINDOW,
        WebviewUrl::App("index.html?window=onboarding".into()),
    )
    .title("Welcome to Lime")
    .inner_size(WIDTH, HEIGHT)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(false)
    // Must match the settings window: the rounded corners are drawn in CSS, and
    // without this the opaque native background shows through them.
    .transparent(true)
    .center()
    .visible(false)
    .build()?;

    #[cfg(target_os = "windows")]
    if let Some(win) = app.get_webview_window(ONBOARDING_WINDOW) {
        if let Ok(hwnd) = win.hwnd() {
            // Same DWM cleanup the settings window gets, so the two round and
            // border identically.
            wheel_win::vibrancy::make_overlay_transparent_frameless(hwnd.0 as isize);
        }
    }

    info!("Onboarding window created (hidden)");
    Ok(())
}

/// Show onboarding, unless the user has already finished it.
///
/// `settings` is read by the caller so this stays a pure window operation.
pub fn show_if_needed(app: &AppHandle, onboarding_completed: bool) {
    if onboarding_completed {
        return;
    }
    show(app);
}

pub fn show(app: &AppHandle) {
    let Some(win) = app.get_webview_window(ONBOARDING_WINDOW) else {
        return;
    };
    if let Err(e) = win.show() {
        tracing::warn!("Could not show onboarding: {e}");
        return;
    }
    let _ = win.unminimize();
    let _ = win.set_focus();
    #[cfg(target_os = "windows")]
    if let Ok(hwnd) = win.hwnd() {
        wheel_win::force_focus_window(hwnd.0 as isize);
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(ONBOARDING_WINDOW) {
        let _ = win.hide();
    }
}

/// Dismiss onboarding for good: record that it ran, then hide.
///
/// Called from both the in-app Done button and the close button. The window is
/// always hidden rather than closed, because it is created once at startup and
/// has to stay available for "Replay setup"; destroying it would leave a stale
/// window handle behind for the webview to keep posting to.
pub fn complete(app: &AppHandle) {
    use tauri::Manager;

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<crate::AppState>();
        let updated = {
            let mut lock = state.settings.lock().await;
            if lock.general.onboarding_completed {
                None
            } else {
                lock.general.onboarding_completed = true;
                Some(lock.clone())
            }
        };

        if let Some(settings) = updated {
            let path = crate::settings_file_path();
            if let Err(e) = settings.save_to_path(&path) {
                tracing::warn!("Could not persist onboarding completion to {path:?}: {e}");
            }
        }

        hide(&app);
    });
}
