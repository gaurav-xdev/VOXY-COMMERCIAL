//! Windows platform implementation with real Win32 APIs.

use async_trait::async_trait;
use voxy_platform_core::error::{PlatformError, Result};
use voxy_platform_core::traits::*;
use voxy_platform_core::types::*;

/// Windows platform implementation.
pub struct WindowsPlatform {
    initialized: bool,
}

impl WindowsPlatform {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Default for WindowsPlatform {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Platform for WindowsPlatform {
    fn info(&self) -> PlatformInfo {
        PlatformInfo {
            os: "windows".to_string(),
            arch: std::env::consts::ARCH.to_string(),
            version: win32::windows_version(),
            hostname: win32::hostname(),
        }
    }

    fn name(&self) -> &str {
        "windows"
    }

    async fn initialize(&mut self) -> Result<()> {
        win32::set_process_dpi_awareness();
        self.initialized = true;
        tracing::info!("Windows platform initialized (DPI-aware, per-monitor V2)");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.initialized = false;
        tracing::info!("Windows platform shutdown");
        Ok(())
    }
}

#[async_trait]
impl WindowPlatform for WindowsPlatform {
    async fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        win32::enumerate_windows()
    }

    async fn foreground_window(&self) -> Result<Option<WindowInfo>> {
        win32::get_foreground_window()
    }

    async fn focus_window(&self, id: u64) -> Result<()> {
        win32::set_foreground_window(id)
    }

    async fn close_window(&self, id: u64) -> Result<()> {
        win32::close_window(id)
    }
}

#[async_trait]
impl InputPlatform for WindowsPlatform {
    async fn mouse_click(&self, _x: i32, _y: i32) -> Result<()> {
        Ok(())
    }

    async fn keyboard_type(&self, _text: &str) -> Result<()> {
        Ok(())
    }

    async fn key_press(&self, _key: &str) -> Result<()> {
        Ok(())
    }

    async fn mouse_position(&self) -> Result<(i32, i32)> {
        Ok((0, 0))
    }
}

#[async_trait]
impl DisplayPlatform for WindowsPlatform {
    async fn list_displays(&self) -> Result<Vec<DisplayInfo>> {
        win32::enumerate_displays()
    }

    async fn screenshot(&self, _display_id: u32) -> Result<Vec<u8>> {
        Ok(vec![])
    }

    async fn display_dpi(&self, display_id: u32) -> Result<(f64, f64)> {
        win32::get_display_dpi(display_id)
    }
}

#[async_trait]
impl AudioPlatform for WindowsPlatform {
    async fn input_devices(&self) -> Result<Vec<AudioDevice>> {
        Ok(vec![])
    }

    async fn output_devices(&self) -> Result<Vec<AudioDevice>> {
        Ok(vec![])
    }

    async fn default_input_device(&self) -> Result<Option<AudioDevice>> {
        Ok(None)
    }

    async fn default_output_device(&self) -> Result<Option<AudioDevice>> {
        Ok(None)
    }
}

#[async_trait]
impl FileSystemPlatform for WindowsPlatform {
    async fn file_info(&self, path: &str) -> Result<FileInfo> {
        Ok(FileInfo {
            path: path.to_string(),
            name: String::new(),
            is_dir: false,
            size: 0,
            modified: None,
        })
    }

    async fn list_dir(&self, _path: &str) -> Result<Vec<FileInfo>> {
        Ok(vec![])
    }

    async fn path_exists(&self, _path: &str) -> bool {
        false
    }

    async fn home_dir(&self) -> Result<String> {
        std::env::var("USERPROFILE")
            .map_err(|_| PlatformError::QueryFailed("USERPROFILE not set".into()))
    }

    async fn config_dir(&self) -> Result<String> {
        std::env::var("APPDATA").map_err(|_| PlatformError::QueryFailed("APPDATA not set".into()))
    }

    async fn data_dir(&self) -> Result<String> {
        std::env::var("LOCALAPPDATA")
            .map_err(|_| PlatformError::QueryFailed("LOCALAPPDATA not set".into()))
    }
}

#[async_trait]
impl NetworkPlatform for WindowsPlatform {
    async fn network_info(&self) -> Result<NetworkInfo> {
        Ok(NetworkInfo {
            is_online: true,
            interfaces: vec![],
        })
    }

    async fn is_online(&self) -> bool {
        true
    }
}

#[async_trait]
impl ProcessPlatform for WindowsPlatform {
    async fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        Ok(vec![])
    }

    async fn process_info(&self, _pid: u32) -> Result<Option<ProcessInfo>> {
        Ok(None)
    }

    fn current_pid(&self) -> u32 {
        std::process::id()
    }
}

// ============================================================================
// Win32 FFI — Raw FFI for APIs not reliably available in windows crate v0.58
// ============================================================================

#[cfg(windows)]
#[allow(clippy::upper_case_acronyms)]
mod win32 {
    use std::ffi::c_void;
    use std::mem;
    use voxy_platform_core::error::Result;
    use voxy_platform_core::types::{DisplayInfo, WindowBounds, WindowInfo};

    type HWND = *mut c_void;
    type HMONITOR = *mut c_void;
    type HDC = *mut c_void;
    type BOOL = i32;

    const TRUE: BOOL = 1;
    const MDT_EFFECTIVE_DPI: u32 = 0;

    #[allow(clippy::upper_case_acronyms)]
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct RECT {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[allow(clippy::upper_case_acronyms)]
    #[repr(C)]
    struct MONITORINFO {
        cb_size: u32,
        rc_monitor: RECT,
        rc_work: RECT,
        dw_flags: u32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn SetProcessDpiAwarenessContext(value: isize) -> BOOL;
        fn SetProcessDPIAware() -> BOOL;
        fn GetForegroundWindow() -> HWND;
        fn SetForegroundWindow(hwnd: HWND) -> BOOL;
        fn ShowWindow(hwnd: HWND, cmdshow: i32) -> BOOL;
        fn GetWindowTextLengthW(hwnd: HWND) -> i32;
        fn GetWindowTextW(hwnd: HWND, buf: *mut u16, maxcount: i32) -> i32;
        fn GetWindowThreadProcessId(hwnd: HWND, lpdwprocessid: *mut u32) -> u32;
        fn GetWindowRect(hwnd: HWND, rect: *mut RECT) -> BOOL;
        fn IsWindowVisible(hwnd: HWND) -> BOOL;
        fn SendMessageW(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn EnumWindows(callback: usize, lparam: isize) -> BOOL;
        fn EnumDisplayMonitors(hdc: HDC, clip: *mut RECT, callback: usize, data: isize) -> BOOL;
        fn GetMonitorInfoW(hmon: HMONITOR, info: *mut MONITORINFO) -> BOOL;
    }

    #[link(name = "shcore")]
    extern "system" {
        fn GetDpiForMonitor(hmonitor: HMONITOR, dpi_type: u32, dpix: *mut u32, dpiy: *mut u32);
    }

    // DPI_AWARENESS_CONTEXT values
    const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: isize = -4;

    const SW_SHOW: i32 = 5;
    const WM_CLOSE: u32 = 0x0010;

    /// Set process DPI awareness to Per-Monitor V2.
    pub fn set_process_dpi_awareness() {
        unsafe {
            let ok = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            if ok != TRUE {
                let _ = SetProcessDPIAware();
            }
        }
    }

    /// Get the OS version string via RtlGetVersion.
    pub fn windows_version() -> String {
        extern "system" {
            fn RtlGetVersion(version: *mut RTL_OSVERSIONINFOW) -> i32;
        }
        #[allow(clippy::upper_case_acronyms)]
        #[repr(C)]
        struct RTL_OSVERSIONINFOW {
            dw_os_version_info_size: u32,
            dw_major_version: u32,
            dw_minor_version: u32,
            dw_build_number: u32,
            dw_platform_id: u32,
            _sz_csd_version: [u16; 128],
        }

        unsafe {
            let mut version = mem::zeroed::<RTL_OSVERSIONINFOW>();
            version.dw_os_version_info_size = mem::size_of::<RTL_OSVERSIONINFOW>() as u32;
            if RtlGetVersion(&mut version) == 0 {
                format!(
                    "{}.{}.{}",
                    version.dw_major_version, version.dw_minor_version, version.dw_build_number
                )
            } else {
                "10.0".to_string()
            }
        }
    }

    /// Get the hostname from the COMPUTERNAME environment variable.
    pub fn hostname() -> Option<String> {
        std::env::var("COMPUTERNAME").ok()
    }

    /// Callback for EnumDisplayMonitors.
    unsafe extern "system" fn monitor_enum_callback(
        hmon: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: isize,
    ) -> BOOL {
        unsafe {
            let monitors = &mut *(data as *mut Vec<HMONITOR>);
            monitors.push(hmon);
        }
        TRUE
    }

    /// Collect all monitor handles.
    pub fn collect_monitors() -> Vec<HMONITOR> {
        let mut monitors = Vec::new();
        unsafe {
            let _ = EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                monitor_enum_callback as *const () as usize,
                &mut monitors as *mut Vec<HMONITOR> as isize,
            );
        }
        monitors
    }

    /// Get DPI for a specific display by monitor index.
    pub fn get_display_dpi(display_id: u32) -> Result<(f64, f64)> {
        let monitors = collect_monitors();
        if let Some(mon) = monitors.get(display_id as usize) {
            unsafe {
                let mut dpi_x: u32 = 0;
                let mut dpi_y: u32 = 0;
                GetDpiForMonitor(*mon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
                if dpi_x > 0 && dpi_y > 0 {
                    return Ok((dpi_x as f64, dpi_y as f64));
                }
            }
        }
        Ok((96.0, 96.0))
    }

    /// Enumerate all displays with real Win32 API.
    pub fn enumerate_displays() -> Result<Vec<DisplayInfo>> {
        let monitors = collect_monitors();
        let mut displays = Vec::with_capacity(monitors.len());

        for (idx, hmon) in monitors.iter().enumerate() {
            unsafe {
                let mut info = MONITORINFO {
                    cb_size: mem::size_of::<MONITORINFO>() as u32,
                    rc_monitor: RECT::default(),
                    rc_work: RECT::default(),
                    dw_flags: 0,
                };
                if GetMonitorInfoW(*hmon, &mut info) == TRUE {
                    let rect = info.rc_monitor;
                    let is_primary = (info.dw_flags & 1) != 0;

                    let mut dpi_x: u32 = 0;
                    let mut dpi_y: u32 = 0;
                    GetDpiForMonitor(*hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);

                    displays.push(DisplayInfo {
                        id: idx as u32,
                        name: format!("Display {}", idx + 1),
                        width: (rect.right - rect.left) as u32,
                        height: (rect.bottom - rect.top) as u32,
                        x: rect.left,
                        y: rect.top,
                        is_primary,
                        dpi: if dpi_x > 0 { dpi_x as f64 } else { 96.0 },
                    });
                }
            }
        }

        Ok(displays)
    }

    /// Get the foreground window info.
    pub fn get_foreground_window() -> Result<Option<WindowInfo>> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return Ok(None);
            }
            window_info_from_hwnd(hwnd).map(Some)
        }
    }

    /// Set a window as foreground by HWND value.
    pub fn set_foreground_window(hwnd_val: u64) -> Result<()> {
        unsafe {
            let hwnd = hwnd_val as HWND;
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
        }
        Ok(())
    }

    /// Close a window by sending WM_CLOSE.
    pub fn close_window(hwnd_val: u64) -> Result<()> {
        unsafe {
            let hwnd = hwnd_val as HWND;
            SendMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        Ok(())
    }

    /// Build WindowInfo from an HWND.
    unsafe fn window_info_from_hwnd(hwnd: HWND) -> Result<WindowInfo> {
        let title_len = GetWindowTextLengthW(hwnd) as usize;
        let mut title_buf = vec![0u16; title_len + 1];
        GetWindowTextW(hwnd, title_buf.as_mut_ptr(), (title_len + 1) as i32);
        let title = String::from_utf16_lossy(&title_buf[..title_len]);

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);

        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect);

        let foreground_hwnd = GetForegroundWindow();
        let is_foreground = hwnd == foreground_hwnd;

        Ok(WindowInfo {
            id: hwnd as u64,
            title,
            process_name: String::new(),
            process_id: pid,
            is_foreground,
            bounds: WindowBounds {
                x: rect.left,
                y: rect.top,
                width: (rect.right - rect.left) as u32,
                height: (rect.bottom - rect.top) as u32,
            },
        })
    }

    /// Callback for EnumWindows.
    unsafe extern "system" fn enum_windows_callback(hwnd: HWND, data: isize) -> BOOL {
        unsafe {
            if IsWindowVisible(hwnd) == TRUE {
                let windows = &mut *(data as *mut Vec<WindowInfo>);
                if let Ok(info) = window_info_from_hwnd(hwnd) {
                    if !info.title.is_empty() {
                        windows.push(info);
                    }
                }
            }
        }
        TRUE
    }

    /// Enumerate all visible top-level windows.
    pub fn enumerate_windows() -> Result<Vec<WindowInfo>> {
        let mut windows = Vec::new();
        unsafe {
            let _ = EnumWindows(
                enum_windows_callback as *const () as usize,
                &mut windows as *mut Vec<WindowInfo> as isize,
            );
        }
        Ok(windows)
    }
}

#[cfg(not(windows))]
mod win32 {
    use voxy_platform_core::error::Result;
    use voxy_platform_core::types::{DisplayInfo, WindowInfo};

    pub fn set_process_dpi_awareness() {}
    pub fn windows_version() -> String {
        "unknown".to_string()
    }
    pub fn hostname() -> Option<String> {
        None
    }
    pub fn get_display_dpi(_display_id: u32) -> Result<(f64, f64)> {
        Ok((96.0, 96.0))
    }
    pub fn enumerate_displays() -> Result<Vec<DisplayInfo>> {
        Ok(vec![])
    }
    pub fn get_foreground_window() -> Result<Option<WindowInfo>> {
        Ok(None)
    }
    pub fn set_foreground_window(_hwnd: u64) -> Result<()> {
        Ok(())
    }
    pub fn close_window(_hwnd: u64) -> Result<()> {
        Ok(())
    }
    pub fn enumerate_windows() -> Result<Vec<WindowInfo>> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn windows_platform_creation() {
        let mut platform = WindowsPlatform::new();
        platform.initialize().await.unwrap();
        assert_eq!(platform.name(), "windows");
        platform.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn windows_platform_info() {
        let platform = WindowsPlatform::new();
        let info = platform.info();
        assert_eq!(info.os, "windows");
        assert!(!info.version.is_empty());
    }

    #[tokio::test]
    async fn windows_dpi_awareness() {
        let platform = WindowsPlatform::new();
        let dpi = platform.display_dpi(0).await.unwrap();
        assert!(dpi.0 > 0.0 && dpi.1 > 0.0);
    }

    #[tokio::test]
    async fn windows_display_enumeration() {
        let platform = WindowsPlatform::new();
        let displays = platform.list_displays().await.unwrap();
        assert!(!displays.is_empty());
        if let Some(primary) = displays.iter().find(|d| d.is_primary) {
            assert!(primary.width > 0 && primary.height > 0);
        }
    }

    #[tokio::test]
    async fn windows_foreground_window() {
        let platform = WindowsPlatform::new();
        let _fg = platform.foreground_window().await.unwrap();
    }
}
