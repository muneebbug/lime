/// Window vibrancy effects: Mica, Acrylic, Tabbed.
/// Wraps the `window-vibrancy` crate pattern for Tauri 2.

pub enum VibrancyEffect {
    Mica,
    Acrylic,
    Tabbed,
    None,
}

/// Apply the best available backdrop for the current OS.
/// Windows 11 → Mica; Windows 10 20H1+ → Acrylic; fallback → none.
pub fn apply_best_effect(_hwnd: isize) -> VibrancyEffect {
    #[cfg(windows)]
    {
        if is_windows_11() {
            VibrancyEffect::Mica
        } else if is_windows_10_20h1() {
            VibrancyEffect::Acrylic
        } else {
            VibrancyEffect::None
        }
    }
    #[cfg(not(windows))]
    VibrancyEffect::None
}

/// Remove all Windows 11 DWM window frame artifacts (rounded window corners,
/// window border line, window drop shadow, and DWM backdrop tint) so that
/// transparent overlay windows render strictly content pixels with zero box artifact.
pub fn make_overlay_transparent_frameless(hwnd: isize) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute,
            DWMWA_BORDER_COLOR,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            DWM_WINDOW_CORNER_PREFERENCE,
            DWMWCP_DONOTROUND,
            DWMWINDOWATTRIBUTE,
        };

        let hwnd = HWND(hwnd as _);

        unsafe {
            // 1. Disable Windows 11 rounded corners on the window rectangle
            let corner_pref = DWMWCP_DONOTROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner_pref as *const _ as *const _,
                std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
            );

            // 2. Disable DWM window border (0xFFFFFFFE = DWMWA_COLOR_NONE)
            let border_color: u32 = 0xFFFFFFFE;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &border_color as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            // 3. Disable DWM system backdrop material (attribute 38: DWMWA_SYSTEMBACKDROP_TYPE, 1 = DWMSBT_NONE)
            let backdrop_type: u32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(38),
                &backdrop_type as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd;
}

#[cfg(windows)]
fn get_build_number() -> u32 {
    // Read build number from registry (reliable on all Windows 10/11 versions)
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::System::Registry::{
        RegOpenKeyExW, RegQueryValueExW, HKEY_LOCAL_MACHINE, KEY_READ, HKEY,
    };
    use windows::core::PCWSTR;

    unsafe {
        let key_path: Vec<u16> = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\0"
            .encode_utf16()
            .collect();
        let value_name: Vec<u16> = "CurrentBuildNumber\0".encode_utf16().collect();

        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(key_path.as_ptr()),
            None,
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            return 0;
        }

        let mut buf = [0u16; 32];
        let mut buf_len = (buf.len() * 2) as u32;
        let _ = RegQueryValueExW(
            hkey,
            PCWSTR(value_name.as_ptr()),
            None,
            None,
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut buf_len),
        );

        // Parse the null-terminated u16 string as a number
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let s = std::ffi::OsString::from_wide(&buf[..end]);
        s.to_string_lossy().parse::<u32>().unwrap_or(0)
    }
}

#[cfg(windows)]
fn is_windows_11() -> bool {
    get_build_number() >= 22000
}

#[cfg(windows)]
fn is_windows_10_20h1() -> bool {
    let build = get_build_number();
    build >= 19041 && build < 22000
}
