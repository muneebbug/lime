// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let rt = tokio::runtime::Runtime::new().expect("failed to initialize Tokio runtime");
    let _guard = rt.enter();
    desktop_lib::run();
}
