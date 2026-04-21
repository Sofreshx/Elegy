//! Desktop and OS observation for agentic workflows.
//!
//! Provides safe, structured observation of desktop state:
//! - Process snapshots (cross-platform via `sysinfo`)
//! - System information (cross-platform via `sysinfo`)
//! - Clipboard contents (cross-platform via `arboard`)
//! - Active/foreground window info (Windows only)
//! - Window enumeration (Windows only)
//! - Screen capture to PNG (Windows only)
//! - Filesystem snapshot diff (cross-platform)

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from observation operations.
#[derive(Debug, thiserror::Error)]
pub enum ObserveError {
    /// The operation is not supported on this platform.
    #[error("not supported on this platform")]
    Unsupported,
    /// A Win32 API call failed (Windows only).
    #[error("win32 error: {0}")]
    Win32(String),
    /// Clipboard access failed.
    #[error("clipboard error: {0}")]
    Clipboard(String),
    /// PNG encoding failed.
    #[error("PNG encoding error: {0}")]
    PngEncode(String),
    /// Filesystem operation failed.
    #[error("filesystem error: {0}")]
    Filesystem(#[from] std::io::Error),
}

#[cfg(windows)]
impl From<elegy_observe_win32::Win32Error> for ObserveError {
    fn from(e: elegy_observe_win32::Win32Error) -> Self {
        match e {
            elegy_observe_win32::Win32Error::Unsupported => ObserveError::Unsupported,
            other => ObserveError::Win32(other.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Re-export window types (platform-gated)
// ---------------------------------------------------------------------------

// On Windows, re-export the win32 types directly.
#[cfg(windows)]
pub use elegy_observe_win32::{Rect, WindowInfo};

// On non-Windows, provide compatible stub types.
#[cfg(not(windows))]
mod window_types {
    use serde::Serialize;

    /// Axis-aligned rectangle.
    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Rect {
        pub x: i32,
        pub y: i32,
        pub width: i32,
        pub height: i32,
    }

    /// Information about a visible desktop window.
    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct WindowInfo {
        pub hwnd: u64,
        pub title: String,
        pub process_id: u32,
        pub bounds: Rect,
    }
}

#[cfg(not(windows))]
pub use window_types::{Rect, WindowInfo};

// ---------------------------------------------------------------------------
// Process observation (cross-platform)
// ---------------------------------------------------------------------------

/// Information about a single running process.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    /// Process ID.
    pub pid: u32,
    /// Process name (executable name).
    pub name: String,
    /// Memory usage in megabytes.
    pub memory_mb: f64,
    /// CPU usage as a percentage (0-100).
    pub cpu_percent: f32,
}

/// A snapshot of running processes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSnapshot {
    /// List of matched processes.
    pub processes: Vec<ProcessInfo>,
    /// UTC timestamp when the snapshot was taken.
    pub snapshot_at_utc: String,
}

/// Take a snapshot of running processes, optionally filtered by name pattern.
pub fn snapshot_processes(filter: Option<&str>) -> ProcessSnapshot {
    use sysinfo::System;

    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let filter_lower = filter.map(|f| f.to_lowercase());

    let processes: Vec<ProcessInfo> = sys
        .processes()
        .values()
        .filter(|p| {
            if let Some(ref f) = filter_lower {
                p.name().to_string_lossy().to_lowercase().contains(f)
            } else {
                true
            }
        })
        .map(|p| ProcessInfo {
            pid: p.pid().as_u32(),
            name: p.name().to_string_lossy().to_string(),
            memory_mb: p.memory() as f64 / (1024.0 * 1024.0),
            cpu_percent: p.cpu_usage(),
        })
        .collect();

    ProcessSnapshot {
        processes,
        snapshot_at_utc: utc_now_rfc3339(),
    }
}

// ---------------------------------------------------------------------------
// System information (cross-platform)
// ---------------------------------------------------------------------------

/// System hardware and OS information snapshot.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSnapshot {
    /// OS name (e.g. "Windows 11").
    pub os_name: String,
    /// OS version string.
    pub os_version: String,
    /// Hostname.
    pub hostname: String,
    /// Total physical memory in megabytes.
    pub total_memory_mb: u64,
    /// Used physical memory in megabytes.
    pub used_memory_mb: u64,
    /// Number of logical CPUs.
    pub cpu_count: usize,
    /// UTC timestamp.
    pub snapshot_at_utc: String,
}

/// Get a snapshot of system information.
pub fn system_info() -> SystemSnapshot {
    use sysinfo::System;

    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();

    SystemSnapshot {
        os_name: System::name().unwrap_or_default(),
        os_version: System::os_version().unwrap_or_default(),
        hostname: System::host_name().unwrap_or_default(),
        total_memory_mb: sys.total_memory() / (1024 * 1024),
        used_memory_mb: sys.used_memory() / (1024 * 1024),
        cpu_count: sys.cpus().len(),
        snapshot_at_utc: utc_now_rfc3339(),
    }
}

// ---------------------------------------------------------------------------
// Clipboard (cross-platform via arboard)
// ---------------------------------------------------------------------------

/// Contents of the system clipboard.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardContents {
    /// Text content (if clipboard contains text).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Whether the clipboard contains an image.
    pub has_image: bool,
    /// UTC timestamp.
    pub read_at_utc: String,
}

/// Read the current clipboard contents.
pub fn read_clipboard() -> Result<ClipboardContents, ObserveError> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| ObserveError::Clipboard(e.to_string()))?;

    let text = clipboard.get_text().ok();
    let has_image = clipboard.get_image().is_ok();

    Ok(ClipboardContents {
        text,
        has_image,
        read_at_utc: utc_now_rfc3339(),
    })
}

// ---------------------------------------------------------------------------
// Window observation (delegates to win32 on Windows)
// ---------------------------------------------------------------------------

/// Get information about the foreground (active) window.
pub fn foreground_window() -> Result<WindowInfo, ObserveError> {
    #[cfg(windows)]
    {
        Ok(elegy_observe_win32::foreground_window()?)
    }
    #[cfg(not(windows))]
    {
        Err(ObserveError::Unsupported)
    }
}

/// List all visible top-level windows, optionally filtered by title.
pub fn list_windows(filter: Option<&str>) -> Result<Vec<WindowInfo>, ObserveError> {
    #[cfg(windows)]
    {
        let windows = elegy_observe_win32::list_windows()?;
        match filter {
            Some(f) => {
                let f_lower = f.to_lowercase();
                Ok(windows
                    .into_iter()
                    .filter(|w| w.title.to_lowercase().contains(&f_lower))
                    .collect())
            }
            None => Ok(windows),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = filter;
        Err(ObserveError::Unsupported)
    }
}

// ---------------------------------------------------------------------------
// Screen capture (delegates to win32 on Windows, encodes PNG)
// ---------------------------------------------------------------------------

/// Result of a screen capture operation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenCaptureResult {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Monitor index captured.
    pub monitor: u32,
    /// Output file path (if saved to file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    /// Base64-encoded PNG data (if not saved to file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub png_base64: Option<String>,
    /// UTC timestamp.
    pub captured_at_utc: String,
}

/// Capture the screen and return the result.
///
/// If `output` is `Some`, saves the PNG to that path and returns metadata.
/// If `output` is `None`, returns the PNG as base64-encoded data.
pub fn capture_screen(
    monitor: Option<u32>,
    output: Option<&Path>,
) -> Result<ScreenCaptureResult, ObserveError> {
    let monitor_idx = monitor.unwrap_or(0);

    #[cfg(windows)]
    {
        let raw = elegy_observe_win32::capture_screen(monitor_idx)?;
        let png_data = encode_rgba_to_png(raw.width, raw.height, &raw.rgba_data)?;

        let (output_path, png_base64) = if let Some(path) = output {
            std::fs::write(path, &png_data)?;
            (Some(path.to_string_lossy().to_string()), None)
        } else {
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD.encode(&png_data);
            (None, Some(encoded))
        };

        Ok(ScreenCaptureResult {
            width: raw.width,
            height: raw.height,
            monitor: monitor_idx,
            output_path,
            png_base64,
            captured_at_utc: utc_now_rfc3339(),
        })
    }
    #[cfg(not(windows))]
    {
        let _ = (monitor_idx, output);
        Err(ObserveError::Unsupported)
    }
}

/// Encode raw RGBA pixel data to PNG format.
#[cfg(windows)]
fn encode_rgba_to_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, ObserveError> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut encoder = png::Encoder::new(&mut buf, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| ObserveError::PngEncode(e.to_string()))?;
        writer
            .write_image_data(rgba)
            .map_err(|e| ObserveError::PngEncode(e.to_string()))?;
    }
    Ok(buf.into_inner())
}

// ---------------------------------------------------------------------------
// Filesystem snapshot diff (cross-platform)
// ---------------------------------------------------------------------------

/// Metadata about a file at a point in time.
#[derive(Debug, Clone)]
struct FileSnapshot {
    size: u64,
    modified: std::time::SystemTime,
    #[allow(dead_code)]
    is_dir: bool,
}

/// A changed file detected by filesystem diff.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsChange {
    /// Relative path of the changed file.
    pub path: String,
    /// Type of change.
    pub change_type: String,
}

/// Result of a bounded filesystem observation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsDiffResult {
    /// Path that was observed.
    pub watched_path: String,
    /// Duration of observation in seconds.
    pub duration_seconds: u64,
    /// Changes detected.
    pub changes: Vec<FsChange>,
    /// UTC timestamp when observation started.
    pub started_at_utc: String,
    /// UTC timestamp when observation ended.
    pub ended_at_utc: String,
}

/// Observe a filesystem path for changes over a bounded time window.
///
/// Takes a snapshot of the directory, waits for `timeout`, then takes another
/// snapshot and returns the diff. This is a bounded observation, not a
/// continuous watch.
pub fn observe_filesystem(path: &Path, timeout: Duration) -> Result<FsDiffResult, ObserveError> {
    let started = utc_now_rfc3339();
    let before = snapshot_directory(path)?;

    std::thread::sleep(timeout);

    let ended = utc_now_rfc3339();
    let after = snapshot_directory(path)?;

    let mut changes = Vec::new();

    // Detect created and modified files.
    for (file_path, after_snap) in &after {
        match before.get(file_path) {
            None => {
                changes.push(FsChange {
                    path: file_path.to_string_lossy().to_string(),
                    change_type: "created".to_string(),
                });
            }
            Some(before_snap) => {
                if before_snap.size != after_snap.size
                    || before_snap.modified != after_snap.modified
                {
                    changes.push(FsChange {
                        path: file_path.to_string_lossy().to_string(),
                        change_type: "modified".to_string(),
                    });
                }
            }
        }
    }

    // Detect deleted files.
    for file_path in before.keys() {
        if !after.contains_key(file_path) {
            changes.push(FsChange {
                path: file_path.to_string_lossy().to_string(),
                change_type: "deleted".to_string(),
            });
        }
    }

    Ok(FsDiffResult {
        watched_path: path.to_string_lossy().to_string(),
        duration_seconds: timeout.as_secs(),
        changes,
        started_at_utc: started,
        ended_at_utc: ended,
    })
}

fn snapshot_directory(path: &Path) -> Result<HashMap<PathBuf, FileSnapshot>, std::io::Error> {
    let mut map = HashMap::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        map.insert(
            entry.path(),
            FileSnapshot {
                size: metadata.len(),
                modified: metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                is_dir: metadata.is_dir(),
            },
        );
    }
    Ok(map)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn utc_now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_processes_returns_results() {
        let snap = snapshot_processes(None);
        assert!(
            !snap.processes.is_empty(),
            "should find at least one process"
        );
        assert!(!snap.snapshot_at_utc.is_empty());
    }

    #[test]
    fn snapshot_processes_with_filter() {
        // Filter for a process that definitely exists: the test runner itself
        let snap = snapshot_processes(Some("cargo"));
        // May or may not find cargo depending on how tests are run
        assert!(!snap.snapshot_at_utc.is_empty());
    }

    #[test]
    fn system_info_returns_populated_data() {
        let info = system_info();
        assert!(info.total_memory_mb > 0);
        assert!(info.cpu_count > 0);
        assert!(!info.snapshot_at_utc.is_empty());
    }

    #[test]
    fn clipboard_read_does_not_panic() {
        // Clipboard may or may not be accessible in CI/test environments
        let _ = read_clipboard();
    }

    #[test]
    fn observe_filesystem_detects_no_changes() {
        let dir = std::env::temp_dir();
        // Very short timeout — just verify the function works
        let result = observe_filesystem(&dir, Duration::from_millis(10));
        assert!(result.is_ok());
        #[expect(clippy::unwrap_used)]
        let diff = result.unwrap();
        assert!(!diff.started_at_utc.is_empty());
        assert!(!diff.ended_at_utc.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn foreground_window_returns_info() {
        // In a desktop environment, there should be a foreground window
        let result = foreground_window();
        // May fail in headless CI, so just check it doesn't panic
        if let Ok(info) = result {
            assert!(!info.title.is_empty() || info.hwnd > 0);
        }
    }

    #[cfg(windows)]
    #[test]
    fn list_windows_returns_results() {
        let result = list_windows(None);
        if let Ok(windows) = result {
            // In a desktop environment, there should be at least one window
            assert!(!windows.is_empty());
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn window_functions_return_unsupported() {
        assert!(matches!(
            foreground_window(),
            Err(ObserveError::Unsupported)
        ));
        assert!(matches!(list_windows(None), Err(ObserveError::Unsupported)));
    }
}
