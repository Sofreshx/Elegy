//! Safe desktop automation for agentic workflows.
//!
//! Provides mouse clicks, keyboard input, and window management with:
//! - **Dry-run support**: preview actions without executing them
//! - **Evidence capture**: before/after foreground window state
//! - **Title-based window lookup**: resolves human-readable titles to HWNDs
//! - **Strict matching**: fails on ambiguous (>1) or zero window matches
//!
//! Platform support:
//! - Windows: full functionality via `elegy-desktop-win32`
//! - Other platforms: returns `DesktopError::Unsupported`

use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from desktop automation operations.
#[derive(Debug, thiserror::Error)]
pub enum DesktopError {
    /// The operation is not supported on this platform.
    #[error("not supported on this platform")]
    Unsupported,
    /// A Win32 API call failed (Windows only).
    #[error("win32 error: {0}")]
    Win32(String),
    /// No window matched the given title pattern.
    #[error("no window matched title pattern: \"{0}\"")]
    NoWindowMatch(String),
    /// Multiple windows matched — ambiguous target.
    #[error("ambiguous: {0} windows matched title pattern \"{1}\" — use --hwnd for precision")]
    AmbiguousMatch(usize, String),
    /// Key combo parsing failed.
    #[error("invalid key combo: {0}")]
    InvalidKeyCombo(String),
    /// An observation error occurred during evidence capture.
    #[error("observation error: {0}")]
    ObservationError(String),
}

impl From<elegy_desktop_win32::DesktopWin32Error> for DesktopError {
    fn from(e: elegy_desktop_win32::DesktopWin32Error) -> Self {
        match e {
            elegy_desktop_win32::DesktopWin32Error::Unsupported => DesktopError::Unsupported,
            elegy_desktop_win32::DesktopWin32Error::WindowNotFound => {
                DesktopError::NoWindowMatch("(hwnd not found)".to_string())
            }
            other => DesktopError::Win32(other.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Result types (all Serialize for JSON envelope)
// ---------------------------------------------------------------------------

/// Result of a mouse click action.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickResult {
    pub action: String,
    pub x: i32,
    pub y: i32,
    pub button: String,
    pub dry_run: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_window: Option<String>,
    pub executed_at_utc: String,
}

/// Result of a text typing action.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeResult {
    pub action: String,
    pub text: String,
    pub character_count: usize,
    pub dry_run: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_window: Option<String>,
    pub executed_at_utc: String,
}

/// Result of a key combo action.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyResult {
    pub action: String,
    pub combo: String,
    pub keys: Vec<String>,
    pub dry_run: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_window: Option<String>,
    pub executed_at_utc: String,
}

/// Result of a window management action.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowActionResult {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hwnd: Option<u64>,
    pub dry_run: bool,
    pub executed_at_utc: String,
}

// ---------------------------------------------------------------------------
// Evidence capture helpers
// ---------------------------------------------------------------------------

/// Best-effort capture of the current foreground window title.
fn capture_foreground_title() -> Option<String> {
    elegy_observe::foreground_window().ok().map(|w| w.title)
}

fn utc_now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Window resolution (title → HWND)
// ---------------------------------------------------------------------------

/// Resolve a title pattern to a single window HWND.
///
/// Fails if zero or multiple windows match (strict matching for safety).
fn resolve_window_by_title(title: &str) -> Result<(u64, String), DesktopError> {
    let windows = elegy_observe::list_windows(Some(title))
        .map_err(|e| DesktopError::ObservationError(e.to_string()))?;

    match windows.len() {
        0 => Err(DesktopError::NoWindowMatch(title.to_string())),
        1 => {
            let w = &windows[0];
            Ok((w.hwnd, w.title.clone()))
        }
        n => Err(DesktopError::AmbiguousMatch(n, title.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Public API — Mouse
// ---------------------------------------------------------------------------

/// Simulate a mouse click at pixel coordinates.
///
/// In dry-run mode, captures the target context but does not inject input.
/// Evidence: captures foreground window title as best-effort.
pub fn click(x: i32, y: i32, button: &str, dry_run: bool) -> Result<ClickResult, DesktopError> {
    let btn = parse_mouse_button(button)?;
    let target_window = capture_foreground_title();

    if !dry_run {
        elegy_desktop_win32::click(x, y, btn)?;
    }

    Ok(ClickResult {
        action: "click".to_string(),
        x,
        y,
        button: button.to_string(),
        dry_run,
        target_window,
        executed_at_utc: utc_now_rfc3339(),
    })
}

// ---------------------------------------------------------------------------
// Public API — Keyboard
// ---------------------------------------------------------------------------

/// Type text by injecting Unicode key events.
///
/// In dry-run mode, reports what would be typed without injecting input.
pub fn type_text(text: &str, dry_run: bool) -> Result<TypeResult, DesktopError> {
    let target_window = capture_foreground_title();

    if !dry_run {
        elegy_desktop_win32::type_text(text)?;
    }

    Ok(TypeResult {
        action: "type".to_string(),
        text: text.to_string(),
        character_count: text.chars().count(),
        dry_run,
        target_window,
        executed_at_utc: utc_now_rfc3339(),
    })
}

/// Send a key combination (e.g., "ctrl+s", "alt+tab", "enter").
///
/// In dry-run mode, parses and validates the combo without injecting input.
pub fn send_key(combo: &str, dry_run: bool) -> Result<KeyResult, DesktopError> {
    let parsed = parse_key_combo(combo)?;
    let key_names: Vec<String> = parsed.iter().map(|k| format!("{k:?}")).collect();
    let target_window = capture_foreground_title();

    if !dry_run {
        let vkeys: Vec<elegy_desktop_win32::VirtualKey> = parsed
            .into_iter()
            .map(to_win32_vk)
            .collect::<Result<_, _>>()?;
        elegy_desktop_win32::send_key_combo(&vkeys)?;
    }

    Ok(KeyResult {
        action: "key".to_string(),
        combo: combo.to_string(),
        keys: key_names,
        dry_run,
        target_window,
        executed_at_utc: utc_now_rfc3339(),
    })
}

// ---------------------------------------------------------------------------
// Public API — Window management
// ---------------------------------------------------------------------------

/// Focus a window by title pattern or raw HWND.
pub fn focus_window(
    title: Option<&str>,
    hwnd: Option<u64>,
    dry_run: bool,
) -> Result<WindowActionResult, DesktopError> {
    let (resolved_hwnd, matched_title) = resolve_target(title, hwnd)?;

    if !dry_run {
        elegy_desktop_win32::focus_window(resolved_hwnd)?;
    }

    Ok(WindowActionResult {
        action: "focus".to_string(),
        title_pattern: title.map(String::from),
        matched_title: Some(matched_title),
        hwnd: Some(resolved_hwnd),
        dry_run,
        executed_at_utc: utc_now_rfc3339(),
    })
}

/// Move and optionally resize a window by title pattern or raw HWND.
pub fn move_window(
    title: Option<&str>,
    hwnd: Option<u64>,
    x: i32,
    y: i32,
    width: Option<u32>,
    height: Option<u32>,
    dry_run: bool,
) -> Result<WindowActionResult, DesktopError> {
    let (resolved_hwnd, matched_title) = resolve_target(title, hwnd)?;

    if !dry_run {
        elegy_desktop_win32::move_window(resolved_hwnd, x, y, width, height)?;
    }

    Ok(WindowActionResult {
        action: "move".to_string(),
        title_pattern: title.map(String::from),
        matched_title: Some(matched_title),
        hwnd: Some(resolved_hwnd),
        dry_run,
        executed_at_utc: utc_now_rfc3339(),
    })
}

/// Minimize a window by title pattern or raw HWND.
pub fn minimize_window(
    title: Option<&str>,
    hwnd: Option<u64>,
    dry_run: bool,
) -> Result<WindowActionResult, DesktopError> {
    let (resolved_hwnd, matched_title) = resolve_target(title, hwnd)?;

    if !dry_run {
        elegy_desktop_win32::minimize_window(resolved_hwnd)?;
    }

    Ok(WindowActionResult {
        action: "minimize".to_string(),
        title_pattern: title.map(String::from),
        matched_title: Some(matched_title),
        hwnd: Some(resolved_hwnd),
        dry_run,
        executed_at_utc: utc_now_rfc3339(),
    })
}

/// Maximize a window by title pattern or raw HWND.
pub fn maximize_window(
    title: Option<&str>,
    hwnd: Option<u64>,
    dry_run: bool,
) -> Result<WindowActionResult, DesktopError> {
    let (resolved_hwnd, matched_title) = resolve_target(title, hwnd)?;

    if !dry_run {
        elegy_desktop_win32::maximize_window(resolved_hwnd)?;
    }

    Ok(WindowActionResult {
        action: "maximize".to_string(),
        title_pattern: title.map(String::from),
        matched_title: Some(matched_title),
        hwnd: Some(resolved_hwnd),
        dry_run,
        executed_at_utc: utc_now_rfc3339(),
    })
}

// ---------------------------------------------------------------------------
// Internal: target resolution
// ---------------------------------------------------------------------------

fn resolve_target(title: Option<&str>, hwnd: Option<u64>) -> Result<(u64, String), DesktopError> {
    match (title, hwnd) {
        (_, Some(h)) => {
            // HWND takes priority — use it directly. Title is best-effort.
            let title_str = title.unwrap_or("(resolved by hwnd)").to_string();
            Ok((h, title_str))
        }
        (Some(t), None) => resolve_window_by_title(t),
        (None, None) => Err(DesktopError::NoWindowMatch(
            "(no title or hwnd specified)".to_string(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Key combo parser
// ---------------------------------------------------------------------------

/// A parsed key from a combo string.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ParsedKey {
    Ctrl,
    Alt,
    Shift,
    Win,
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
    Up,
    Down,
    Left,
    Right,
    F(u8),
    Char(char),
}

fn parse_key_combo(combo: &str) -> Result<Vec<ParsedKey>, DesktopError> {
    let parts: Vec<&str> = combo.split('+').map(|s| s.trim()).collect();
    let mut keys = Vec::new();

    for part in parts {
        let key = match part.to_lowercase().as_str() {
            "ctrl" | "control" => ParsedKey::Ctrl,
            "alt" => ParsedKey::Alt,
            "shift" => ParsedKey::Shift,
            "win" | "super" | "meta" => ParsedKey::Win,
            "enter" | "return" => ParsedKey::Enter,
            "tab" => ParsedKey::Tab,
            "esc" | "escape" => ParsedKey::Escape,
            "backspace" | "bs" => ParsedKey::Backspace,
            "delete" | "del" => ParsedKey::Delete,
            "space" => ParsedKey::Space,
            "home" => ParsedKey::Home,
            "end" => ParsedKey::End,
            "pageup" | "pgup" => ParsedKey::PageUp,
            "pagedown" | "pgdn" => ParsedKey::PageDown,
            "up" => ParsedKey::Up,
            "down" => ParsedKey::Down,
            "left" => ParsedKey::Left,
            "right" => ParsedKey::Right,
            "f1" => ParsedKey::F(1),
            "f2" => ParsedKey::F(2),
            "f3" => ParsedKey::F(3),
            "f4" => ParsedKey::F(4),
            "f5" => ParsedKey::F(5),
            "f6" => ParsedKey::F(6),
            "f7" => ParsedKey::F(7),
            "f8" => ParsedKey::F(8),
            "f9" => ParsedKey::F(9),
            "f10" => ParsedKey::F(10),
            "f11" => ParsedKey::F(11),
            "f12" => ParsedKey::F(12),
            s if s.len() == 1 => {
                let c = s.chars().next().unwrap_or('?');
                if c.is_ascii_alphanumeric() {
                    ParsedKey::Char(c.to_ascii_uppercase())
                } else {
                    return Err(DesktopError::InvalidKeyCombo(format!(
                        "unsupported key: '{c}' — MVP supports A-Z, 0-9, modifiers, and navigation keys"
                    )));
                }
            }
            other => {
                return Err(DesktopError::InvalidKeyCombo(format!(
                    "unknown key name: \"{other}\""
                )));
            }
        };
        keys.push(key);
    }

    if keys.is_empty() {
        return Err(DesktopError::InvalidKeyCombo("empty key combo".to_string()));
    }

    Ok(keys)
}

/// Convert a `ParsedKey` to the win32 crate's `VirtualKey`.
fn to_win32_vk(key: ParsedKey) -> Result<elegy_desktop_win32::VirtualKey, DesktopError> {
    use elegy_desktop_win32::VirtualKey;
    match key {
        ParsedKey::Ctrl => Ok(VirtualKey::Ctrl),
        ParsedKey::Alt => Ok(VirtualKey::Alt),
        ParsedKey::Shift => Ok(VirtualKey::Shift),
        ParsedKey::Win => Ok(VirtualKey::Win),
        ParsedKey::Enter => Ok(VirtualKey::Enter),
        ParsedKey::Tab => Ok(VirtualKey::Tab),
        ParsedKey::Escape => Ok(VirtualKey::Escape),
        ParsedKey::Backspace => Ok(VirtualKey::Backspace),
        ParsedKey::Delete => Ok(VirtualKey::Delete),
        ParsedKey::Space => Ok(VirtualKey::Space),
        ParsedKey::Home => Ok(VirtualKey::Home),
        ParsedKey::End => Ok(VirtualKey::End),
        ParsedKey::PageUp => Ok(VirtualKey::PageUp),
        ParsedKey::PageDown => Ok(VirtualKey::PageDown),
        ParsedKey::Up => Ok(VirtualKey::Up),
        ParsedKey::Down => Ok(VirtualKey::Down),
        ParsedKey::Left => Ok(VirtualKey::Left),
        ParsedKey::Right => Ok(VirtualKey::Right),
        ParsedKey::F(1) => Ok(VirtualKey::F1),
        ParsedKey::F(2) => Ok(VirtualKey::F2),
        ParsedKey::F(3) => Ok(VirtualKey::F3),
        ParsedKey::F(4) => Ok(VirtualKey::F4),
        ParsedKey::F(5) => Ok(VirtualKey::F5),
        ParsedKey::F(6) => Ok(VirtualKey::F6),
        ParsedKey::F(7) => Ok(VirtualKey::F7),
        ParsedKey::F(8) => Ok(VirtualKey::F8),
        ParsedKey::F(9) => Ok(VirtualKey::F9),
        ParsedKey::F(10) => Ok(VirtualKey::F10),
        ParsedKey::F(11) => Ok(VirtualKey::F11),
        ParsedKey::F(12) => Ok(VirtualKey::F12),
        ParsedKey::F(n) => Err(DesktopError::InvalidKeyCombo(format!(
            "unsupported F-key: F{n}"
        ))),
        ParsedKey::Char(c) => Ok(VirtualKey::Char(c)),
    }
}

// ---------------------------------------------------------------------------
// Mouse button parser
// ---------------------------------------------------------------------------

fn parse_mouse_button(s: &str) -> Result<elegy_desktop_win32::MouseButton, DesktopError> {
    match s.to_lowercase().as_str() {
        "left" => Ok(elegy_desktop_win32::MouseButton::Left),
        "right" => Ok(elegy_desktop_win32::MouseButton::Right),
        "middle" => Ok(elegy_desktop_win32::MouseButton::Middle),
        other => Err(DesktopError::InvalidKeyCombo(format!(
            "unknown mouse button: \"{other}\" — use left, right, or middle"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_key_combo_single_key() {
        let keys = parse_key_combo("enter");
        assert!(keys.is_ok());
        #[expect(clippy::unwrap_used)]
        let keys = keys.unwrap();
        assert_eq!(keys, vec![ParsedKey::Enter]);
    }

    #[test]
    fn parse_key_combo_ctrl_s() {
        let keys = parse_key_combo("ctrl+s");
        assert!(keys.is_ok());
        #[expect(clippy::unwrap_used)]
        let keys = keys.unwrap();
        assert_eq!(keys, vec![ParsedKey::Ctrl, ParsedKey::Char('S')]);
    }

    #[test]
    fn parse_key_combo_ctrl_shift_f5() {
        let keys = parse_key_combo("ctrl+shift+f5");
        assert!(keys.is_ok());
        #[expect(clippy::unwrap_used)]
        let keys = keys.unwrap();
        assert_eq!(
            keys,
            vec![ParsedKey::Ctrl, ParsedKey::Shift, ParsedKey::F(5)]
        );
    }

    #[test]
    fn parse_key_combo_alt_tab() {
        let keys = parse_key_combo("alt+tab");
        assert!(keys.is_ok());
        #[expect(clippy::unwrap_used)]
        let keys = keys.unwrap();
        assert_eq!(keys, vec![ParsedKey::Alt, ParsedKey::Tab]);
    }

    #[test]
    fn parse_key_combo_rejects_empty() {
        let keys = parse_key_combo("");
        assert!(keys.is_err());
    }

    #[test]
    fn parse_key_combo_rejects_unsupported() {
        let keys = parse_key_combo("ctrl+@");
        assert!(keys.is_err());
    }

    #[test]
    fn parse_mouse_button_valid() {
        assert!(parse_mouse_button("left").is_ok());
        assert!(parse_mouse_button("RIGHT").is_ok());
        assert!(parse_mouse_button("Middle").is_ok());
    }

    #[test]
    fn parse_mouse_button_invalid() {
        assert!(parse_mouse_button("unknown").is_err());
    }

    #[test]
    fn click_result_serializes() {
        let result = ClickResult {
            action: "click".to_string(),
            x: 100,
            y: 200,
            button: "left".to_string(),
            dry_run: true,
            target_window: Some("Test".to_string()),
            executed_at_utc: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&result);
        assert!(json.is_ok());
    }

    #[test]
    fn error_display_messages() {
        assert_eq!(
            DesktopError::NoWindowMatch("Test".to_string()).to_string(),
            "no window matched title pattern: \"Test\""
        );
        assert!(DesktopError::AmbiguousMatch(3, "Code".to_string())
            .to_string()
            .contains("3 windows matched"));
    }

    // Dry-run tests — these work on all platforms since dry-run skips
    // the platform-specific code path.
    #[test]
    fn click_dry_run_succeeds() {
        let result = click(100, 200, "left", true);
        assert!(result.is_ok());
        #[expect(clippy::unwrap_used)]
        let r = result.unwrap();
        assert!(r.dry_run);
        assert_eq!(r.action, "click");
    }

    #[test]
    fn type_text_dry_run_succeeds() {
        let result = type_text("hello", true);
        assert!(result.is_ok());
        #[expect(clippy::unwrap_used)]
        let r = result.unwrap();
        assert!(r.dry_run);
        assert_eq!(r.character_count, 5);
    }

    #[test]
    fn send_key_dry_run_succeeds() {
        let result = send_key("ctrl+s", true);
        assert!(result.is_ok());
        #[expect(clippy::unwrap_used)]
        let r = result.unwrap();
        assert!(r.dry_run);
        assert_eq!(r.combo, "ctrl+s");
    }
}
