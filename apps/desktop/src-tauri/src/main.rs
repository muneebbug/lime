// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(all(windows, debug_assertions))]
fn cleanup_stale_dev_processes() {
    use std::os::windows::process::CommandExt;
    let current_pid = std::process::id();
    let mut cmd = std::process::Command::new("taskkill");
    cmd.creation_flags(0x08000000)
        .args([
            "/FI",
            "IMAGENAME eq desktop.exe",
            "/FI",
            &format!("PID ne {}", current_pid),
            "/F",
        ]);
    let _ = cmd.output();
    std::thread::sleep(std::time::Duration::from_millis(150));
}

fn main() {
    #[cfg(all(windows, debug_assertions))]
    cleanup_stale_dev_processes();

    let rt = tokio::runtime::Runtime::new().expect("failed to initialize Tokio runtime");
    let _guard = rt.enter();
    desktop_lib::run();
}

