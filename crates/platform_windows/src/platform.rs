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

    async fn minimize_window(&self, id: u64) -> Result<()> {
        win32::minimize_window(id)
    }

    async fn maximize_window(&self, id: u64) -> Result<()> {
        win32::maximize_window(id)
    }

    async fn restore_window(&self, id: u64) -> Result<()> {
        win32::restore_window(id)
    }

    async fn resize_window(&self, id: u64, width: u32, height: u32) -> Result<()> {
        win32::resize_window(id, width, height)
    }

    async fn move_window(&self, id: u64, x: i32, y: i32) -> Result<()> {
        win32::move_window(id, x, y)
    }
}

#[async_trait]
impl InputPlatform for WindowsPlatform {
    async fn mouse_click(&self, x: i32, y: i32) -> Result<()> {
        win32::mouse_click(x, y)
    }

    async fn keyboard_type(&self, text: &str) -> Result<()> {
        win32::keyboard_type(text)
    }

    async fn key_press(&self, key: &str) -> Result<()> {
        win32::key_press(key)
    }

    async fn mouse_position(&self) -> Result<(i32, i32)> {
        win32::mouse_position()
    }
}

#[async_trait]
impl DisplayPlatform for WindowsPlatform {
    async fn list_displays(&self) -> Result<Vec<DisplayInfo>> {
        win32::enumerate_displays()
    }

    async fn screenshot(&self, display_id: u32) -> Result<Vec<u8>> {
        win32::capture_screenshot(display_id)
    }

    async fn display_dpi(&self, display_id: u32) -> Result<(f64, f64)> {
        win32::get_display_dpi(display_id)
    }
}

#[async_trait]
impl AudioPlatform for WindowsPlatform {
    async fn input_devices(&self) -> Result<Vec<AudioDevice>> {
        win32::enumerate_audio_input_devices()
    }

    async fn output_devices(&self) -> Result<Vec<AudioDevice>> {
        win32::enumerate_audio_output_devices()
    }

    async fn default_input_device(&self) -> Result<Option<AudioDevice>> {
        let devs = self.input_devices().await?;
        Ok(devs.into_iter().next())
    }

    async fn default_output_device(&self) -> Result<Option<AudioDevice>> {
        let devs = self.output_devices().await?;
        Ok(devs.into_iter().next())
    }
}

#[async_trait]
impl FileSystemPlatform for WindowsPlatform {
    async fn file_info(&self, path: &str) -> Result<FileInfo> {
        let p = std::path::Path::new(path);
        let meta = tokio::fs::metadata(p)
            .await
            .map_err(|e| PlatformError::QueryFailed(e.to_string()))?;
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let modified = meta.modified().ok().map(|t| {
            let dt: chrono::DateTime<chrono::Utc> = t.into();
            dt.to_rfc3339()
        });
        Ok(FileInfo {
            path: path.to_string(),
            name,
            is_dir: meta.is_dir(),
            size: meta.len(),
            modified,
        })
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<FileInfo>> {
        let p = std::path::Path::new(path);
        let mut entries = tokio::fs::read_dir(p)
            .await
            .map_err(|e| PlatformError::QueryFailed(e.to_string()))?;
        let mut result = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let entry_path = entry.path();
            let entry_path_str = entry_path.to_string_lossy().to_string();
            if let Ok(meta) = entry.metadata().await {
                let name = entry.file_name().to_string_lossy().to_string();
                let modified = meta.modified().ok().map(|t| {
                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                    dt.to_rfc3339()
                });
                result.push(FileInfo {
                    path: entry_path_str,
                    name,
                    is_dir: meta.is_dir(),
                    size: meta.len(),
                    modified,
                });
            }
        }
        Ok(result)
    }

    async fn path_exists(&self, path: &str) -> bool {
        tokio::fs::try_exists(path).await.unwrap_or(false)
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
        win32::get_network_info()
    }

    async fn is_online(&self) -> bool {
        win32::is_network_online()
    }
}

#[async_trait]
impl ProcessPlatform for WindowsPlatform {
    async fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        win32::list_processes()
    }

    async fn process_info(&self, pid: u32) -> Result<Option<ProcessInfo>> {
        win32::get_process_info(pid)
    }

    fn current_pid(&self) -> u32 {
        std::process::id()
    }
}

// ============================================================================
// Win32 FFI — Comprehensive native Windows APIs
// ============================================================================

#[cfg(windows)]
#[allow(clippy::upper_case_acronyms)]
mod win32 {
    use std::ffi::c_void;
    use std::mem;
    use voxy_platform_core::error::{PlatformError, Result};
    use voxy_platform_core::types::{
        AudioDevice, DisplayInfo, NetworkInfo, NetworkInterface, ProcessInfo, WindowBounds,
        WindowInfo,
    };

    type HWND = *mut c_void;
    type HMONITOR = *mut c_void;
    type HDC = *mut c_void;
    type HBITMAP = *mut c_void;
    type HGDIOBJ = *mut c_void;
    type HANDLE = *mut c_void;
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
    #[derive(Clone, Copy, Default)]
    struct POINT {
        x: i32,
        y: i32,
    }

    #[allow(clippy::upper_case_acronyms)]
    #[repr(C)]
    struct MONITORINFO {
        cb_size: u32,
        rc_monitor: RECT,
        rc_work: RECT,
        dw_flags: u32,
    }

    #[allow(non_snake_case)]
    #[repr(C)]
    struct BITMAPINFOHEADER {
        biSize: u32,
        biWidth: i32,
        biHeight: i32,
        biPlanes: u16,
        biBitCount: u16,
        biCompression: u32,
        biSizeImage: u32,
        biXPelsPerMeter: i32,
        biYPelsPerMeter: i32,
        biClrUsed: u32,
        biClrImportant: u32,
    }

    #[allow(non_snake_case)]
    #[repr(C)]
    struct RGBQUAD {
        rgbBlue: u8,
        rgbGreen: u8,
        rgbRed: u8,
        rgbReserved: u8,
    }

    #[allow(non_snake_case)]
    #[repr(C)]
    struct BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER,
        bmiColors: [RGBQUAD; 1],
    }

    #[repr(C)]
    struct INPUT {
        type_: u32,
        u: INPUT_UNION,
    }

    #[repr(C)]
    union INPUT_UNION {
        ki: KEYBDINPUT,
        mi: MOUSEINPUT,
        _align: [u64; 4],
    }

    #[allow(non_snake_case)]
    #[derive(Copy, Clone)]
    #[repr(C)]
    struct KEYBDINPUT {
        wVk: u16,
        wScan: u16,
        dwFlags: u32,
        time: u32,
        dwExtraInfo: usize,
    }

    #[allow(non_snake_case)]
    #[derive(Copy, Clone)]
    #[repr(C)]
    struct MOUSEINPUT {
        dx: i32,
        dy: i32,
        mouseData: u32,
        dwFlags: u32,
        time: u32,
        dwExtraInfo: usize,
    }

    #[allow(non_snake_case)]
    #[repr(C)]
    struct PROCESSENTRY32W {
        dwSize: u32,
        cntUsage: u32,
        th32ProcessID: u32,
        th32DefaultHeapID: usize,
        th32ModuleID: u32,
        cntThreads: u32,
        th32ParentProcessID: u32,
        pcPriClassBase: i32,
        dwFlags: u32,
        szExeFile: [u16; 260],
    }

    #[allow(non_snake_case)]
    #[repr(C)]
    struct PROCESS_MEMORY_COUNTERS {
        cb: u32,
        PageFaultCount: u32,
        PeakWorkingSetSize: usize,
        WorkingSetSize: usize,
        QuotaPeakPagedPoolUsage: usize,
        QuotaPagedPoolUsage: usize,
        QuotaPeakNonPagedPoolUsage: usize,
        QuotaNonPagedPoolUsage: usize,
        PagefileUsage: usize,
        PeakPagefileUsage: usize,
    }

    #[allow(non_snake_case)]
    #[repr(C)]
    struct WAVEINCAPSW {
        wMid: u16,
        wPid: u16,
        vDriverVersion: u32,
        szPname: [u16; 32],
        dwFormats: u32,
        wChannels: u16,
        wReserved1: u16,
    }

    #[allow(non_snake_case)]
    #[repr(C)]
    struct WAVEOUTCAPSW {
        wMid: u16,
        wPid: u16,
        vDriverVersion: u32,
        szPname: [u16; 32],
        dwFormats: u32,
        wChannels: u16,
        wReserved1: u16,
        dwSupport: u32,
    }

    const INPUT_MOUSE: u32 = 0;
    const INPUT_KEYBOARD: u32 = 1;
    const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
    const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
    const KEYEVENTF_KEYUP: u32 = 0x0002;
    const KEYEVENTF_UNICODE: u32 = 0x0004;

    const SW_SHOW: i32 = 5;
    const SW_SHOWMINIMIZED: i32 = 2;
    const SW_SHOWMAXIMIZED: i32 = 3;
    const SW_RESTORE: i32 = 9;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const WM_CLOSE: u32 = 0x0010;

    const SRCCOPY: u32 = 0x00CC0020;
    const SM_CXSCREEN: i32 = 0;
    const SM_CYSCREEN: i32 = 1;
    const DIB_RGB_COLORS: u32 = 0;

    #[link(name = "user32")]
    extern "system" {
        fn SetProcessDpiAwarenessContext(value: isize) -> BOOL;
        fn SetProcessDPIAware() -> BOOL;
        fn GetForegroundWindow() -> HWND;
        fn SetForegroundWindow(hwnd: HWND) -> BOOL;
        fn ShowWindow(hwnd: HWND, cmdshow: i32) -> BOOL;
        fn SetWindowPos(
            hwnd: HWND,
            hwnd_insert_after: HWND,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            uflags: u32,
        ) -> BOOL;
        fn GetWindowTextLengthW(hwnd: HWND) -> i32;
        fn GetWindowTextW(hwnd: HWND, buf: *mut u16, maxcount: i32) -> i32;
        fn GetWindowThreadProcessId(hwnd: HWND, lpdwprocessid: *mut u32) -> u32;
        fn GetWindowRect(hwnd: HWND, rect: *mut RECT) -> BOOL;
        fn IsWindowVisible(hwnd: HWND) -> BOOL;
        fn SendMessageW(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn EnumWindows(callback: usize, lparam: isize) -> BOOL;
        fn EnumDisplayMonitors(hdc: HDC, clip: *mut RECT, callback: usize, data: isize) -> BOOL;
        fn GetMonitorInfoW(hmon: HMONITOR, info: *mut MONITORINFO) -> BOOL;
        fn GetDC(hwnd: HWND) -> HDC;
        fn ReleaseDC(hwnd: HWND, hdc: HDC) -> i32;
        fn GetSystemMetrics(nindex: i32) -> i32;
        fn SetCursorPos(x: i32, y: i32) -> BOOL;
        fn GetCursorPos(point: *mut POINT) -> BOOL;
        fn SendInput(cinputs: u32, pinputs: *mut INPUT, cbsize: i32) -> u32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateCompatibleDC(hdc: HDC) -> HDC;
        fn CreateCompatibleBitmap(hdc: HDC, cx: i32, cy: i32) -> HBITMAP;
        fn SelectObject(hdc: HDC, hgdiobj: HGDIOBJ) -> HGDIOBJ;
        fn BitBlt(
            hdcdest: HDC,
            nxdest: i32,
            nydest: i32,
            nwidth: i32,
            nheight: i32,
            hdcsrc: HDC,
            nxsrc: i32,
            nysrc: i32,
            dwrop: u32,
        ) -> BOOL;
        fn GetDIBits(
            hdc: HDC,
            hbm: HBITMAP,
            start: u32,
            clines: u32,
            lpvbits: *mut c_void,
            lpbmi: *mut BITMAPINFO,
            usage: u32,
        ) -> i32;
        fn DeleteDC(hdc: HDC) -> BOOL;
        fn DeleteObject(ho: HGDIOBJ) -> BOOL;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateToolhelp32Snapshot(dwflags: u32, th32processid: u32) -> HANDLE;
        fn Process32FirstW(hsnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
        fn Process32NextW(hsnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
        fn CloseHandle(hobject: HANDLE) -> BOOL;
        fn OpenProcess(dwdesiredaccess: u32, binherithandle: BOOL, dwprocessid: u32) -> HANDLE;
        fn QueryFullProcessImageNameW(
            hprocess: HANDLE,
            dwflags: u32,
            lpexename: *mut u16,
            lpdwsize: *mut u32,
        ) -> BOOL;
    }

    #[link(name = "psapi")]
    extern "system" {
        fn GetProcessMemoryInfo(
            hprocess: HANDLE,
            ppsms: *mut PROCESS_MEMORY_COUNTERS,
            cb: u32,
        ) -> BOOL;
    }

    #[link(name = "shcore")]
    extern "system" {
        fn GetDpiForMonitor(hmonitor: HMONITOR, dpi_type: u32, dpix: *mut u32, dpiy: *mut u32);
    }

    #[link(name = "winmm")]
    extern "system" {
        fn waveInGetNumDevs() -> u32;
        fn waveInGetDevCapsW(udeviceid: usize, pwic: *mut WAVEINCAPSW, cbwic: u32) -> u32;
        fn waveOutGetNumDevs() -> u32;
        fn waveOutGetDevCapsW(udeviceid: usize, pwoc: *mut WAVEOUTCAPSW, cbwoc: u32) -> u32;
    }

    #[link(name = "wininet")]
    extern "system" {
        fn InternetGetConnectedState(lpdwflags: *mut u32, dwreserved: u32) -> BOOL;
    }

    const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: isize = -4;

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

    /// Get the hostname from COMPUTERNAME environment variable.
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

    /// Capture screenshot of a monitor into standard BMP byte stream.
    pub fn capture_screenshot(display_id: u32) -> Result<Vec<u8>> {
        let monitors = collect_monitors();
        let hmon = monitors.get(display_id as usize).copied();

        let (x, y, w, h) = if let Some(hmon) = hmon {
            unsafe {
                let mut info = MONITORINFO {
                    cb_size: mem::size_of::<MONITORINFO>() as u32,
                    rc_monitor: RECT::default(),
                    rc_work: RECT::default(),
                    dw_flags: 0,
                };
                if GetMonitorInfoW(hmon, &mut info) == TRUE {
                    let r = info.rc_monitor;
                    (
                        r.left,
                        r.top,
                        (r.right - r.left).max(1) as u32,
                        (r.bottom - r.top).max(1) as u32,
                    )
                } else {
                    (0, 0, 1920, 1080)
                }
            }
        } else {
            unsafe {
                let w = GetSystemMetrics(SM_CXSCREEN).max(1) as u32;
                let h = GetSystemMetrics(SM_CYSCREEN).max(1) as u32;
                (0, 0, w, h)
            }
        };

        unsafe {
            let hdc_screen = GetDC(std::ptr::null_mut());
            if hdc_screen.is_null() {
                return Err(PlatformError::QueryFailed("Failed to get screen DC".into()));
            }
            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let hbm = CreateCompatibleBitmap(hdc_screen, w as i32, h as i32);
            let old_bm = SelectObject(hdc_mem, hbm as HGDIOBJ);

            BitBlt(hdc_mem, 0, 0, w as i32, h as i32, hdc_screen, x, y, SRCCOPY);

            let row_stride = w * 4;
            let image_size = (row_stride * h) as usize;
            let mut pixels = vec![0u8; image_size];

            let mut bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w as i32,
                    biHeight: -(h as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: 0,
                    biSizeImage: image_size as u32,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD {
                    rgbBlue: 0,
                    rgbGreen: 0,
                    rgbRed: 0,
                    rgbReserved: 0,
                }],
            };

            GetDIBits(
                hdc_mem,
                hbm,
                0,
                h,
                pixels.as_mut_ptr() as *mut c_void,
                &mut bi,
                DIB_RGB_COLORS,
            );

            SelectObject(hdc_mem, old_bm);
            DeleteObject(hbm as HGDIOBJ);
            DeleteDC(hdc_mem);
            ReleaseDC(std::ptr::null_mut(), hdc_screen);

            let file_size = 14 + 40 + image_size;
            let mut bmp = Vec::with_capacity(file_size);
            bmp.extend_from_slice(&0x4D42u16.to_le_bytes());
            bmp.extend_from_slice(&(file_size as u32).to_le_bytes());
            bmp.extend_from_slice(&0u32.to_le_bytes());
            bmp.extend_from_slice(&54u32.to_le_bytes());
            bmp.extend_from_slice(&40u32.to_le_bytes());
            bmp.extend_from_slice(&(w as i32).to_le_bytes());
            bmp.extend_from_slice(&(-(h as i32)).to_le_bytes());
            bmp.extend_from_slice(&1u16.to_le_bytes());
            bmp.extend_from_slice(&32u16.to_le_bytes());
            bmp.extend_from_slice(&0u32.to_le_bytes());
            bmp.extend_from_slice(&(image_size as u32).to_le_bytes());
            bmp.extend_from_slice(&0u32.to_le_bytes());
            bmp.extend_from_slice(&0u32.to_le_bytes());
            bmp.extend_from_slice(&0u32.to_le_bytes());
            bmp.extend_from_slice(&0u32.to_le_bytes());
            bmp.extend_from_slice(&pixels);

            Ok(bmp)
        }
    }

    /// Mouse click simulation.
    pub fn mouse_click(x: i32, y: i32) -> Result<()> {
        unsafe {
            SetCursorPos(x, y);
            let mut inputs = [
                INPUT {
                    type_: INPUT_MOUSE,
                    u: INPUT_UNION {
                        mi: MOUSEINPUT {
                            dx: 0,
                            dy: 0,
                            mouseData: 0,
                            dwFlags: MOUSEEVENTF_LEFTDOWN,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
                INPUT {
                    type_: INPUT_MOUSE,
                    u: INPUT_UNION {
                        mi: MOUSEINPUT {
                            dx: 0,
                            dy: 0,
                            mouseData: 0,
                            dwFlags: MOUSEEVENTF_LEFTUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
            ];
            SendInput(2, inputs.as_mut_ptr(), mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }

    /// Mouse position inquiry.
    pub fn mouse_position() -> Result<(i32, i32)> {
        unsafe {
            let mut pt = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut pt) != 0 {
                Ok((pt.x, pt.y))
            } else {
                Err(PlatformError::QueryFailed(
                    "Failed to get cursor position".into(),
                ))
            }
        }
    }

    /// Keyboard type simulation using Unicode.
    pub fn keyboard_type(text: &str) -> Result<()> {
        for c in text.encode_utf16() {
            unsafe {
                let mut inputs = [
                    INPUT {
                        type_: INPUT_KEYBOARD,
                        u: INPUT_UNION {
                            ki: KEYBDINPUT {
                                wVk: 0,
                                wScan: c,
                                dwFlags: KEYEVENTF_UNICODE,
                                time: 0,
                                dwExtraInfo: 0,
                            },
                        },
                    },
                    INPUT {
                        type_: INPUT_KEYBOARD,
                        u: INPUT_UNION {
                            ki: KEYBDINPUT {
                                wVk: 0,
                                wScan: c,
                                dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                                time: 0,
                                dwExtraInfo: 0,
                            },
                        },
                    },
                ];
                SendInput(2, inputs.as_mut_ptr(), mem::size_of::<INPUT>() as i32);
            }
        }
        Ok(())
    }

    /// Keyboard key press simulation.
    pub fn key_press(key: &str) -> Result<()> {
        let vk: u16 = match key.to_lowercase().as_str() {
            "return" | "enter" => 0x0D,
            "tab" => 0x09,
            "escape" | "esc" => 0x1B,
            "backspace" => 0x08,
            "space" => 0x20,
            "left" => 0x25,
            "up" => 0x26,
            "right" => 0x27,
            "down" => 0x28,
            "delete" => 0x2E,
            "shift" => 0x10,
            "control" | "ctrl" => 0x11,
            "alt" => 0x12,
            _ => 0,
        };
        if vk != 0 {
            unsafe {
                let mut inputs = [
                    INPUT {
                        type_: INPUT_KEYBOARD,
                        u: INPUT_UNION {
                            ki: KEYBDINPUT {
                                wVk: vk,
                                wScan: 0,
                                dwFlags: 0,
                                time: 0,
                                dwExtraInfo: 0,
                            },
                        },
                    },
                    INPUT {
                        type_: INPUT_KEYBOARD,
                        u: INPUT_UNION {
                            ki: KEYBDINPUT {
                                wVk: vk,
                                wScan: 0,
                                dwFlags: KEYEVENTF_KEYUP,
                                time: 0,
                                dwExtraInfo: 0,
                            },
                        },
                    },
                ];
                SendInput(2, inputs.as_mut_ptr(), mem::size_of::<INPUT>() as i32);
            }
        }
        Ok(())
    }

    /// Audio input devices via waveIn.
    pub fn enumerate_audio_input_devices() -> Result<Vec<AudioDevice>> {
        let count = unsafe { waveInGetNumDevs() };
        let mut devices = Vec::with_capacity(count as usize);
        for idx in 0..count {
            unsafe {
                let mut caps: WAVEINCAPSW = mem::zeroed();
                if waveInGetDevCapsW(
                    idx as usize,
                    &mut caps,
                    mem::size_of::<WAVEINCAPSW>() as u32,
                ) == 0
                {
                    let name = String::from_utf16_lossy(&caps.szPname)
                        .trim_matches('\0')
                        .to_string();
                    devices.push(AudioDevice {
                        id: format!("input:{}", idx),
                        name,
                        is_input: true,
                        is_default: idx == 0,
                        sample_rate: 44100,
                        channels: caps.wChannels,
                    });
                }
            }
        }
        Ok(devices)
    }

    /// Audio output devices via waveOut.
    pub fn enumerate_audio_output_devices() -> Result<Vec<AudioDevice>> {
        let count = unsafe { waveOutGetNumDevs() };
        let mut devices = Vec::with_capacity(count as usize);
        for idx in 0..count {
            unsafe {
                let mut caps: WAVEOUTCAPSW = mem::zeroed();
                if waveOutGetDevCapsW(
                    idx as usize,
                    &mut caps,
                    mem::size_of::<WAVEOUTCAPSW>() as u32,
                ) == 0
                {
                    let name = String::from_utf16_lossy(&caps.szPname)
                        .trim_matches('\0')
                        .to_string();
                    devices.push(AudioDevice {
                        id: format!("output:{}", idx),
                        name,
                        is_input: false,
                        is_default: idx == 0,
                        sample_rate: 44100,
                        channels: caps.wChannels,
                    });
                }
            }
        }
        Ok(devices)
    }

    /// Process list using Toolhelp32 snapshot.
    pub fn list_processes() -> Result<Vec<ProcessInfo>> {
        let mut list = Vec::new();
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(0x00000002, 0);
            if snapshot.is_null() || snapshot == -1isize as *mut c_void {
                return Err(PlatformError::QueryFailed(
                    "Failed to create process snapshot".into(),
                ));
            }

            let mut entry: PROCESSENTRY32W = mem::zeroed();
            entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snapshot, &mut entry) == TRUE {
                loop {
                    let name = String::from_utf16_lossy(&entry.szExeFile)
                        .trim_matches('\0')
                        .to_string();
                    list.push(ProcessInfo {
                        pid: entry.th32ProcessID,
                        name,
                        cpu_usage: 0.0,
                        memory_usage: 0,
                        executable_path: None,
                    });
                    if Process32NextW(snapshot, &mut entry) != TRUE {
                        break;
                    }
                }
            }
            CloseHandle(snapshot);
        }
        Ok(list)
    }

    /// Process info querying memory and path.
    pub fn get_process_info(pid: u32) -> Result<Option<ProcessInfo>> {
        unsafe {
            let hproc = OpenProcess(0x1000, 0, pid);
            if hproc.is_null() {
                return Ok(None);
            }

            let mut path_buf = [0u16; 1024];
            let mut path_len = path_buf.len() as u32;
            let mut exe_path = None;
            if QueryFullProcessImageNameW(hproc, 0, path_buf.as_mut_ptr(), &mut path_len) == TRUE {
                exe_path = Some(String::from_utf16_lossy(&path_buf[..path_len as usize]));
            }

            let mut pmc: PROCESS_MEMORY_COUNTERS = mem::zeroed();
            pmc.cb = mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            let mem_usage = if GetProcessMemoryInfo(hproc, &mut pmc, pmc.cb) == TRUE {
                pmc.WorkingSetSize as u64
            } else {
                0
            };

            CloseHandle(hproc);

            let name = exe_path
                .as_ref()
                .and_then(|p| {
                    std::path::Path::new(p)
                        .file_name()?
                        .to_str()
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| format!("pid:{}", pid));

            Ok(Some(ProcessInfo {
                pid,
                name,
                cpu_usage: 0.0,
                memory_usage: mem_usage,
                executable_path: exe_path,
            }))
        }
    }

    /// Check if network is connected.
    pub fn is_network_online() -> bool {
        unsafe {
            let mut flags = 0u32;
            InternetGetConnectedState(&mut flags, 0) == TRUE
        }
    }

    /// Get basic network configuration.
    pub fn get_network_info() -> Result<NetworkInfo> {
        let online = is_network_online();
        let host = hostname().unwrap_or_else(|| "localhost".to_string());
        Ok(NetworkInfo {
            is_online: online,
            interfaces: vec![NetworkInterface {
                name: host,
                ip_addresses: vec!["127.0.0.1".to_string()],
                is_up: online,
            }],
        })
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

    /// Minimize a window by HWND.
    pub fn minimize_window(hwnd_val: u64) -> Result<()> {
        unsafe {
            let hwnd = hwnd_val as HWND;
            ShowWindow(hwnd, SW_SHOWMINIMIZED);
        }
        Ok(())
    }

    /// Maximize a window by HWND.
    pub fn maximize_window(hwnd_val: u64) -> Result<()> {
        unsafe {
            let hwnd = hwnd_val as HWND;
            ShowWindow(hwnd, SW_SHOWMAXIMIZED);
        }
        Ok(())
    }

    /// Restore a window by HWND.
    pub fn restore_window(hwnd_val: u64) -> Result<()> {
        unsafe {
            let hwnd = hwnd_val as HWND;
            ShowWindow(hwnd, SW_RESTORE);
        }
        Ok(())
    }

    /// Resize a window by HWND.
    pub fn resize_window(hwnd_val: u64, width: u32, height: u32) -> Result<()> {
        unsafe {
            let hwnd = hwnd_val as HWND;
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                width as i32,
                height as i32,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        Ok(())
    }

    /// Move a window by HWND.
    pub fn move_window(hwnd_val: u64, x: i32, y: i32) -> Result<()> {
        unsafe {
            let hwnd = hwnd_val as HWND;
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
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

        let mut process_name = String::new();
        if pid > 0 {
            let hproc = OpenProcess(0x1000, 0, pid);
            if !hproc.is_null() {
                let mut path_buf = [0u16; 512];
                let mut path_len = path_buf.len() as u32;
                if QueryFullProcessImageNameW(hproc, 0, path_buf.as_mut_ptr(), &mut path_len)
                    == TRUE
                {
                    let p = String::from_utf16_lossy(&path_buf[..path_len as usize]);
                    if let Some(name) = std::path::Path::new(&p).file_name() {
                        process_name = name.to_string_lossy().to_string();
                    }
                }
                CloseHandle(hproc);
            }
        }

        Ok(WindowInfo {
            id: hwnd as u64,
            title,
            process_name,
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
    use voxy_platform_core::types::{
        AudioDevice, DisplayInfo, NetworkInfo, ProcessInfo, WindowInfo,
    };

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
    pub fn capture_screenshot(_display_id: u32) -> Result<Vec<u8>> {
        Ok(vec![])
    }
    pub fn mouse_click(_x: i32, _y: i32) -> Result<()> {
        Ok(())
    }
    pub fn mouse_position() -> Result<(i32, i32)> {
        Ok((0, 0))
    }
    pub fn keyboard_type(_text: &str) -> Result<()> {
        Ok(())
    }
    pub fn key_press(_key: &str) -> Result<()> {
        Ok(())
    }
    pub fn enumerate_audio_input_devices() -> Result<Vec<AudioDevice>> {
        Ok(vec![])
    }
    pub fn enumerate_audio_output_devices() -> Result<Vec<AudioDevice>> {
        Ok(vec![])
    }
    pub fn list_processes() -> Result<Vec<ProcessInfo>> {
        Ok(vec![])
    }
    pub fn get_process_info(_pid: u32) -> Result<Option<ProcessInfo>> {
        Ok(None)
    }
    pub fn is_network_online() -> bool {
        true
    }
    pub fn get_network_info() -> Result<NetworkInfo> {
        Ok(NetworkInfo {
            is_online: true,
            interfaces: vec![],
        })
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
    pub fn minimize_window(_hwnd: u64) -> Result<()> {
        Ok(())
    }
    pub fn maximize_window(_hwnd: u64) -> Result<()> {
        Ok(())
    }
    pub fn restore_window(_hwnd: u64) -> Result<()> {
        Ok(())
    }
    pub fn resize_window(_hwnd: u64, _w: u32, _h: u32) -> Result<()> {
        Ok(())
    }
    pub fn move_window(_hwnd: u64, _x: i32, _y: i32) -> Result<()> {
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

    #[tokio::test]
    async fn windows_process_enumeration() {
        let platform = WindowsPlatform::new();
        let procs = platform.list_processes().await.unwrap();
        assert!(!procs.is_empty());
        let cur_pid = platform.current_pid();
        let cur_proc = platform.process_info(cur_pid).await.unwrap();
        assert!(cur_proc.is_some());
    }

    #[tokio::test]
    async fn windows_filesystem_operations() {
        let platform = WindowsPlatform::new();
        let home = platform.home_dir().await.unwrap();
        assert!(!home.is_empty());
        assert!(platform.path_exists(&home).await);
        let info = platform.file_info(&home).await.unwrap();
        assert!(info.is_dir);
    }

    #[tokio::test]
    async fn windows_screenshot() {
        let platform = WindowsPlatform::new();
        let bytes = platform.screenshot(0).await.unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[0..2], b"BM");
    }
}
