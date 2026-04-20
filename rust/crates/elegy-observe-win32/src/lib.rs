//! Minimal Win32 FFI wrappers for desktop observation.
//!
//! This crate intentionally allows `unsafe_code` because it wraps raw Win32 API
//! calls. All public functions expose a fully safe typed API. Every `unsafe` block
//! has a `// SAFETY:` comment explaining the invariant.
//!
//! This crate must remain under 500 lines of source. If it grows beyond that,
//! split into focused modules.

use serde::Serialize;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from Win32 observation operations.
#[derive(Debug, Error)]
pub enum Win32Error {
    /// The operation is not supported on this platform.
    #[error("operation not supported on this platform")]
    Unsupported,
    /// A Win32 API call failed.
    #[error("Win32 API call failed: {0}")]
    ApiError(String),
    /// No foreground window was found.
    #[error("no foreground window found")]
    NoForegroundWindow,
    /// The requested monitor was not found.
    #[error("monitor index {0} not found")]
    MonitorNotFound(u32),
    /// A callback panicked during window enumeration.
    #[error("callback panicked during window enumeration")]
    CallbackPanic,
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

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
    /// Raw window handle as a u64 (platform-specific).
    pub hwnd: u64,
    /// Window title text.
    pub title: String,
    /// Process ID that owns this window.
    pub process_id: u32,
    /// Window bounding rectangle.
    pub bounds: Rect,
}

/// Raw screen capture data (RGBA, top-down, row-major).
#[derive(Debug, Clone)]
pub struct RawScreenCapture {
    pub width: u32,
    pub height: u32,
    /// RGBA pixel data. Alpha is normalized to 255.
    pub rgba_data: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Win32 implementation (Windows only)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win32 {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    use crate::{Rect, RawScreenCapture, Win32Error, WindowInfo};

    /// Context passed through the `EnumWindows` callback via LPARAM.
    struct EnumContext {
        windows: Vec<WindowInfo>,
        error: Option<Win32Error>,
    }

    pub fn foreground_window() -> Result<WindowInfo, Win32Error> {
        // SAFETY: GetForegroundWindow returns the HWND of the foreground window
        // or a null handle if none exists. No memory is written.
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0.is_null() {
            return Err(Win32Error::NoForegroundWindow);
        }
        window_info_from_hwnd(hwnd)
    }

    pub fn list_windows() -> Result<Vec<WindowInfo>, Win32Error> {
        let mut ctx = EnumContext {
            windows: Vec::new(),
            error: None,
        };

        let ctx_ptr: *mut EnumContext = &mut ctx;

        // SAFETY: EnumWindows invokes `enum_windows_callback` for each top-level
        // window. `ctx_ptr` points to a stack-allocated `EnumContext` that
        // outlives the `EnumWindows` call. The callback uses `catch_unwind` to
        // prevent panics from crossing the FFI boundary.
        let result = unsafe {
            EnumWindows(Some(enum_windows_callback), LPARAM(ctx_ptr as isize))
        };

        if let Some(err) = ctx.error {
            return Err(err);
        }
        if result.is_err() {
            return Err(Win32Error::ApiError("EnumWindows failed".to_string()));
        }

        Ok(ctx.windows)
    }

    /// Callback invoked by `EnumWindows` for each top-level window.
    ///
    /// # Safety
    ///
    /// `lparam` must be a valid pointer to an `EnumContext` that outlives this
    /// call. The function uses `catch_unwind` so panics do not unwind across
    /// the `extern "system"` FFI boundary (which would be UB).
    unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let result = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: lparam points to a valid, aligned EnumContext that outlives
            // the EnumWindows call. We have exclusive access because EnumWindows
            // is single-threaded and invokes callbacks sequentially.
            let ctx = unsafe { &mut *(lparam.0 as *mut EnumContext) };

            // SAFETY: IsWindowVisible reads window state; no mutation.
            let visible = unsafe { IsWindowVisible(hwnd) };
            if !visible.as_bool() {
                return BOOL(1); // continue enumeration
            }

            match window_info_from_hwnd(hwnd) {
                Ok(info) => {
                    if !info.title.is_empty() {
                        ctx.windows.push(info);
                    }
                    BOOL(1)
                }
                Err(_) => BOOL(1), // skip errors, continue enumeration
            }
        }));

        match result {
            Ok(b) => b,
            Err(_) => {
                // Panic occurred — record error and stop enumeration.
                // SAFETY: same invariant as above — lparam is valid for the
                // duration of EnumWindows.
                let ctx = unsafe { &mut *(lparam.0 as *mut EnumContext) };
                ctx.error = Some(Win32Error::CallbackPanic);
                BOOL(0)
            }
        }
    }

    fn window_info_from_hwnd(hwnd: HWND) -> Result<WindowInfo, Win32Error> {
        let title = get_window_title(hwnd);
        let bounds = get_window_rect_safe(hwnd)?;
        let process_id = get_window_process_id(hwnd);

        Ok(WindowInfo {
            hwnd: hwnd.0 as u64,
            title,
            process_id,
            bounds,
        })
    }

    fn get_window_title(hwnd: HWND) -> String {
        let mut buf = [0u16; 512];
        // SAFETY: GetWindowTextW writes into `buf` up to buf.len() characters.
        // If the title is longer it is truncated, not overflowed.
        let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
        if len <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..len as usize])
    }

    fn get_window_rect_safe(hwnd: HWND) -> Result<Rect, Win32Error> {
        let mut rect = RECT::default();
        // SAFETY: GetWindowRect writes a RECT to our stack-allocated struct.
        unsafe {
            GetWindowRect(hwnd, &mut rect)
                .map_err(|e| Win32Error::ApiError(format!("GetWindowRect: {e}")))?;
        }
        Ok(Rect {
            x: rect.left,
            y: rect.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
        })
    }

    fn get_window_process_id(hwnd: HWND) -> u32 {
        let mut pid = 0u32;
        // SAFETY: GetWindowThreadProcessId writes the owning process ID to `pid`.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        pid
    }

    pub fn capture_screen(monitor: u32) -> Result<RawScreenCapture, Win32Error> {
        // MVP: only primary display (monitor 0).
        if monitor != 0 {
            return Err(Win32Error::MonitorNotFound(monitor));
        }

        // SAFETY: GetDC(None) returns the device context for the entire desktop.
        let hdc_screen = unsafe { GetDC(None) };
        if hdc_screen.is_invalid() {
            return Err(Win32Error::ApiError("GetDC(desktop) failed".to_string()));
        }

        // SAFETY: GetSystemMetrics reads display metrics; no mutation.
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };

        if width <= 0 || height <= 0 {
            // SAFETY: ReleaseDC releases the DC acquired above.
            unsafe { ReleaseDC(None, hdc_screen) };
            return Err(Win32Error::ApiError("invalid screen dimensions".to_string()));
        }

        let result = capture_screen_inner(hdc_screen, width, height);

        // SAFETY: ReleaseDC releases the desktop DC we acquired with GetDC.
        unsafe { ReleaseDC(None, hdc_screen) };

        result
    }

    fn capture_screen_inner(
        hdc_screen: HDC,
        width: i32,
        height: i32,
    ) -> Result<RawScreenCapture, Win32Error> {
        // SAFETY: CreateCompatibleDC creates a memory DC compatible with the
        // screen DC. Must be deleted with DeleteDC when done.
        let hdc_mem = unsafe { CreateCompatibleDC(hdc_screen) };
        if hdc_mem.is_invalid() {
            return Err(Win32Error::ApiError("CreateCompatibleDC failed".to_string()));
        }

        // SAFETY: CreateCompatibleBitmap allocates a bitmap compatible with the
        // screen DC. Must be deleted with DeleteObject when done.
        let hbm = unsafe { CreateCompatibleBitmap(hdc_screen, width, height) };
        if hbm.is_invalid() {
            unsafe { let _ = DeleteDC(hdc_mem); };
            return Err(Win32Error::ApiError(
                "CreateCompatibleBitmap failed".to_string(),
            ));
        }

        // SAFETY: SelectObject selects our bitmap into the memory DC for drawing.
        // The old object is returned so we can restore it before cleanup.
        let old_bm = unsafe { SelectObject(hdc_mem, hbm) };

        // SAFETY: BitBlt copies pixel data from the screen DC to our memory DC.
        let blt_result = unsafe {
            BitBlt(hdc_mem, 0, 0, width, height, hdc_screen, 0, 0, SRCCOPY)
        };

        if blt_result.is_err() {
            unsafe {
                SelectObject(hdc_mem, old_bm);
                let _ = DeleteObject(hbm);
                let _ = DeleteDC(hdc_mem);
            }
            return Err(Win32Error::ApiError("BitBlt failed".to_string()));
        }

        // Request a 32-bit top-down DIB (negative height = top-down row order).
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        // Row stride must be 4-byte aligned.
        let stride = ((width as u32 * 4 + 3) & !3) as usize;
        let mut bgra_data = vec![0u8; stride * height as usize];

        // SAFETY: GetDIBits reads pixel data from our bitmap into `bgra_data`.
        // The buffer is large enough for `height` rows of `stride` bytes each.
        let lines = unsafe {
            GetDIBits(
                hdc_mem,
                hbm,
                0,
                height as u32,
                Some(bgra_data.as_mut_ptr().cast()),
                &mut bmi,
                DIB_RGB_COLORS,
            )
        };

        // Clean up GDI objects.
        unsafe {
            SelectObject(hdc_mem, old_bm);
            let _ = DeleteObject(hbm);
            let _ = DeleteDC(hdc_mem);
        }

        if lines == 0 {
            return Err(Win32Error::ApiError("GetDIBits returned 0 lines".to_string()));
        }

        // Convert BGRA to RGBA and normalize alpha to 255 (desktop captures
        // often have garbage alpha values).
        let w = width as usize;
        let h = height as usize;
        let mut rgba_data = Vec::with_capacity(w * h * 4);

        for y in 0..h {
            let row_start = y * stride;
            for x in 0..w {
                let offset = row_start + x * 4;
                let b = bgra_data[offset];
                let g = bgra_data[offset + 1];
                let r = bgra_data[offset + 2];
                rgba_data.push(r);
                rgba_data.push(g);
                rgba_data.push(b);
                rgba_data.push(255);
            }
        }

        Ok(RawScreenCapture {
            width: width as u32,
            height: height as u32,
            rgba_data,
        })
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Get information about the current foreground (active) window.
///
/// Returns `Win32Error::Unsupported` on non-Windows platforms.
pub fn foreground_window() -> Result<WindowInfo, Win32Error> {
    #[cfg(windows)]
    {
        win32::foreground_window()
    }
    #[cfg(not(windows))]
    {
        Err(Win32Error::Unsupported)
    }
}

/// List all visible top-level windows.
///
/// Returns `Win32Error::Unsupported` on non-Windows platforms.
pub fn list_windows() -> Result<Vec<WindowInfo>, Win32Error> {
    #[cfg(windows)]
    {
        win32::list_windows()
    }
    #[cfg(not(windows))]
    {
        Err(Win32Error::Unsupported)
    }
}

/// Capture the screen as raw RGBA pixel data.
///
/// Currently only supports the primary monitor (`monitor = 0`).
/// Returns `Win32Error::Unsupported` on non-Windows platforms.
pub fn capture_screen(monitor: u32) -> Result<RawScreenCapture, Win32Error> {
    #[cfg(windows)]
    {
        win32::capture_screen(monitor)
    }
    #[cfg(not(windows))]
    {
        let _ = monitor;
        Err(Win32Error::Unsupported)
    }
}
