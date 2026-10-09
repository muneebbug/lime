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
// CLSID_FileOpenDialog, declared locally because this crate's feature set does
// not export it. Verified against ShObjIdl_core.h and the registry:
//   class DECLSPEC_UUID("DC1C5A9C-E88A-4dde-A5A1-60F82A20AEF7") FileOpenDialog
//
// Do not "simplify" this by hand. A wrong CLSID fails with
// CLASS_E_CLASSNOTAVAIL (0x80040154), which the caller treats as "no folder
// chosen", so the only symptom is a dialog that never opens. `dialog_tests`
// resolves this same constant to catch that.
const CLSID_FILE_OPEN_DIALOG: windows::core::GUID =
    windows::core::GUID::from_u128(0xdc1c5a9c_e88a_4dde_a5a1_60f82a20aef7);

/// Ask the user to choose a folder.
///
/// Uses the Common Item Dialog (`IFileDialog` with `FOS_PICKFOLDERS`), which is
/// the same Explorer-style picker Windows shows for "Save as". The older
/// `SHBrowseForFolder` / `FolderBrowserDialog` tree view is still what many apps
/// ship, and it has no owner window, so it opens behind the app and cannot be
/// focused.
///
/// The dialog is owned by `parent`, which both centres it on the window and
/// makes it modal, so it cannot end up stranded behind it.
pub fn pick_folder() -> Result<Option<std::path::PathBuf>> {
    pick_folder_owned(None, "Choose a folder")
}

/// As [`pick_folder`], but owned by `parent` and with a custom title.
pub fn pick_folder_owned(parent: Option<isize>, title: &str) -> Result<Option<std::path::PathBuf>> {
    #[cfg(windows)]
    {
        windows_pick_folder(parent, title).map_err(|e| anyhow::anyhow!("{e}"))
    }
    #[cfg(not(windows))]
    {
        let _ = (parent, title);
        Ok(None)
    }
}

/// The real implementation. Kept separate so the `cfg` split stays readable.
#[cfg(windows)]
fn windows_pick_folder(parent: Option<isize>, title: &str) -> Result<Option<std::path::PathBuf>> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
    use windows::Win32::UI::Shell::{
        FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, IFileOpenDialog, SIGDN_FILESYSPATH,
    };
    use windows::Win32::Foundation::ERROR_CANCELLED;

    unsafe {
        // A modal dialog needs a message pump on its own thread.
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;

        let result = (|| -> windows::core::Result<Option<std::path::PathBuf>> {
            // CLSID_FileOpenDialog, declared locally because this crate's feature set does
            // not export it.
            let dialog: IFileOpenDialog =
                CoCreateInstance(&CLSID_FILE_OPEN_DIALOG, None, CLSCTX_INPROC_SERVER)?;

            // FOS_PICKFOLDERS turns the Save dialog into a folder chooser,
            // keeping the modern layout rather than the legacy tree view. Re-read
            // and re-apply because a provider may drop the flag on creation.
            let options = dialog.GetOptions()? | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM;
            dialog.SetOptions(options)?;

            if !title.is_empty() {
                dialog.SetTitle(&HSTRING::from(title))?;
            }

            // Own the dialog so it is modal to, and centred on, the app window.
            // Without this it opens unowned: no focus, and behind the window.
            let owner: HWND = match parent {
                Some(hwnd) => HWND(hwnd as *mut std::ffi::c_void),
                None => GetActiveWindow(),
            };
            dialog.Show(Some(owner))?;

            // Dismissing the dialog is not a failure. `GetResult` reports
            // ERROR_CANCELLED, which must come back as "no folder" so the caller
            // can fall back rather than treating it as a broken dialog.
            let item = match dialog.GetResult() {
                Ok(item) => item,
                Err(e) if e.code() == ERROR_CANCELLED.to_hresult() => return Ok(None),
                Err(e) => return Err(e),
            };

            let path = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let value = path.to_string().unwrap_or_default();
            // The display name is COM-allocated; free it or it leaks per dialog.
            CoTaskMemFree(Some(path.0 as *const _));

            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(std::path::PathBuf::from(value)))
            }
        })();

        CoUninitialize();
        result.map_err(|e| anyhow::anyhow!("{e}"))
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

#[cfg(all(test, windows))]
mod dialog_tests {
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::IFileOpenDialog;

    /// The constant production actually uses. Deliberately not a second copy:
    /// a duplicated literal would pass even when production was wrong, which is
    /// exactly the bug this test exists to catch.
    const EXPECTED_CLSID: windows::core::GUID = super::CLSID_FILE_OPEN_DIALOG;

    #[test]
    fn test_the_dialog_clsid_is_the_real_one() {
        // Regression: a hand-written CLSID that was not the registered one made
        // every dialog fail with CLASS_E_CLASSNOTAVAIL (0x80040154). Because the
        // caller treats that as "no folder chosen", the symptom was a dialog that
        // never opened, with nothing to indicate a wrong constant.
        assert_eq!(
            EXPECTED_CLSID.data1, 0xDC1C5A9C,
            "CLSID_FileOpenDialog data1 must be DC1C5A9C"
        );
        assert_eq!(
            EXPECTED_CLSID.data3, 0x4DDE,
            "CLSID_FileOpenDialog data3 must be 4DDE"
        );
    }

    #[test]
    fn test_the_file_open_dialog_class_is_registered_and_constructible() {
        // Constructing the object without showing it proves the CLSID resolves on
        // this machine. This is the check that would have caught the bad GUID.
        unsafe {
            // Each test runs on its own thread, so COM is initialised here rather
            // than relying on process-wide state. A failure is not an error: COM
            // is already initialised often enough, and an RPC_E_CHANGED_MODE
            // here would not stop the CLSID check that follows.
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let outcome = CoCreateInstance::<_, IFileOpenDialog>(
                &EXPECTED_CLSID,
                None,
                CLSCTX_INPROC_SERVER,
            )
            // Release the COM object *before* CoUninitialize. Tearing down the
            // apartment while a live interface still holds a reference is a
            // use-after-uninitialize, and crashes the test binary.
            .map(|dialog| drop(dialog));

            CoUninitialize();

            assert!(
                outcome.is_ok(),
                "CLSID_FileOpenDialog must be registered and constructible; got {:?}",
                outcome.err()
            );
        }
    }
}
