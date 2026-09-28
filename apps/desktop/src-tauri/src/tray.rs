use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle,
};
use tracing::info;

pub fn setup_tray(app: &AppHandle) -> anyhow::Result<()> {
    let pause_item = MenuItem::with_id(app, "pause", "Pause Wheel", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Open Settings", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = PredefinedMenuItem::quit(app, Some("Quit Wheel"))?;

    let menu = Menu::with_items(
        app,
        &[&pause_item, &settings_item, &separator, &quit_item],
    )?;

    let _tray = TrayIconBuilder::with_id("wheel-tray")
        .tooltip("Wheel — drag files to convert and edit")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "pause" => {
                info!("Tray: pause/resume toggled");
            }
            "settings" => {
                info!("Tray: open settings");
                open_settings_window(app);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                info!("Tray icon left-clicked");
                open_settings_window(tray.app_handle());
            }
        })
        .build(app)?;

    info!("System tray created");
    Ok(())
}

pub fn open_settings_window(app: &AppHandle) {
    use tauri::Manager;
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }

    let _ = tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("index.html?window=settings".into()),
    )
    .title("Wheel Settings")
    .inner_size(900.0, 680.0)
    .decorations(false)
    .transparent(true)
    .resizable(true)
    .center()
    .build();
}

pub fn open_palette_window(app: &AppHandle) {
    use tauri::Manager;
    if let Some(win) = app.get_webview_window("palette") {
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }

    let _ = tauri::WebviewWindowBuilder::new(
        app,
        "palette",
        tauri::WebviewUrl::App("index.html?window=palette".into()),
    )
    .title("Wheel Command Palette")
    .inner_size(640.0, 480.0)
    .decorations(false)
    .transparent(true)
    .resizable(false)
    .center()
    .always_on_top(true)
    .build();
}
