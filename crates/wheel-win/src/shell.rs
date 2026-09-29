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

/// Check if Wheel is registered to run at Windows login under HKCU
pub fn is_launch_at_login_registered() -> bool {
    let status = std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "Wheel"])
        .output();

    match status {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

/// Register or unregister Wheel in the Windows startup registry (HKCU Run key)
pub fn set_launch_at_login(enabled: bool) -> Result<()> {
    if enabled {
        let exe = std::env::current_exe().context("Failed to get current executable path")?;
        let exe_str = exe.to_string_lossy();
        let cmd_str = format!("\"{}\"", exe_str);

        info!("Registering startup run key for Wheel: {}", cmd_str);
        let status = std::process::Command::new("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                "Wheel",
                "/t",
                "REG_SZ",
                "/d",
                &cmd_str,
                "/f",
            ])
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to register Wheel in Windows startup registry");
        }
    } else {
        info!("Unregistering startup run key for Wheel");
        let _ = std::process::Command::new("reg")
            .args([
                "delete",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                "Wheel",
                "/f",
            ])
            .status();
    }
    Ok(())
}

/// Copy file paths to Windows clipboard
pub fn copy_files_to_clipboard(paths: &[std::path::PathBuf]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }

    let escaped_items = paths
        .iter()
        .map(|p| format!("'{}'", p.to_string_lossy().replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",");

    let script = format!("Set-Clipboard -Path @({})", escaped_items);

    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status();

    if let Err(e) = status {
        tracing::warn!("Failed to set clipboard: {}", e);
    }
    Ok(())
}

/// Prompt the user to select a folder using a native Windows folder browser dialog
pub fn pick_folder() -> Result<Option<std::path::PathBuf>> {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.Description = "Select Wheel Output Folder"
$dialog.ShowNewFolderButton = $true
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
    [Console]::Out.Write($dialog.SelectedPath)
}
"#;
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()?;
    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path_str.is_empty() {
        Ok(None)
    } else {
        Ok(Some(std::path::PathBuf::from(path_str)))
    }
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
