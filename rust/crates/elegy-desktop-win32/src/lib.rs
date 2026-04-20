//! Minimal Win32 FFI wrappers for desktop input automation.
//!
//! This crate intentionally allows `unsafe_code` because it wraps raw Win32 API
//! calls. All public functions expose a fully safe typed API. Every `unsafe` block
//! has a `// SAFETY:` comment explaining the invariant.
//!
//! This crate must remain under 800 lines of source. If it grows beyond that,
//! split into focused modules.

use serde::Serialize;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from Win32 desktop automation operations.
#[derive(Debug, Error)]
pub enum DesktopWin32Error {
    /// The operation is not supported on this platform.
    #[error("operation not supported on this platform")]
    Unsupported,
    /// A Win32 API call failed.
    #[error("Win32 API call failed: {0}")]
    ApiError(String),
    /// The target window was not found.
    #[error("window not found")]
    WindowNotFound,
    /// Input injection failed.
    #[error("input injection failed: {0}")]
    InputFailed(String),
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Mouse button for click operations.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Virtual key codes supported for key combo operations.
///
/// MVP subset: modifiers, common navigation, F-keys, A-Z, 0-9.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualKey {
    // Modifiers
    Ctrl,
    Alt,
    Shift,
    Win,
    // Navigation / editing
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Space,
    Home,
    End,
    PageUp,
    PageDown,
    // Arrows
    Up,
    Down,
    Left,
    Right,
    // F-keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    // Alphanumeric (uppercase letter or digit character)
    Char(char),
}

// ---------------------------------------------------------------------------
// Win32 implementation (Windows only)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win32 {
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    use crate::{DesktopWin32Error, MouseButton, VirtualKey};

    /// Move the mouse cursor to absolute pixel coordinates.
    pub fn move_cursor(x: i32, y: i32) -> Result<(), DesktopWin32Error> {
        // SAFETY: SetCursorPos moves the cursor to the specified screen
        // coordinates. No memory is read/written beyond the two integers.
        let result = unsafe { SetCursorPos(x, y) };
        if result.is_err() {
            return Err(DesktopWin32Error::ApiError("SetCursorPos failed".to_string()));
        }
        Ok(())
    }

    /// Get current mouse cursor position in screen pixels.
    pub fn get_cursor_pos() -> Result<(i32, i32), DesktopWin32Error> {
        let mut pt = POINT::default();
        // SAFETY: GetCursorPos writes a POINT to our stack-allocated struct.
        let result = unsafe { GetCursorPos(&mut pt) };
        if result.is_err() {
            return Err(DesktopWin32Error::ApiError("GetCursorPos failed".to_string()));
        }
        Ok((pt.x, pt.y))
    }

    /// Simulate a mouse click at the given pixel coordinates.
    ///
    /// Uses `SetCursorPos` for positioning (pixel-accurate) and `SendInput`
    /// for the button press/release events.
    pub fn click(x: i32, y: i32, button: MouseButton) -> Result<(), DesktopWin32Error> {
        // First move cursor to target position.
        move_cursor(x, y)?;

        let (down_flag, up_flag) = match button {
            MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
            MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
            MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
        };

        let inputs = [make_mouse_input(down_flag), make_mouse_input(up_flag)];

        send_inputs(&inputs)
    }

    /// Type text by injecting Unicode key events.
    ///
    /// Each character is converted to UTF-16 code units. Supplementary
    /// characters (outside BMP) produce surrogate pairs, each requiring
    /// a separate down/up event.
    pub fn type_text(text: &str) -> Result<(), DesktopWin32Error> {
        let mut inputs = Vec::new();

        for code_unit in text.encode_utf16() {
            inputs.push(make_unicode_key_input(code_unit, false));
            inputs.push(make_unicode_key_input(code_unit, true));
        }

        if inputs.is_empty() {
            return Ok(());
        }

        send_inputs(&inputs)
    }

    /// Send a key combination (e.g., Ctrl+S).
    ///
    /// Presses all keys in order, then releases in reverse order.
    pub fn send_key_combo(keys: &[VirtualKey]) -> Result<(), DesktopWin32Error> {
        if keys.is_empty() {
            return Ok(());
        }

        let mut inputs = Vec::new();

        // Press all keys in order.
        for key in keys {
            let vk = virtual_key_to_vk(*key)?;
            inputs.push(make_vk_input(vk, false));
        }

        // Release all keys in reverse order.
        for key in keys.iter().rev() {
            let vk = virtual_key_to_vk(*key)?;
            inputs.push(make_vk_input(vk, true));
        }

        send_inputs(&inputs)
    }

    /// Set focus to a window by its raw HWND value.
    ///
    /// Attempts `ShowWindow(SW_RESTORE)` first for minimized windows, then
    /// `SetForegroundWindow`. Verifies focus was actually acquired.
    pub fn focus_window(hwnd: u64) -> Result<(), DesktopWin32Error> {
        let handle = HWND(hwnd as *mut _);

        // SAFETY: IsWindow checks if the HWND is valid. No memory mutation.
        if !unsafe { IsWindow(handle) }.as_bool() {
            return Err(DesktopWin32Error::WindowNotFound);
        }

        // SAFETY: ShowWindow changes window state. SW_RESTORE un-minimizes
        // without resizing if the window isn't minimized.
        let _ = unsafe { ShowWindow(handle, SW_RESTORE) };

        // SAFETY: SetForegroundWindow attempts to bring the window to front.
        // May fail due to UIPI or foreground lock restrictions.
        let result = unsafe { SetForegroundWindow(handle) };
        if !result.as_bool() {
            // Try BringWindowToTop as fallback.
            // SAFETY: BringWindowToTop changes Z-order.
            let _ = unsafe { BringWindowToTop(handle) };
        }

        // Small delay to allow window manager to process the focus change.
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Verify focus was acquired.
        // SAFETY: GetForegroundWindow reads current foreground HWND. No mutation.
        let fg = unsafe { GetForegroundWindow() };
        if fg != handle {
            return Err(DesktopWin32Error::ApiError(
                "failed to acquire foreground focus (may be blocked by UIPI)".to_string(),
            ));
        }

        Ok(())
    }

    /// Move and optionally resize a window.
    pub fn move_window(
        hwnd: u64,
        x: i32,
        y: i32,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Result<(), DesktopWin32Error> {
        let handle = HWND(hwnd as *mut _);

        // SAFETY: IsWindow checks validity. No mutation.
        if !unsafe { IsWindow(handle) }.as_bool() {
            return Err(DesktopWin32Error::WindowNotFound);
        }

        // If dimensions not specified, preserve current size.
        let (w, h) = match (width, height) {
            (Some(w), Some(h)) => (w as i32, h as i32),
            _ => {
                let mut rect = RECT::default();
                // SAFETY: GetWindowRect reads the window bounding rect.
                unsafe {
                    GetWindowRect(handle, &mut rect)
                        .map_err(|e| DesktopWin32Error::ApiError(format!("GetWindowRect: {e}")))?;
                }
                (
                    width.map_or(rect.right - rect.left, |w| w as i32),
                    height.map_or(rect.bottom - rect.top, |h| h as i32),
                )
            }
        };

        // SAFETY: MoveWindow repositions and resizes the window. The `true`
        // parameter triggers a repaint.
        unsafe {
            MoveWindow(handle, x, y, w, h, true)
                .map_err(|_| DesktopWin32Error::ApiError("MoveWindow failed".to_string()))?;
        }

        Ok(())
    }

    /// Minimize a window.
    pub fn minimize_window(hwnd: u64) -> Result<(), DesktopWin32Error> {
        show_window_command(hwnd, SW_MINIMIZE)
    }

    /// Maximize a window.
    pub fn maximize_window(hwnd: u64) -> Result<(), DesktopWin32Error> {
        show_window_command(hwnd, SW_MAXIMIZE)
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn show_window_command(hwnd: u64, cmd: SHOW_WINDOW_CMD) -> Result<(), DesktopWin32Error> {
        let handle = HWND(hwnd as *mut _);
        // SAFETY: IsWindow checks validity. No mutation.
        if !unsafe { IsWindow(handle) }.as_bool() {
            return Err(DesktopWin32Error::WindowNotFound);
        }
        // SAFETY: ShowWindow changes the window show state.
        let _ = unsafe { ShowWindow(handle, cmd) };
        Ok(())
    }

    fn make_mouse_input(flags: MOUSE_EVENT_FLAGS) -> INPUT {
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn make_unicode_key_input(code_unit: u16, key_up: bool) -> INPUT {
        let mut flags = KEYEVENTF_UNICODE;
        if key_up {
            flags |= KEYEVENTF_KEYUP;
        }
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: code_unit,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn make_vk_input(vk: VIRTUAL_KEY, key_up: bool) -> INPUT {
        let mut flags = KEYBD_EVENT_FLAGS(0);
        if key_up {
            flags |= KEYEVENTF_KEYUP;
        }
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn send_inputs(inputs: &[INPUT]) -> Result<(), DesktopWin32Error> {
        let size = std::mem::size_of::<INPUT>() as i32;
        // SAFETY: SendInput injects the given input events into the input
        // stream. The inputs slice is valid for the duration of this call.
        // We pass the exact count and struct size.
        let sent = unsafe { SendInput(inputs, size) };
        if sent != inputs.len() as u32 {
            return Err(DesktopWin32Error::InputFailed(format!(
                "SendInput sent {sent}/{} events",
                inputs.len()
            )));
        }
        Ok(())
    }

    fn virtual_key_to_vk(key: VirtualKey) -> Result<VIRTUAL_KEY, DesktopWin32Error> {
        match key {
            VirtualKey::Ctrl => Ok(VK_CONTROL),
            VirtualKey::Alt => Ok(VK_MENU),
            VirtualKey::Shift => Ok(VK_SHIFT),
            VirtualKey::Win => Ok(VK_LWIN),
            VirtualKey::Enter => Ok(VK_RETURN),
            VirtualKey::Tab => Ok(VK_TAB),
            VirtualKey::Escape => Ok(VK_ESCAPE),
            VirtualKey::Backspace => Ok(VK_BACK),
            VirtualKey::Delete => Ok(VK_DELETE),
            VirtualKey::Space => Ok(VK_SPACE),
            VirtualKey::Home => Ok(VK_HOME),
            VirtualKey::End => Ok(VK_END),
            VirtualKey::PageUp => Ok(VK_PRIOR),
            VirtualKey::PageDown => Ok(VK_NEXT),
            VirtualKey::Up => Ok(VK_UP),
            VirtualKey::Down => Ok(VK_DOWN),
            VirtualKey::Left => Ok(VK_LEFT),
            VirtualKey::Right => Ok(VK_RIGHT),
            VirtualKey::F1 => Ok(VK_F1),
            VirtualKey::F2 => Ok(VK_F2),
            VirtualKey::F3 => Ok(VK_F3),
            VirtualKey::F4 => Ok(VK_F4),
            VirtualKey::F5 => Ok(VK_F5),
            VirtualKey::F6 => Ok(VK_F6),
            VirtualKey::F7 => Ok(VK_F7),
            VirtualKey::F8 => Ok(VK_F8),
            VirtualKey::F9 => Ok(VK_F9),
            VirtualKey::F10 => Ok(VK_F10),
            VirtualKey::F11 => Ok(VK_F11),
            VirtualKey::F12 => Ok(VK_F12),
            VirtualKey::Char(c) => {
                let upper = c.to_ascii_uppercase();
                match upper {
                    'A'..='Z' => Ok(VIRTUAL_KEY(upper as u16)),
                    '0'..='9' => Ok(VIRTUAL_KEY(upper as u16)),
                    _ => Err(DesktopWin32Error::InputFailed(format!(
                        "unsupported key character: '{c}' — MVP supports A-Z, 0-9, modifiers, and navigation keys only"
                    ))),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Move the mouse cursor to absolute pixel coordinates.
///
/// Returns `DesktopWin32Error::Unsupported` on non-Windows platforms.
pub fn move_cursor(x: i32, y: i32) -> Result<(), DesktopWin32Error> {
    #[cfg(windows)]
    {
        win32::move_cursor(x, y)
    }
    #[cfg(not(windows))]
    {
        let _ = (x, y);
        Err(DesktopWin32Error::Unsupported)
    }
}

/// Get the current mouse cursor position in screen pixels.
///
/// Returns `DesktopWin32Error::Unsupported` on non-Windows platforms.
pub fn get_cursor_pos() -> Result<(i32, i32), DesktopWin32Error> {
    #[cfg(windows)]
    {
        win32::get_cursor_pos()
    }
    #[cfg(not(windows))]
    {
        Err(DesktopWin32Error::Unsupported)
    }
}

/// Simulate a mouse click at the given pixel coordinates.
///
/// Uses `SetCursorPos` for pixel-accurate positioning and `SendInput` for
/// button press/release. Returns `DesktopWin32Error::Unsupported` on non-Windows.
pub fn click(x: i32, y: i32, button: MouseButton) -> Result<(), DesktopWin32Error> {
    #[cfg(windows)]
    {
        win32::click(x, y, button)
    }
    #[cfg(not(windows))]
    {
        let _ = (x, y, button);
        Err(DesktopWin32Error::Unsupported)
    }
}

/// Type text by injecting Unicode key events via `SendInput`.
///
/// Handles supplementary characters (surrogate pairs) correctly.
/// Returns `DesktopWin32Error::Unsupported` on non-Windows.
pub fn type_text(text: &str) -> Result<(), DesktopWin32Error> {
    #[cfg(windows)]
    {
        win32::type_text(text)
    }
    #[cfg(not(windows))]
    {
        let _ = text;
        Err(DesktopWin32Error::Unsupported)
    }
}

/// Send a key combination (e.g., Ctrl+S).
///
/// Presses all keys in order, releases in reverse order.
/// Returns `DesktopWin32Error::Unsupported` on non-Windows.
pub fn send_key_combo(keys: &[VirtualKey]) -> Result<(), DesktopWin32Error> {
    #[cfg(windows)]
    {
        win32::send_key_combo(keys)
    }
    #[cfg(not(windows))]
    {
        let _ = keys;
        Err(DesktopWin32Error::Unsupported)
    }
}

/// Set focus to a window by raw HWND.
///
/// Attempts `ShowWindow(SW_RESTORE)` then `SetForegroundWindow`. Verifies
/// focus was acquired. May fail due to UIPI restrictions.
/// Returns `DesktopWin32Error::Unsupported` on non-Windows.
pub fn focus_window(hwnd: u64) -> Result<(), DesktopWin32Error> {
    #[cfg(windows)]
    {
        win32::focus_window(hwnd)
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        Err(DesktopWin32Error::Unsupported)
    }
}

/// Move and optionally resize a window by raw HWND.
///
/// If width/height are None, the current dimensions are preserved.
/// Returns `DesktopWin32Error::Unsupported` on non-Windows.
pub fn move_window(
    hwnd: u64,
    x: i32,
    y: i32,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<(), DesktopWin32Error> {
    #[cfg(windows)]
    {
        win32::move_window(hwnd, x, y, width, height)
    }
    #[cfg(not(windows))]
    {
        let _ = (hwnd, x, y, width, height);
        Err(DesktopWin32Error::Unsupported)
    }
}

/// Minimize a window by raw HWND.
///
/// Returns `DesktopWin32Error::Unsupported` on non-Windows.
pub fn minimize_window(hwnd: u64) -> Result<(), DesktopWin32Error> {
    #[cfg(windows)]
    {
        win32::minimize_window(hwnd)
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        Err(DesktopWin32Error::Unsupported)
    }
}

/// Maximize a window by raw HWND.
///
/// Returns `DesktopWin32Error::Unsupported` on non-Windows.
pub fn maximize_window(hwnd: u64) -> Result<(), DesktopWin32Error> {
    #[cfg(windows)]
    {
        win32::maximize_window(hwnd)
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        Err(DesktopWin32Error::Unsupported)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_key_variants_exist() {
        // Verify the enum is well-formed and Debug works.
        let keys = vec![
            VirtualKey::Ctrl,
            VirtualKey::Alt,
            VirtualKey::Shift,
            VirtualKey::Enter,
            VirtualKey::Tab,
            VirtualKey::Escape,
            VirtualKey::F1,
            VirtualKey::Char('A'),
        ];
        for key in &keys {
            let debug = format!("{key:?}");
            assert!(!debug.is_empty());
        }
    }

    #[test]
    fn mouse_button_serializes() {
        let left = serde_json::to_string(&MouseButton::Left);
        assert!(left.is_ok());
    }

    #[test]
    fn error_display() {
        let err = DesktopWin32Error::Unsupported;
        assert_eq!(err.to_string(), "operation not supported on this platform");
    }

    #[cfg(not(windows))]
    #[test]
    fn functions_return_unsupported_on_non_windows() {
        assert!(matches!(
            move_cursor(0, 0),
            Err(DesktopWin32Error::Unsupported)
        ));
        assert!(matches!(
            get_cursor_pos(),
            Err(DesktopWin32Error::Unsupported)
        ));
        assert!(matches!(
            click(0, 0, MouseButton::Left),
            Err(DesktopWin32Error::Unsupported)
        ));
        assert!(matches!(
            type_text("hello"),
            Err(DesktopWin32Error::Unsupported)
        ));
        assert!(matches!(
            send_key_combo(&[VirtualKey::Ctrl]),
            Err(DesktopWin32Error::Unsupported)
        ));
        assert!(matches!(
            focus_window(0),
            Err(DesktopWin32Error::Unsupported)
        ));
        assert!(matches!(
            move_window(0, 0, 0, None, None),
            Err(DesktopWin32Error::Unsupported)
        ));
        assert!(matches!(
            minimize_window(0),
            Err(DesktopWin32Error::Unsupported)
        ));
        assert!(matches!(
            maximize_window(0),
            Err(DesktopWin32Error::Unsupported)
        ));
    }
}
