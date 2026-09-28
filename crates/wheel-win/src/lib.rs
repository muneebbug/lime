/// wheel-win: all Win32/OLE/Shell code lives here behind safe wrappers.
/// This crate is only compiled on Windows.
pub mod hooks;
pub mod drop_target;
pub mod dpi;
pub mod vibrancy;

#[cfg(not(windows))]
pub mod stubs {
    // Stubs for non-Windows compilation (CI, docs)
}

/// Event sent from the Win32 hook thread to the main application
#[derive(Debug, Clone)]
pub enum WinEvent {
    /// Shift key state changed
    ShiftChanged(bool),
    /// Left mouse button state changed
    LButtonChanged { pressed: bool, x: i32, y: i32 },
    /// Mouse moved while button is held
    MouseMove { x: i32, y: i32 },
    /// Drag potentially confirmed (movement threshold exceeded with modifier)
    DragArmed { x: i32, y: i32 },
    /// Drag ended (button released or Escape pressed)
    DragCancelled,
    /// Escape was pressed during an armed drag
    EscapePressed,
}
