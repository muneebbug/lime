use std::path::Path;
use anyhow::{Context, Result};
use tracing::info;

/// Check if Wheel is registered in Windows Explorer context menu (HKCU)
pub fn is_context_menu_registered() -> bool {
    let status = std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Classes\*\shell\Wheel"])
        .output();

    match status {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

/// Register Wheel in the Windows Explorer context menu under HKCU (no admin required)
pub fn register_context_menu(custom_exe: Option<&Path>) -> Result<()> {
    let exe = match custom_exe {
        Some(p) => p.to_path_buf(),
        None => std::env::current_exe().context("Failed to get current executable path")?,
    };

    let exe_str = exe.to_string_lossy();
    let cmd_str = format!("\"{}\" \"%1\"", exe_str);

    info!("Registering Explorer context menu for Wheel: {}", exe_str);

    // 1. Files: HKCU\Software\Classes\*\shell\Wheel
    let status = std::process::Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Classes\*\shell\Wheel",
            "/ve",
            "/d",
            "Open with Wheel",
            "/f",
        ])
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to create Wheel context menu file key");
    }

    let _ = std::process::Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Classes\*\shell\Wheel",
            "/v",
            "Icon",
            "/d",
            &exe_str,
            "/f",
        ])
        .status();

    let status = std::process::Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Classes\*\shell\Wheel\command",
            "/ve",
            "/d",
            &cmd_str,
            "/f",
        ])
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to create Wheel context menu command key");
    }

    // 2. Directories: HKCU\Software\Classes\Directory\shell\Wheel
    let _ = std::process::Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Classes\Directory\shell\Wheel",
            "/ve",
            "/d",
            "Open with Wheel",
            "/f",
        ])
        .status();

    let _ = std::process::Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Classes\Directory\shell\Wheel",
            "/v",
            "Icon",
            "/d",
            &exe_str,
            "/f",
        ])
        .status();

    let _ = std::process::Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Classes\Directory\shell\Wheel\command",
            "/ve",
            "/d",
            &cmd_str,
            "/f",
        ])
        .status();

    info!("Wheel Explorer context menu registered successfully");
    Ok(())
}

/// Unregister Wheel from Windows Explorer context menu
pub fn unregister_context_menu() -> Result<()> {
    info!("Unregistering Explorer context menu for Wheel");

    let _ = std::process::Command::new("reg")
        .args(["delete", r"HKCU\Software\Classes\*\shell\Wheel", "/f"])
        .status();

    let _ = std::process::Command::new("reg")
        .args(["delete", r"HKCU\Software\Classes\Directory\shell\Wheel", "/f"])
        .status();

    info!("Wheel Explorer context menu unregistered");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_menu_status_check() {
        // Querying HKCU registry key should succeed or return false cleanly without crashing
        let _ = is_context_menu_registered();
    }
}
