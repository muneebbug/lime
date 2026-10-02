/// DPI awareness helpers for Per-Monitor V2 scaling.

#[cfg(windows)]
mod windows_impl {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::HiDpi::GetDpiForWindow;

    /// Get the DPI scale factor for a window (relative to 96 DPI baseline).
    pub fn dpi_scale_for_window(hwnd: isize) -> f64 {
        let dpi = unsafe { GetDpiForWindow(HWND(hwnd as *mut std::ffi::c_void)) };
        dpi as f64 / 96.0
    }

    /// Convert logical pixels to physical pixels for a given window.
    pub fn logical_to_physical(hwnd: isize, logical: i32) -> i32 {
        let scale = dpi_scale_for_window(hwnd);
        (logical as f64 * scale).round() as i32
    }

    /// Returns the monitor work area (excluding taskbar) for the point.
    pub fn work_area_for_point(x: i32, y: i32) -> (i32, i32, i32, i32) {
        use windows::Win32::Graphics::Gdi::{MonitorFromPoint, GetMonitorInfoW, MONITORINFO, MONITOR_DEFAULTTONEAREST};
        use windows::Win32::Foundation::POINT;

        unsafe {
            let pt = POINT { x, y };
            let monitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let _ = GetMonitorInfoW(monitor, &mut info);
            let rc = info.rcWork;
            (rc.left, rc.top, rc.right, rc.bottom)
        }
    }

    /// Clamp a window's position so it stays within the work area.
    /// Returns (x, y) clamped so the window of size (w, h) is fully visible.
    pub fn clamp_to_work_area(cx: i32, cy: i32, w: i32, h: i32) -> (i32, i32) {
        let (left, top, right, bottom) = work_area_for_point(cx, cy);
        let half_w = w / 2;
        let half_h = h / 2;
        let x = cx.max(left + half_w).min(right - half_w);
        let y = cy.max(top + half_h).min(bottom - half_h);
        (x - half_w, y - half_h)
    }
}

#[cfg(windows)]
pub use windows_impl::{dpi_scale_for_window, logical_to_physical, work_area_for_point, clamp_to_work_area};

#[cfg(not(windows))]
pub fn work_area_for_point(_x: i32, _y: i32) -> (i32, i32, i32, i32) {
    (0, 0, 1920, 1080)
}

#[cfg(not(windows))]
pub fn clamp_to_work_area(cx: i32, cy: i32, w: i32, h: i32) -> (i32, i32) {
    (cx - w / 2, cy - h / 2)
}
