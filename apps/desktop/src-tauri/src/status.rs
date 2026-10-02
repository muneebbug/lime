use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder};
use tracing::info;
use wheel_core::{StatusHorizontal, StatusVertical};

/// Label of the detached status/progress window.
const STATUS_WINDOW: &str = "status";

/// HUD width. Height is generous enough for the longest error message we render.
const HUD_WIDTH: f64 = 320.0;
const HUD_HEIGHT: f64 = 104.0;

/// Gap between the HUD and the work-area edge, so it never sits flush against
/// the taskbar or a screen border.
const EDGE_MARGIN: f64 = 24.0;

/// Create the detached status/progress window.
///
/// Like the overlay it is pre-created and hidden at startup so showing it during
/// a conversion costs nothing. It is transparent, borderless, always-on-top,
/// absent from the taskbar, and must never steal focus from the user's app.
pub fn create_status_window(app: &AppHandle) -> anyhow::Result<()> {
    info!("Creating status window");

    let hud = WebviewWindowBuilder::new(
        app,
        STATUS_WINDOW,
        WebviewUrl::App("index.html?window=status".into()),
    )
    .title("Lime Status")
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(false)
    .inner_size(HUD_WIDTH, HUD_HEIGHT)
    .visible(false)
    .build()?;

    #[cfg(target_os = "windows")]
    if let Ok(hwnd) = hud.hwnd() {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        };

        unsafe {
            let hwnd = HWND(hwnd.0);
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                ex_style | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize,
            );
        }

        // Strip Win11 rounded corners, border and Mica so only our pixels render.
        wheel_win::vibrancy::make_overlay_transparent_frameless(hwnd.0 as isize);
    }

    info!("Status window created (hidden)");
    Ok(())
}

/// Move the HUD to the anchor described by the user's vertical/horizontal choice.
///
/// The window's *centre* is placed at the anchor point so the HUD straddles the
/// chosen edge/centre line rather than being pushed flush against it.
pub fn apply_position(
    app: &AppHandle,
    vertical: StatusVertical,
    horizontal: StatusHorizontal,
) -> anyhow::Result<()> {
    let hud = match app.get_webview_window(STATUS_WINDOW) {
        Some(w) => w,
        None => return Ok(()),
    };

    let (left, top, right, bottom) = wheel_win::dpi::work_area_for_point(0, 0);
    let (wa_w, wa_h) = (right - left, bottom - top);

    let x = left as f64 + wa_w as f64 * horizontal.anchor() as f64;
    let y = top as f64 + wa_h as f64 * vertical.anchor() as f64;

    // Pull the HUD fully inside the work area regardless of anchor.
    let min_x = left as f64 + EDGE_MARGIN + HUD_WIDTH / 2.0;
    let max_x = right as f64 - EDGE_MARGIN - HUD_WIDTH / 2.0;
    let min_y = top as f64 + EDGE_MARGIN + HUD_HEIGHT / 2.0;
    let max_y = bottom as f64 - EDGE_MARGIN - HUD_HEIGHT / 2.0;

    let cx = x.clamp(min_x.min(max_x), max_x.max(min_x));
    let cy = y.clamp(min_y.min(max_y), max_y.max(min_y));

    hud.set_position(LogicalPosition::new(cx - HUD_WIDTH / 2.0, cy - HUD_HEIGHT / 2.0))?;
    let _ = hud.set_size(LogicalSize::new(HUD_WIDTH, HUD_HEIGHT));

    info!(
        "Status window positioned at {:?} / {:?} -> ({:.0}, {:.0})",
        vertical, horizontal, cx, cy
    );
    Ok(())
}

/// Show the HUD and (re)apply its anchor. Called when a job starts.
pub fn show(app: &AppHandle) {
    let Some(hud) = app.get_webview_window(STATUS_WINDOW) else {
        return;
    };
    let _ = hud.show();
}

/// Hide the HUD. Called on job completion, failure or user dismissal.
pub fn hide(app: &AppHandle) {
    let Some(hud) = app.get_webview_window(STATUS_WINDOW) else {
        return;
    };
    let _ = hud.hide();
}