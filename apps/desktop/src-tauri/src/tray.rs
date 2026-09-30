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

    let mut builder = TrayIconBuilder::with_id("wheel-tray")
        .tooltip("Wheel — drag files to convert and edit")
        .menu(&menu)
        .show_menu_on_left_click(false);

    if let Some(icon) = get_tray_icon() {
        builder = builder.icon(icon);
    } else if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    let _tray = builder
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

pub fn get_app_icon() -> Option<tauri::image::Image<'static>> {
    let img = image::load_from_memory(include_bytes!("../icons/128x128.png"))
        .or_else(|_| image::load_from_memory(include_bytes!("../icons/32x32.png")))
        .ok()?
        .to_rgba8();
    let (w, h) = img.dimensions();
    Some(tauri::image::Image::new_owned(img.into_raw(), w, h))
}

pub fn get_tray_icon() -> Option<tauri::image::Image<'static>> {
    let img = image::load_from_memory(include_bytes!("../icons/32x32.png")).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(tauri::image::Image::new_owned(img.into_raw(), w, h))
}

pub fn open_settings_window(app: &AppHandle) {
    use tauri::Manager;
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        #[cfg(target_os = "windows")]
        if let Ok(hwnd) = win.hwnd() {
            wheel_win::force_focus_window(hwnd.0 as isize);
        }
        return;
    }

    let mut builder = tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("index.html?window=settings".into()),
    )
    .title("Wheel Settings")
    .inner_size(900.0, 680.0)
    .decorations(false)
    .transparent(true)
    .resizable(true)
    .maximizable(false)
    .center()
    .focused(true);

    if let Some(icon) = get_app_icon() {
        builder = builder.icon(icon).expect("valid icon");
    }

    let win = builder.build();

    if let Ok(win) = win {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        #[cfg(target_os = "windows")]
        if let Ok(hwnd) = win.hwnd() {
            wheel_win::force_focus_window(hwnd.0 as isize);
        }
    }
}

pub fn open_palette_window(app: &AppHandle) {
    use tauri::Manager;
    if let Some(win) = app.get_webview_window("palette") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        #[cfg(target_os = "windows")]
        if let Ok(hwnd) = win.hwnd() {
            wheel_win::force_focus_window(hwnd.0 as isize);
        }
        return;
    }

    let mut builder = tauri::WebviewWindowBuilder::new(
        app,
        "palette",
        tauri::WebviewUrl::App("index.html?window=palette".into()),
    )
    .title("Wheel Command Palette")
    .inner_size(640.0, 480.0)
    .decorations(false)
    .transparent(true)
    .resizable(false)
    .maximizable(false)
    .center()
    .always_on_top(true)
    .focused(true);

    if let Some(icon) = get_app_icon() {
        builder = builder.icon(icon).expect("valid icon");
    }

    let win = builder.build();

    if let Ok(win) = win {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        #[cfg(target_os = "windows")]
        if let Ok(hwnd) = win.hwnd() {
            wheel_win::force_focus_window(hwnd.0 as isize);
        }
    }
}
