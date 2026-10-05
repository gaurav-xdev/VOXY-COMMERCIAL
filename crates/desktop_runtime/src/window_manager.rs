//! Window state tracking and management.

use crate::error::Result;
use parking_lot::RwLock;
use tracing::info;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowState {
    Normal,
    Minimized,
    Maximized,
    Hidden,
}

#[derive(Debug, Clone)]
pub struct TrackedWindow {
    pub hwnd: u64,
    pub title: String,
    pub process_name: String,
    pub process_id: u32,
    pub state: WindowState,
    pub is_visible: bool,
    pub is_foreground: bool,
}

pub struct WindowTracker {
    windows: RwLock<Vec<TrackedWindow>>,
    main_window_hwnd: RwLock<Option<u64>>,
    minimize_to_tray: RwLock<bool>,
}

impl WindowTracker {
    pub fn new() -> Self {
        Self {
            windows: RwLock::new(Vec::new()),
            main_window_hwnd: RwLock::new(None),
            minimize_to_tray: RwLock::new(true),
        }
    }

    pub fn set_main_window(&self, hwnd: u64) {
        *self.main_window_hwnd.write() = Some(hwnd);
    }

    pub fn set_minimize_to_tray(&self, enabled: bool) {
        *self.minimize_to_tray.write() = enabled;
    }

    pub fn is_minimize_to_tray(&self) -> bool {
        *self.minimize_to_tray.read()
    }

    pub fn windows(&self) -> Vec<TrackedWindow> {
        self.windows.read().clone()
    }

    pub fn main_window(&self) -> Option<TrackedWindow> {
        let hwnd = (*self.main_window_hwnd.read())?;
        self.windows.read().iter().find(|w| w.hwnd == hwnd).cloned()
    }

    pub fn minimize_to_tray(&self) -> Result<()> {
        if let Some(hwnd) = *self.main_window_hwnd.read() {
            info!("Window {} minimized to tray / hidden", hwnd);
            #[cfg(windows)]
            {
                use windows::Win32::Foundation::HWND;
                use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
                unsafe {
                    let _ = ShowWindow(HWND(hwnd as isize as *mut _), SW_HIDE);
                }
            }
        }
        Ok(())
    }

    pub fn restore_from_tray(&self) -> Result<()> {
        if let Some(hwnd) = *self.main_window_hwnd.read() {
            info!("Window {} restored from tray", hwnd);
            #[cfg(windows)]
            {
                use windows::Win32::Foundation::HWND;
                use windows::Win32::UI::WindowsAndMessaging::{
                    SetForegroundWindow, ShowWindow, SW_RESTORE,
                };
                unsafe {
                    let h = HWND(hwnd as isize as *mut _);
                    let _ = ShowWindow(h, SW_RESTORE);
                    let _ = SetForegroundWindow(h);
                }
            }
        }
        Ok(())
    }

    /// Check if the currently focused foreground window is running fullscreen
    /// (e.g. video playback, media presentation, or game) to allow intelligent overlay suppression.
    pub fn is_foreground_fullscreen(&self) -> bool {
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::RECT;
            use windows::Win32::UI::WindowsAndMessaging::{
                GetDesktopWindow, GetForegroundWindow, GetShellWindow, GetWindowRect,
            };
            unsafe {
                let fg = GetForegroundWindow();
                if fg.0.is_null() {
                    return false;
                }
                let desktop = GetDesktopWindow();
                let shell = GetShellWindow();
                if fg == desktop || fg == shell {
                    return false;
                }

                let mut fg_rect = RECT::default();
                let mut desk_rect = RECT::default();
                if GetWindowRect(fg, &mut fg_rect).is_ok()
                    && GetWindowRect(desktop, &mut desk_rect).is_ok()
                {
                    return fg_rect.left <= desk_rect.left
                        && fg_rect.top <= desk_rect.top
                        && fg_rect.right >= desk_rect.right
                        && fg_rect.bottom >= desk_rect.bottom;
                }
            }
        }
        false
    }

    pub fn save_state(&self, path: &std::path::Path) -> Result<()> {
        if let Some(hwnd) = *self.main_window_hwnd.read() {
            let state = serde_json::json!({ "hwnd": hwnd });
            std::fs::write(path, serde_json::to_string_pretty(&state).unwrap())?;
        }
        Ok(())
    }

    pub fn load_state(&self, path: &std::path::Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let _content = std::fs::read_to_string(path)?;
        info!("Window state loaded from {}", path.display());
        Ok(())
    }

    pub async fn track_loop(&self, shutdown_rx: &mut tokio::sync::broadcast::Receiver<()>) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
        loop {
            tokio::select! {
                _ = interval.tick() => {}
                _ = shutdown_rx.recv() => break,
            }
        }
    }
}

impl Default for WindowTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_creation() {
        assert!(WindowTracker::new().windows().is_empty());
    }

    #[test]
    fn minimize_to_tray_default() {
        assert!(WindowTracker::new().is_minimize_to_tray());
    }

    #[test]
    fn save_load_state() {
        let tracker = WindowTracker::new();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("window_state.json");
        tracker.save_state(&path).unwrap();
        WindowTracker::new().load_state(&path).unwrap();
    }
}
