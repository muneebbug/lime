/// Low-level mouse and keyboard hooks.
///
/// # Safety
/// Hook procedures run on a dedicated thread and MUST return immediately.
/// All processing is offloaded to a lock-free channel.
///
/// # Elevated-app note
/// WH_MOUSE_LL / WH_KEYBOARD_LL from a non-elevated process still receive
/// input events even from elevated windows. OLE DragDrop across integrity
/// boundaries may be blocked — handled in the drop target module.

#[cfg(windows)]
mod windows_impl {
    use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
    use tokio::sync::mpsc::UnboundedSender;
    use tracing::debug;

    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW,
        TranslateMessage, MSG, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT,
        WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP,
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_ESCAPE, VK_LSHIFT, VK_RSHIFT, VK_SHIFT,
    };

    use crate::WinEvent;

    // Global state shared between the hook callback and the processing thread.
    // Using atomics to avoid locks in the hook callback.
    static SHIFT_DOWN: AtomicBool = AtomicBool::new(false);
    static LBUTTON_DOWN: AtomicBool = AtomicBool::new(false);
    static BUTTON_X: AtomicI32 = AtomicI32::new(0);
    static BUTTON_Y: AtomicI32 = AtomicI32::new(0);
    static CURSOR_X: AtomicI32 = AtomicI32::new(0);
    static CURSOR_Y: AtomicI32 = AtomicI32::new(0);
    static DRAG_ARMED: AtomicBool = AtomicBool::new(false);
    static THRESHOLD_PX: AtomicI32 = AtomicI32::new(6);
    static PAUSED: AtomicBool = AtomicBool::new(false);
    static ALWAYS_SHOW: AtomicBool = AtomicBool::new(false);

    // Thread-local sender; set once when the hook thread starts.
    thread_local! {
        static SENDER: std::cell::RefCell<Option<UnboundedSender<WinEvent>>> =
            std::cell::RefCell::new(None);
    }

    fn send_event(event: WinEvent) {
        SENDER.with(|s| {
            if let Some(tx) = s.borrow().as_ref() {
                let _ = tx.send(event);
            }
        });
    }

    #[inline]
    fn is_shift_pressed() -> bool {
        SHIFT_DOWN.load(Ordering::Relaxed)
            || unsafe {
                (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0
                    || (GetAsyncKeyState(VK_LSHIFT.0 as i32) as u16 & 0x8000) != 0
                    || (GetAsyncKeyState(VK_RSHIFT.0 as i32) as u16 & 0x8000) != 0
            }
    }

    unsafe extern "system" fn mouse_hook_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code >= 0 && !PAUSED.load(Ordering::Relaxed) {
            let ms = &*(lparam.0 as *const MSLLHOOKSTRUCT);
            let x = ms.pt.x;
            let y = ms.pt.y;

            match wparam.0 as u32 {
                v if v == WM_LBUTTONDOWN => {
                    LBUTTON_DOWN.store(true, Ordering::Relaxed);
                    DRAG_ARMED.store(false, Ordering::Relaxed);
                    BUTTON_X.store(x, Ordering::Relaxed);
                    BUTTON_Y.store(y, Ordering::Relaxed);
                    CURSOR_X.store(x, Ordering::Relaxed);
                    CURSOR_Y.store(y, Ordering::Relaxed);
                    send_event(WinEvent::LButtonChanged { pressed: true, x, y });
                }
                v if v == WM_LBUTTONUP => {
                    LBUTTON_DOWN.store(false, Ordering::Relaxed);
                    let _was_armed = DRAG_ARMED.swap(false, Ordering::Relaxed);
                    send_event(WinEvent::LButtonChanged { pressed: false, x, y });
                }
                v if v == WM_MOUSEMOVE => {
                    CURSOR_X.store(x, Ordering::Relaxed);
                    CURSOR_Y.store(y, Ordering::Relaxed);

                    if LBUTTON_DOWN.load(Ordering::Relaxed) && !DRAG_ARMED.load(Ordering::Relaxed) {
                        let bx = BUTTON_X.load(Ordering::Relaxed);
                        let by = BUTTON_Y.load(Ordering::Relaxed);
                        let dx = (x - bx).abs();
                        let dy = (y - by).abs();
                        let threshold = THRESHOLD_PX.load(Ordering::Relaxed);
                        if (dx * dx + dy * dy) >= threshold * threshold
                            && (ALWAYS_SHOW.load(Ordering::Relaxed) || is_shift_pressed())
                        {
                            DRAG_ARMED.store(true, Ordering::Relaxed);
                            send_event(WinEvent::DragArmed { x, y });
                        }
                    }
                }
                _ => {}
            }
        }

        CallNextHookEx(None, code, wparam, lparam)
    }

    unsafe extern "system" fn keyboard_hook_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code >= 0 {
            let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let vk = kb.vkCode;
            let is_shift = vk == VK_SHIFT.0 as u32
                || vk == VK_LSHIFT.0 as u32
                || vk == VK_RSHIFT.0 as u32;

            match wparam.0 as u32 {
                v if v == WM_KEYDOWN || v == WM_SYSKEYDOWN => {
                    if is_shift {
                        SHIFT_DOWN.store(true, Ordering::Relaxed);
                        send_event(WinEvent::ShiftChanged(true));

                        // If left button is already held and mouse moved past threshold,
                        // arm the drag immediately upon pressing Shift!
                        if LBUTTON_DOWN.load(Ordering::Relaxed) && !DRAG_ARMED.load(Ordering::Relaxed) {
                            let bx = BUTTON_X.load(Ordering::Relaxed);
                            let by = BUTTON_Y.load(Ordering::Relaxed);
                            let cx = CURSOR_X.load(Ordering::Relaxed);
                            let cy = CURSOR_Y.load(Ordering::Relaxed);
                            let dx = (cx - bx).abs();
                            let dy = (cy - by).abs();
                            let threshold = THRESHOLD_PX.load(Ordering::Relaxed);
                            if (dx * dx + dy * dy) >= threshold * threshold {
                                DRAG_ARMED.store(true, Ordering::Relaxed);
                                send_event(WinEvent::DragArmed { x: cx, y: cy });
                            }
                        }
                    } else if vk == VK_ESCAPE.0 as u32 && DRAG_ARMED.load(Ordering::Relaxed) {
                        DRAG_ARMED.store(false, Ordering::Relaxed);
                        send_event(WinEvent::EscapePressed);
                        send_event(WinEvent::DragCancelled);
                    }
                }
                v if v == WM_KEYUP || v == WM_SYSKEYUP => {
                    if is_shift {
                        SHIFT_DOWN.store(false, Ordering::Relaxed);
                        send_event(WinEvent::ShiftChanged(false));
                    }
                }
                _ => {}
            }
        }

        CallNextHookEx(None, code, wparam, lparam)
    }

    /// Start the hook thread. Returns a channel receiver for WinEvents.
    /// The thread runs its own Win32 message pump; call `stop_hooks` with the
    /// returned thread handle to clean up.
    pub fn start_hooks(
        threshold_px: i32,
    ) -> (
        tokio::sync::mpsc::UnboundedReceiver<WinEvent>,
        std::thread::JoinHandle<()>,
    ) {
        THRESHOLD_PX.store(threshold_px, Ordering::Relaxed);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let handle = std::thread::Builder::new()
            .name("wheel-hooks".into())
            .spawn(move || unsafe {
                SENDER.with(|s| *s.borrow_mut() = Some(tx));

                let mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), None, 0)
                    .expect("Failed to install WH_MOUSE_LL");
                let keyboard_hook =
                    SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0)
                        .expect("Failed to install WH_KEYBOARD_LL");

                debug!("Hooks installed: mouse={:?} keyboard={:?}", mouse_hook, keyboard_hook);

                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            })
            .expect("Failed to spawn hook thread");

        (rx, handle)
    }

    pub fn set_paused(paused: bool) {
        PAUSED.store(paused, Ordering::Relaxed);
    }

    pub fn set_threshold(px: i32) {
        THRESHOLD_PX.store(px, Ordering::Relaxed);
    }

    pub fn set_always_show(always: bool) {
        ALWAYS_SHOW.store(always, Ordering::Relaxed);
    }
}

#[cfg(windows)]
pub use windows_impl::{start_hooks, set_paused, set_threshold, set_always_show};

#[cfg(not(windows))]
pub fn start_hooks(
    _threshold_px: i32,
) -> (
    tokio::sync::mpsc::UnboundedReceiver<crate::WinEvent>,
    std::thread::JoinHandle<()>,
) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = std::thread::spawn(|| {});
    (rx, handle)
}

#[cfg(not(windows))]
pub fn set_paused(_paused: bool) {}
#[cfg(not(windows))]
pub fn set_threshold(_px: i32) {}
#[cfg(not(windows))]
pub fn set_always_show(_always: bool) {}
