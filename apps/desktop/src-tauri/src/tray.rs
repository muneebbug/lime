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
        .on_menu_event(|_app, event| match event.id.as_ref() {
            "pause" => {
                info!("Tray: pause/resume toggled");
                // TODO: toggle wheel_win::hooks::set_paused
            }
            "settings" => {
                info!("Tray: open settings");
                // TODO: open settings window
            }
            _ => {}
        })
        .on_tray_icon_event(|_tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                // Left-click tray icon: open settings (or show recent jobs)
                info!("Tray icon left-clicked");
            }
        })
        .build(app)?;

    info!("System tray created");
    Ok(())
}
