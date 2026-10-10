/// DPI awareness helpers for Per-Monitor V2 scaling.

/// Clamp a window's top-left so it stays inside `work_area` = (left, top, right, bottom).
///
/// Platform-independent arithmetic, deliberately kept out of the Win32 module so
/// it can be tested without a multi-monitor rig attached - which is exactly
/// where this logic goes wrong, and exactly where a test is impossible.
///
/// Centres on (cx, cy), then pulls the window back inside the area.
///
/// A window larger than the work area is pinned to its top-left rather than
/// clamped. Clamping with inverted bounds - `left + half > right - half` - used
/// to push it to a *negative* coordinate, which put a third of the window off
/// the left of the screen. Overhanging is the lesser evil when nothing fits.
pub fn clamp_to_rect(cx: i32, cy: i32, w: i32, h: i32, work: (i32, i32, i32, i32)) -> (i32, i32) {
    let (left, top, right, bottom) = work;

    let x = if w <= right - left {
        (cx - w / 2).max(left).min(right - w)
    } else {
        left
    };
    let y = if h <= bottom - top {
        (cy - h / 2).max(top).min(bottom - h)
    } else {
        top
    };

    (x, y)
}

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
        let work = work_area_for_point(cx, cy);
        crate::dpi::clamp_to_rect(cx, cy, w, h, work)
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

#[cfg(test)]
mod tests {
    use super::clamp_to_rect;

    /// A single 1920x1080 monitor with no taskbar.
    const ONE: (i32, i32, i32, i32) = (0, 0, 1920, 1080);
    /// Two 1920x1080 monitors side by side, the second starting at x=1920.
    const TWO: (i32, i32, i32, i32) = (0, 0, 3840, 1080);

    #[test]
    fn a_window_lands_centred_on_the_anchor() {
        let (x, y) = clamp_to_rect(960, 540, 800, 600, ONE);
        assert_eq!((x, y), (560, 240));
    }

    #[test]
    fn the_second_monitor_wins_over_the_primary() {
        // The whole point: an anchor on the right-hand display must produce a
        // window over there, not one snapped back to the left.
        let (x, y) = clamp_to_rect(2880, 540, 800, 600, TWO);
        assert_eq!((x, y), (2480, 240));
        assert!(x >= 1920, "window opened on the wrong monitor");
    }

    #[test]
    fn a_window_anchored_inside_the_second_monitor_stays_there() {
        let (x, _) = clamp_to_rect(3000, 540, 800, 600, TWO);
        assert!(x >= 1920, "window snapped back to the primary monitor: {x}");
    }

    #[test]
    fn an_anchor_on_the_seam_centres_across_it() {
        // Deliberate, not a bug: a window anchored exactly where the two
        // monitors meet straddles the seam, because the cursor is on both.
        let (x, _) = clamp_to_rect(1920, 540, 800, 600, TWO);
        assert_eq!(x, 1520);
    }

    #[test]
    fn a_window_never_strays_right_of_the_work_area() {
        let (x, _) = clamp_to_rect(3839, 540, 800, 600, TWO);
        assert_eq!(x, 3840 - 800, "must not overhang the right edge");
    }

    #[test]
    fn a_work_area_offset_by_a_taskbar_is_respected() {
        // Windows taskbar along the bottom: the usable area stops at y=1040.
        let work = (0, 0, 1920, 1040);
        let (x, y) = clamp_to_rect(960, 1039, 800, 600, work);
        assert_eq!((x, y), (560, 1040 - 600));
    }

    #[test]
    fn a_monitor_above_and_left_of_the_origin_is_handled() {
        // Secondary displays can sit at negative coordinates.
        let work = (-1920, -300, 0, 780);
        let (x, y) = clamp_to_rect(-960, 240, 800, 600, work);
        assert_eq!((x, y), (-1360, -60));
        assert!(x >= -1920 && x + 800 <= 0, "escaped the negative monitor");
    }

    #[test]
    fn a_window_larger_than_the_screen_pins_to_the_top_left() {
        // Clamping with inverted bounds used to hand back a negative x, which
        // put part of the window off the left of the screen entirely.
        let (x, y) = clamp_to_rect(960, 540, 3000, 2000, ONE);
        assert_eq!((x, y), (0, 0), "an oversized window must not invert");

        let (x, y) = clamp_to_rect(2880, 540, 3000, 2000, TWO);
        // Wider than one monitor but narrower than both together, so it still
        // fits horizontally and is only pinned vertically.
        assert_eq!((x, y), (840, 0));
        assert!(x >= 0 && y >= 0, "an oversized window must never go negative");
    }
}
