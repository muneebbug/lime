use anyhow::{Context, Result};
use tracing::info;

#[cfg(windows)]
fn value_exists(sub_key: &str, value_name: &str) -> bool {
    use windows::Win32::System::Registry::{
        RegOpenKeyExW, RegQueryValueExW, RegCloseKey, HKEY_CURRENT_USER, KEY_READ, HKEY,
    };
    use windows::core::PCWSTR;

    let sub_key_w: Vec<u16> = sub_key.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey = HKEY::default();
    let res = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(sub_key_w.as_ptr()),
            None,
            KEY_READ,
            &mut hkey,
        )
    };
    if res.is_err() {
        return false;
    }

    let val_w: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    let query_res = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(val_w.as_ptr()),
            None,
            None,
            None,
            None,
        )
    };
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    query_res.is_ok()
}

#[cfg(windows)]
fn set_reg_string(sub_key: &str, value_name: Option<&str>, data: &str) -> Result<()> {
    use windows::Win32::System::Registry::{
        RegCreateKeyW, RegSetValueExW, RegCloseKey, HKEY_CURRENT_USER, HKEY,
        REG_SZ,
    };
    use windows::core::PCWSTR;

    let sub_key_w: Vec<u16> = sub_key.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey = HKEY::default();
    let res = unsafe {
        RegCreateKeyW(
            HKEY_CURRENT_USER,
            PCWSTR(sub_key_w.as_ptr()),
            &mut hkey,
        )
    };
    if !res.is_ok() {
        anyhow::bail!("Failed to create registry key {}: {:?}", sub_key, res);
    }

    let data_w: Vec<u16> = data.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes_len = (data_w.len() * 2) as u32;

    let val_name_w: Option<Vec<u16>> = value_name.map(|v| v.encode_utf16().chain(std::iter::once(0)).collect());
    let val_ptr = match &val_name_w {
        Some(w) => PCWSTR(w.as_ptr()),
        None => PCWSTR::null(),
    };

    let set_res = unsafe {
        RegSetValueExW(
            hkey,
            val_ptr,
            None,
            REG_SZ,
            Some(std::slice::from_raw_parts(data_w.as_ptr() as *const u8, bytes_len as usize)),
        )
    };
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    if !set_res.is_ok() {
        anyhow::bail!("Failed to set registry value in {}: {:?}", sub_key, set_res);
    }
    Ok(())
}

#[cfg(windows)]
fn delete_reg_value(sub_key: &str, value_name: &str) -> Result<()> {
    use windows::Win32::System::Registry::{
        RegOpenKeyExW, RegDeleteValueW, RegCloseKey, HKEY_CURRENT_USER, KEY_WRITE, HKEY,
    };
    use windows::core::PCWSTR;

    let sub_key_w: Vec<u16> = sub_key.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey = HKEY::default();
    let res = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(sub_key_w.as_ptr()),
            None,
            KEY_WRITE,
            &mut hkey,
        )
    };
    if res.is_ok() {
        let val_w: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = unsafe { RegDeleteValueW(hkey, PCWSTR(val_w.as_ptr())) };
        unsafe {
            let _ = RegCloseKey(hkey);
        }
    }
    Ok(())
}

/// Check if Lime is registered to run at Windows login under HKCU
pub fn is_launch_at_login_registered() -> bool {
    #[cfg(windows)]
    {
        value_exists(r"Software\Microsoft\Windows\CurrentVersion\Run", "Lime")
    }
    #[cfg(not(windows))]
    false
}

/// Register or unregister Lime in the Windows startup registry (HKCU Run key)
pub fn set_launch_at_login(enabled: bool) -> Result<()> {
    if is_launch_at_login_registered() == enabled {
        return Ok(());
    }

    #[cfg(windows)]
    {
        if enabled {
            let exe = std::env::current_exe().context("Failed to get current executable path")?;
            let exe_str = exe.to_string_lossy();
            let cmd_str = format!("\"{}\"", exe_str);

            info!("Registering startup run key for Lime: {}", cmd_str);
            set_reg_string(r"Software\Microsoft\Windows\CurrentVersion\Run", Some("Lime"), &cmd_str)?;
        } else {
            info!("Unregistering startup run key for Lime");
            delete_reg_value(r"Software\Microsoft\Windows\CurrentVersion\Run", "Lime")?;
        }
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

    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let status = cmd.status();

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
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let output = cmd.output()?;
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
