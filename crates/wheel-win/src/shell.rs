use std::path::Path;
use anyhow::{Context, Result};
use tracing::info;

/// Check if Lime is registered in Windows Explorer context menu (HKCU)
pub fn is_context_menu_registered() -> bool {
    let status = std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Classes\*\shell\Lime"])
        .output();

    match status {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

/// Register Lime in the Windows Explorer context menu under HKCU (no admin required)
pub fn register_context_menu(custom_exe: Option<&Path>) -> Result<()> {
    let exe = match custom_exe {
        Some(p) => p.to_path_buf(),
        None => std::env::current_exe().context("Failed to get current executable path")?,
    };

    let exe_str = exe.to_string_lossy();
    let cmd_str = format!("\"{}\" \"%1\"", exe_str);

    info!("Registering Explorer context menu for Lime: {}", exe_str);

    // 1. Files: HKCU\Software\Classes\*\shell\Lime
    let status = std::process::Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Classes\*\shell\Lime",
            "/ve",
            "/d",
            "Open with Lime",
            "/f",
        ])
        .output()?;

    if !status.status.success() {
        anyhow::bail!("Failed to create Lime context menu file key");
    }

    let _ = std::process::Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Classes\*\shell\Lime",
            "/v",
            "Icon",
            "/d",
            &exe_str,
            "/f",
        ])
        .output();

    let status = std::process::Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Classes\*\shell\Lime\command",
            "/ve",
            "/d",
            &cmd_str,
            "/f",
        ])
        .output()?;

    if !status.status.success() {
        anyhow::bail!("Failed to create Lime context menu command key");
    }

    // 2. Directories: HKCU\Software\Classes\Directory\shell\Lime
    let _ = std::process::Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Classes\Directory\shell\Lime",
            "/ve",
            "/d",
            "Open with Lime",
            "/f",
        ])
        .output();

    let _ = std::process::Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Classes\Directory\shell\Lime",
            "/v",
            "Icon",
            "/d",
            &exe_str,
            "/f",
        ])
        .output();

    let _ = std::process::Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Classes\Directory\shell\Lime\command",
            "/ve",
            "/d",
            &cmd_str,
            "/f",
        ])
        .output();

    info!("Lime Explorer context menu registered successfully");
    Ok(())
}

/// Unregister Lime from Windows Explorer context menu
pub fn unregister_context_menu() -> Result<()> {
    if !is_context_menu_registered() {
        return Ok(());
    }

    info!("Unregistering Explorer context menu for Lime");

    let _ = std::process::Command::new("reg")
        .args(["delete", r"HKCU\Software\Classes\*\shell\Lime", "/f"])
        .output();

    let _ = std::process::Command::new("reg")
        .args(["delete", r"HKCU\Software\Classes\Directory\shell\Lime", "/f"])
        .output();

    info!("Lime Explorer context menu unregistered");
    Ok(())
}

/// Check if Lime is registered to run at Windows login under HKCU
pub fn is_launch_at_login_registered() -> bool {
    let status = std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "Lime"])
        .output();

    match status {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

/// Register or unregister Lime in the Windows startup registry (HKCU Run key)
pub fn set_launch_at_login(enabled: bool) -> Result<()> {
    if is_launch_at_login_registered() == enabled {
        return Ok(());
    }

    if enabled {
        let exe = std::env::current_exe().context("Failed to get current executable path")?;
        let exe_str = exe.to_string_lossy();
        let cmd_str = format!("\"{}\"", exe_str);

        info!("Registering startup run key for Lime: {}", cmd_str);
        let status = std::process::Command::new("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                "Lime",
                "/t",
                "REG_SZ",
                "/d",
                &cmd_str,
                "/f",
            ])
            .output()?;

        if !status.status.success() {
            anyhow::bail!("Failed to register Lime in Windows startup registry");
        }
    } else {
        info!("Unregistering startup run key for Lime");
        let _ = std::process::Command::new("reg")
            .args([
                "delete",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                "Lime",
                "/f",
            ])
            .output();
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
$dialog.Description = "Select Lime Output Folder"
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

/// Check if the window at screen coordinates (x, y) is a recognized file drag source.
/// This prevents Lime from arming when dragging inside games, text editors,
/// or non-file windows.
#[cfg(windows)]
pub fn is_potential_file_drag_source(x: i32, y: i32) -> bool {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetAncestor, GetClassNameW, WindowFromPoint, GA_ROOT,
    };

    let pt = POINT { x, y };
    let hwnd = unsafe { WindowFromPoint(pt) };
    if hwnd.0.is_null() {
        return false;
    }

    let mut class_buf = [0u16; 128];
    let len = unsafe { GetClassNameW(hwnd, &mut class_buf) };
    let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);

    let root_hwnd = unsafe { GetAncestor(hwnd, GA_ROOT) };
    let mut root_class_buf = [0u16; 128];
    let root_len = if !root_hwnd.0.is_null() {
        unsafe { GetClassNameW(root_hwnd, &mut root_class_buf) }
    } else {
        0
    };
    let root_class_name = String::from_utf16_lossy(&root_class_buf[..root_len as usize]);

    // Fast check strictly for Explorer, Desktop, file dialogs, and recognized file managers.
    // Exclude general application windows (Chrome_WidgetWin_1, MozillaWindowClass, games, etc.)
    let is_valid = class_name == "CabinetWClass"
        || root_class_name == "CabinetWClass"
        || class_name == "Progman"
        || root_class_name == "Progman"
        || class_name == "WorkerW"
        || root_class_name == "WorkerW"
        || class_name == "ShellTabWindowClass"
        || root_class_name == "ShellTabWindowClass"
        || class_name == "DUIViewWndClassName"
        || class_name == "DirectUIHWND"
        || class_name == "SHELLDLL_DefView"
        || class_name == "SysListView32"
        || class_name == "UIItem"
        || class_name == "#32770"
        || root_class_name == "#32770"
        || class_name == "FilePickerHost"
        || root_class_name == "FilePickerHost"
        || class_name == "TTOTAL_CMD"
        || root_class_name == "TTOTAL_CMD"
        || class_name == "DOpus.Window"
        || root_class_name == "DOpus.Window"
        || class_name == "EVERYTHING"
        || root_class_name == "EVERYTHING"
        || class_name == "FM"
        || root_class_name == "FM"
        || class_name == "WinRAR"
        || root_class_name == "WinRAR";

    is_valid
}

#[cfg(not(windows))]
pub fn is_potential_file_drag_source(_x: i32, _y: i32) -> bool {
    true
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
