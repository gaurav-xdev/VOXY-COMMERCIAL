use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use tracing::info;

use crate::error::Result;
use voxy_orchestrator::automation::{
    AutomationBackend, AutomationCapability, ElementInfo, ElementSelector, MouseButton,
    StateVerification, WindowTarget,
};
use voxy_shared::types::Rect;

#[cfg(windows)]
mod platform {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;

    use super::*;

    pub type Hwnd = isize;

    pub struct WindowsUiaBackendInner {
        initialized: bool,
        dpi_scale: f64,
    }

    impl WindowsUiaBackendInner {
        pub fn new() -> Self {
            Self {
                initialized: false,
                dpi_scale: 1.0,
            }
        }

        pub fn initialize(&mut self) -> Result<()> {
            self.dpi_scale = get_dpi_scale();
            self.initialized = true;
            info!(
                "Windows UIA backend initialized (DPI scale: {})",
                self.dpi_scale
            );
            Ok(())
        }

        pub fn dpi_scale(&self) -> f64 {
            self.dpi_scale
        }
    }

    fn get_dpi_scale() -> f64 {
        1.0
    }

    fn to_wstring(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    #[link(name = "user32")]
    #[link(name = "gdi32")]
    extern "system" {
        fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> Hwnd;
        fn EnumWindows(
            lpEnumFunc: Option<unsafe extern "system" fn(Hwnd, isize) -> i32>,
            lParam: isize,
        ) -> i32;
        fn GetWindowTextW(hWnd: Hwnd, lpString: *mut u16, nMaxCount: i32) -> i32;
        fn GetWindowTextLengthW(hWnd: Hwnd) -> i32;
        fn GetClassNameW(hWnd: Hwnd, lpClassName: *mut u16, nMaxCount: i32) -> i32;
        fn IsWindowVisible(hWnd: Hwnd) -> i32;
        fn GetForegroundWindow() -> Hwnd;
        fn SetForegroundWindow(hWnd: Hwnd) -> i32;
        fn ShowWindow(hWnd: Hwnd, nCmdShow: i32) -> i32;
        fn MoveWindow(hWnd: Hwnd, X: i32, Y: i32, nWidth: i32, nHeight: i32, bRepaint: i32) -> i32;
        fn GetWindowRect(hWnd: Hwnd, lpRect: *mut RECT) -> i32;
        fn CloseWindow(hWnd: Hwnd) -> i32;
        fn GetCursorPos(lpPoint: *mut POINT) -> i32;
        fn SetCursorPos(X: i32, Y: i32) -> i32;
        fn SendInput(cInputs: u32, pInputs: *mut INPUT, cbSize: i32) -> u32;
        fn GetDC(hWnd: Hwnd) -> isize;
        fn ReleaseDC(hWnd: Hwnd, hDC: isize) -> i32;
        fn GetPixel(hDC: isize, X: i32, Y: i32) -> u32;
        fn CreateCompatibleDC(hDC: isize) -> isize;
        fn CreateCompatibleBitmap(hDC: isize, nWidth: i32, nHeight: i32) -> isize;
        fn SelectObject(hDC: isize, hObject: isize) -> isize;
        fn BitBlt(
            hdcDest: isize,
            xDest: i32,
            yDest: i32,
            wDest: i32,
            hDest: i32,
            hdcSrc: isize,
            xSrc: i32,
            ySrc: i32,
            dwRop: u32,
        ) -> i32;
        fn DeleteDC(hDC: isize) -> i32;
        fn DeleteObject(hObject: isize) -> i32;
        fn GetObjectW(hObject: isize, nCount: i32, lpObject: *mut BITMAP) -> i32;
        fn GetDIBits(
            hdc: isize,
            hbm: isize,
            start: u32,
            cLines: u32,
            lpvBits: *mut std::ffi::c_void,
            lpbmi: *mut BITMAPINFO,
            usage: u32,
        ) -> i32;
        pub fn GetSystemMetrics(nIndex: i32) -> i32;
        fn GetWindowThreadProcessId(hWnd: Hwnd, lpdwProcessId: *mut u32) -> u32;
    }

    #[repr(C)]
    struct RECT {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct POINT {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct BITMAP {
        bmType: i32,
        bmWidth: i32,
        bmHeight: i32,
        bmWidthBytes: i32,
        bmPlanes: u16,
        bmBitsPixel: u16,
        bmBits: *mut std::ffi::c_void,
    }

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

    #[repr(C)]
    struct RGBQUAD {
        rgbBlue: u8,
        rgbGreen: u8,
        rgbRed: u8,
        rgbReserved: u8,
    }

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
        hi: HARDWAREINPUT,
    }

    #[derive(Copy, Clone)]
    #[repr(C)]
    struct KEYBDINPUT {
        wVk: u16,
        wScan: u16,
        dwFlags: u32,
        time: u32,
        dwExtraInfo: usize,
    }

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

    #[derive(Copy, Clone)]
    #[repr(C)]
    struct HARDWAREINPUT {
        uMsg: u32,
        wParamL: u16,
        wParamH: u16,
    }

    const INPUT_MOUSE: u32 = 0;
    const INPUT_KEYBOARD: u32 = 1;
    const MOUSEEVENTF_MOVE: u32 = 0x0001;
    const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
    const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
    const MOUSEEVENTF_RIGHTDOWN: u32 = 0x0008;
    const MOUSEEVENTF_RIGHTUP: u32 = 0x0010;
    const MOUSEEVENTF_MIDDLEDOWN: u32 = 0x0020;
    const MOUSEEVENTF_MIDDLEUP: u32 = 0x0040;
    const MOUSEEVENTF_XDOWN: u32 = 0x0080;
    const MOUSEEVENTF_XUP: u32 = 0x0100;
    const MOUSEEVENTF_WHEEL: u32 = 0x0800;
    const MOUSEEVENTF_HWHEEL: u32 = 0x1000;
    const MOUSEEVENTF_ABSOLUTE: u32 = 0x8000;
    const KEYEVENTF_KEYDOWN: u32 = 0x0000;
    const KEYEVENTF_KEYUP: u32 = 0x0002;
    const SW_HIDE: i32 = 0;
    const SW_SHOWNORMAL: i32 = 1;
    const SW_SHOWMINIMIZED: i32 = 2;
    const SW_SHOWMAXIMIZED: i32 = 3;
    const SW_RESTORE: i32 = 9;
    const SRCCOPY: u32 = 0x00CC0020;
    const SM_CMONITORS: i32 = 80;
    const SM_XVIRTUALSCREEN: i32 = 76;
    const SM_YVIRTUALSCREEN: i32 = 77;
    pub const SM_CXVIRTUALSCREEN: i32 = 78;
    pub const SM_CYVIRTUALSCREEN: i32 = 79;
    const DIB_RGB_COLORS: u32 = 0;
    const XBUTTON1: u32 = 0x0001;
    const XBUTTON2: u32 = 0x0002;

    const VK_BACK: u16 = 0x08;
    const VK_TAB: u16 = 0x09;
    const VK_RETURN: u16 = 0x0D;
    const VK_SHIFT: u16 = 0x10;
    const VK_CONTROL: u16 = 0x11;
    const VK_MENU: u16 = 0x12;
    const VK_ESCAPE: u16 = 0x1B;
    const VK_SPACE: u16 = 0x20;
    const VK_LEFT: u16 = 0x25;
    const VK_UP: u16 = 0x26;
    const VK_RIGHT: u16 = 0x27;
    const VK_DOWN: u16 = 0x28;
    const VK_DELETE: u16 = 0x2E;

    fn rect_to_dto(r: RECT) -> Rect {
        Rect::new(
            r.left,
            r.top,
            (r.right - r.left) as u32,
            (r.bottom - r.top) as u32,
        )
    }

    pub fn get_window_text(hwnd: Hwnd) -> String {
        let len = unsafe { GetWindowTextLengthW(hwnd) };
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        let read = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1) };
        if read <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..read as usize])
    }

    fn get_class_name(hwnd: Hwnd) -> String {
        let mut buf = vec![0u16; 256];
        let read = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), 256) };
        if read <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..read as usize])
    }

    fn find_windows(title: &str, class: Option<&str>) -> Result<Vec<Hwnd>> {
        let title_lower = title.to_lowercase();
        let mut results: Vec<Hwnd> = Vec::new();

        unsafe {
            let callback: Option<unsafe extern "system" fn(Hwnd, isize) -> i32> =
                Some(hook_enum_windows);
            let lparam = &mut results as *mut Vec<Hwnd> as isize;
            EnumWindows(callback, lparam);
        }

        let hwnds = std::mem::take(&mut results);
        let mut filtered = Vec::new();

        for hwnd in hwnds {
            let window_title = get_window_text(hwnd);
            if !window_title.to_lowercase().contains(&title_lower) {
                continue;
            }
            if let Some(ref class_filter) = class {
                let window_class = get_class_name(hwnd);
                if !window_class.to_lowercase().contains(class_filter) {
                    continue;
                }
            }
            filtered.push(hwnd);
        }

        Ok(filtered)
    }

    unsafe extern "system" fn hook_enum_windows(hwnd: Hwnd, lparam: isize) -> i32 {
        let results = &mut *(lparam as *mut Vec<Hwnd>);
        results.push(hwnd);
        1
    }

    fn build_window_target(hwnd: Hwnd) -> Option<WindowTarget> {
        let title = get_window_text(hwnd);
        let class_name = get_class_name(hwnd);

        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };

        let visible = unsafe { IsWindowVisible(hwnd) != 0 };
        let has_rect = unsafe { GetWindowRect(hwnd, &mut rect) != 0 };

        if !visible && title.is_empty() {
            return None;
        }

        let bounds = if has_rect {
            rect_to_dto(rect)
        } else {
            Rect::new(0, 0, 0, 0)
        };

        let mut pid: u32 = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
        }

        let focused = unsafe { GetForegroundWindow() } == hwnd;

        Some(WindowTarget {
            id: format!("{:x}", hwnd as u64),
            title,
            class_name: Some(class_name),
            process_id: if pid > 0 { Some(pid) } else { None },
            bounds,
            is_visible: visible,
            is_focused: focused,
        })
    }

    pub struct Win32Automation;

    impl Win32Automation {
        pub fn mouse_click(x: i32, y: i32, button: MouseButton) -> Result<()> {
            unsafe {
                SetCursorPos(x, y);
            }

            let (down_flag, up_flag) = match button {
                MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
                MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
                MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
                MouseButton::X1 => (MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP),
                MouseButton::X2 => (MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP),
            };

            let xdata = match button {
                MouseButton::X1 => XBUTTON1,
                MouseButton::X2 => XBUTTON2,
                _ => 0,
            };

            let mut inputs = [
                build_mouse_input(down_flag, xdata, x, y),
                build_mouse_input(up_flag, xdata, x, y),
            ];

            let sent =
                unsafe { SendInput(2, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32) };

            if sent != 2 {
                return Err(crate::error::action_err("mouse click failed"));
            }

            Ok(())
        }

        pub fn mouse_double_click(x: i32, y: i32) -> Result<()> {
            Self::mouse_click(x, y, MouseButton::Left)?;
            Self::mouse_click(x, y, MouseButton::Left)?;
            Ok(())
        }

        pub fn mouse_move(x: i32, y: i32) -> Result<()> {
            unsafe { SetCursorPos(x, y) };
            Ok(())
        }

        pub fn mouse_drag(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> Result<()> {
            unsafe {
                SetCursorPos(from_x, from_y);
                let mut down = build_mouse_input(MOUSEEVENTF_LEFTDOWN, 0, from_x, from_y);
                SendInput(1, &mut down, std::mem::size_of::<INPUT>() as i32);

                SetCursorPos(to_x, to_y);
                let mut up = build_mouse_input(MOUSEEVENTF_LEFTUP, 0, to_x, to_y);
                SendInput(1, &mut up, std::mem::size_of::<INPUT>() as i32);
            }
            Ok(())
        }

        pub fn scroll(x: i32, y: i32, delta_x: i32, delta_y: i32) -> Result<()> {
            unsafe {
                SetCursorPos(x, y);
            }

            if delta_y != 0 {
                let mut input = build_mouse_input_raw(MOUSEEVENTF_WHEEL, delta_y as u32);
                unsafe {
                    SendInput(1, &mut input, std::mem::size_of::<INPUT>() as i32);
                }
            }
            if delta_x != 0 {
                let mut input = build_mouse_input_raw(MOUSEEVENTF_HWHEEL, delta_x as u32);
                unsafe {
                    SendInput(1, &mut input, std::mem::size_of::<INPUT>() as i32);
                }
            }
            Ok(())
        }

        pub fn key_press(key: &str) -> Result<()> {
            let vk = key_to_vk(key)
                .ok_or_else(|| crate::error::unsupported_err(format!("unknown key: {}", key)))?;
            send_key(vk, false)
        }

        pub fn key_release(key: &str) -> Result<()> {
            let vk = key_to_vk(key)
                .ok_or_else(|| crate::error::unsupported_err(format!("unknown key: {}", key)))?;
            send_key(vk, true)
        }

        pub fn key_combination(keys: &[&str]) -> Result<()> {
            for key in keys {
                let vk = key_to_vk(key).ok_or_else(|| {
                    crate::error::unsupported_err(format!("unknown key: {}", key))
                })?;
                send_key_down(vk)?;
            }
            for key in keys.iter().rev() {
                let vk = key_to_vk(key).ok_or_else(|| {
                    crate::error::unsupported_err(format!("unknown key: {}", key))
                })?;
                send_key_up(vk)?;
            }
            Ok(())
        }

        pub async fn type_text(text: &str, interval_ms: u64) -> Result<()> {
            for ch in text.chars() {
                let vk = char_to_vk(ch);
                if let Some(vk) = vk {
                    send_key_down(vk)?;
                    send_key_up(vk)?;
                }
                if interval_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
                }
            }
            Ok(())
        }

        pub fn screenshot(
            x: Option<i32>,
            y: Option<i32>,
            width: Option<u32>,
            height: Option<u32>,
        ) -> Result<Vec<u8>> {
            let (sx, sy, sw, sh) =
                if let (Some(x), Some(y), Some(w), Some(h)) = (x, y, width, height) {
                    (x, y, w, h)
                } else {
                    let screen_w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
                    let screen_h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
                    let screen_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
                    let screen_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
                    (screen_x, screen_y, screen_w as u32, screen_h as u32)
                };

            let hdc_screen = unsafe { GetDC(0) };
            if hdc_screen == 0 {
                return Err(crate::error::action_err("failed to get screen DC"));
            }

            let hdc_mem = unsafe { CreateCompatibleDC(hdc_screen) };
            if hdc_mem == 0 {
                unsafe { ReleaseDC(0, hdc_screen) };
                return Err(crate::error::action_err("failed to create compatible DC"));
            }

            let hbitmap = unsafe { CreateCompatibleBitmap(hdc_screen, sw as i32, sh as i32) };
            if hbitmap == 0 {
                unsafe {
                    DeleteDC(hdc_mem);
                    ReleaseDC(0, hdc_screen);
                }
                return Err(crate::error::action_err("failed to create bitmap"));
            }

            unsafe {
                SelectObject(hdc_mem, hbitmap);
            };

            let result = unsafe {
                BitBlt(
                    hdc_mem, 0, 0, sw as i32, sh as i32, hdc_screen, sx, sy, SRCCOPY,
                )
            };

            if result == 0 {
                unsafe {
                    DeleteObject(hbitmap);
                    DeleteDC(hdc_mem);
                    ReleaseDC(0, hdc_screen);
                }
                return Err(crate::error::action_err("BitBlt failed"));
            }

            let mut bmp = BITMAP {
                bmType: 0,
                bmWidth: 0,
                bmHeight: 0,
                bmWidthBytes: 0,
                bmPlanes: 0,
                bmBitsPixel: 0,
                bmBits: null_mut(),
            };

            unsafe { GetObjectW(hbitmap, std::mem::size_of::<BITMAP>() as i32, &mut bmp) };

            let bpp = bmp.bmBitsPixel as u32;
            let row_size = ((sw * bpp + 31) / 32) * 4;
            let pixel_data_size = row_size * sh;

            let bmi_header_size = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: bmi_header_size,
                    biWidth: sw as i32,
                    biHeight: -(sh as i32),
                    biPlanes: 1,
                    biBitCount: bpp as u16,
                    biCompression: 0,
                    biSizeImage: pixel_data_size,
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

            let mut pixel_data: Vec<u8> = vec![0; pixel_data_size as usize];

            let dib_result = unsafe {
                GetDIBits(
                    hdc_mem,
                    hbitmap,
                    0,
                    sh,
                    pixel_data.as_mut_ptr() as *mut std::ffi::c_void,
                    &mut bmi,
                    DIB_RGB_COLORS,
                )
            };

            if dib_result == 0 {
                unsafe {
                    DeleteObject(hbitmap);
                    DeleteDC(hdc_mem);
                    ReleaseDC(0, hdc_screen);
                }
                return Err(crate::error::action_err("GetDIBits failed"));
            }

            let bmp_header_size: u32 = 14;
            let file_size = bmp_header_size + bmi_header_size + pixel_data_size;

            let mut file_header = Vec::with_capacity(14);
            file_header.extend_from_slice(b"BM");
            file_header.extend_from_slice(&file_size.to_le_bytes());
            file_header.extend_from_slice(&[0u8; 4]);
            file_header.extend_from_slice(&(bmp_header_size + bmi_header_size).to_le_bytes());

            let mut final_buffer = Vec::with_capacity(file_size as usize);
            final_buffer.extend_from_slice(&file_header);

            let hdr_bytes = unsafe {
                std::slice::from_raw_parts(
                    &bmi.bmiHeader as *const BITMAPINFOHEADER as *const u8,
                    bmi_header_size as usize,
                )
            };
            final_buffer.extend_from_slice(hdr_bytes);
            final_buffer.extend_from_slice(&pixel_data);

            unsafe {
                DeleteObject(hbitmap);
                DeleteDC(hdc_mem);
                ReleaseDC(0, hdc_screen);
            }

            Ok(final_buffer)
        }

        pub fn get_pixel_color(x: i32, y: i32) -> Result<(u8, u8, u8)> {
            let hdc = unsafe { GetDC(0) };
            if hdc == 0 {
                return Err(crate::error::action_err("failed to get screen DC"));
            }
            let pixel = unsafe { GetPixel(hdc, x, y) };
            unsafe { ReleaseDC(0, hdc) };

            let r = (pixel & 0x000000FF) as u8;
            let g = ((pixel & 0x0000FF00) >> 8) as u8;
            let b = ((pixel & 0x00FF0000) >> 16) as u8;
            Ok((r, g, b))
        }

        pub fn get_active_window() -> Result<WindowTarget> {
            let hwnd = unsafe { GetForegroundWindow() };
            build_window_target(hwnd).ok_or_else(|| crate::error::not_found_err("no active window"))
        }

        pub fn find_window(title: &str, class: Option<&str>) -> Result<Vec<WindowTarget>> {
            let hwnds = find_windows(title, class)?;
            let mut results = Vec::new();
            for hwnd in hwnds {
                if let Some(target) = build_window_target(hwnd) {
                    results.push(target);
                }
            }
            Ok(results)
        }

        pub fn focus_window(window_id: &str) -> Result<()> {
            let hwnd = parse_hwnd(window_id)?;
            let result = unsafe { SetForegroundWindow(hwnd) };
            if result == 0 {
                return Err(crate::error::action_err(format!(
                    "failed to focus window {}",
                    window_id
                )));
            }
            Ok(())
        }

        pub fn get_window_bounds(window_id: &str) -> Result<Rect> {
            let hwnd = parse_hwnd(window_id)?;
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            let result = unsafe { GetWindowRect(hwnd, &mut rect) };
            if result == 0 {
                return Err(crate::error::not_found_err(format!(
                    "window {} not found",
                    window_id
                )));
            }
            Ok(rect_to_dto(rect))
        }

        pub fn resize_window(window_id: &str, width: u32, height: u32) -> Result<()> {
            let hwnd = parse_hwnd(window_id)?;
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            unsafe { GetWindowRect(hwnd, &mut rect) };
            let result =
                unsafe { MoveWindow(hwnd, rect.left, rect.top, width as i32, height as i32, 1) };
            if result == 0 {
                return Err(crate::error::action_err(format!(
                    "failed to resize window {}",
                    window_id
                )));
            }
            Ok(())
        }

        pub fn move_window(window_id: &str, x: i32, y: i32) -> Result<()> {
            let hwnd = parse_hwnd(window_id)?;
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            unsafe { GetWindowRect(hwnd, &mut rect) };
            let w = rect.right - rect.left;
            let h = rect.bottom - rect.top;
            let result = unsafe { MoveWindow(hwnd, x, y, w, h, 1) };
            if result == 0 {
                return Err(crate::error::action_err(format!(
                    "failed to move window {}",
                    window_id
                )));
            }
            Ok(())
        }

        pub fn close_window(window_id: &str) -> Result<()> {
            let hwnd = parse_hwnd(window_id)?;
            unsafe {
                CloseWindow(hwnd);
            }
            Ok(())
        }

        pub fn minimize_window(window_id: &str) -> Result<()> {
            let hwnd = parse_hwnd(window_id)?;
            unsafe { ShowWindow(hwnd, SW_SHOWMINIMIZED) };
            Ok(())
        }

        pub fn maximize_window(window_id: &str) -> Result<()> {
            let hwnd = parse_hwnd(window_id)?;
            unsafe { ShowWindow(hwnd, SW_SHOWMAXIMIZED) };
            Ok(())
        }

        pub fn restore_window(window_id: &str) -> Result<()> {
            let hwnd = parse_hwnd(window_id)?;
            unsafe { ShowWindow(hwnd, SW_RESTORE) };
            Ok(())
        }
    }

    pub fn parse_hwnd(window_id: &str) -> Result<Hwnd> {
        let hwnd = isize::from_str_radix(window_id, 16).map_err(|_| {
            crate::error::not_found_err(format!("invalid window id: {}", window_id))
        })?;
        Ok(hwnd)
    }

    fn build_mouse_input(dwFlags: u32, mouse_data: u32, x: i32, y: i32) -> INPUT {
        INPUT {
            type_: INPUT_MOUSE,
            u: INPUT_UNION {
                mi: MOUSEINPUT {
                    dx: x,
                    dy: y,
                    mouseData: mouse_data,
                    dwFlags: dwFlags | MOUSEEVENTF_ABSOLUTE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn build_mouse_input_raw(dwFlags: u32, mouse_data: u32) -> INPUT {
        INPUT {
            type_: INPUT_MOUSE,
            u: INPUT_UNION {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: mouse_data,
                    dwFlags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn send_key(vk: u16, release: bool) -> Result<()> {
        if release {
            send_key_up(vk)
        } else {
            send_key_down(vk)
        }
    }

    fn send_key_down(vk: u16) -> Result<()> {
        let mut input = INPUT {
            type_: INPUT_KEYBOARD,
            u: INPUT_UNION {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYDOWN,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let sent = unsafe { SendInput(1, &mut input, std::mem::size_of::<INPUT>() as i32) };
        if sent != 1 {
            return Err(crate::error::action_err("key down failed"));
        }
        Ok(())
    }

    fn send_key_up(vk: u16) -> Result<()> {
        let mut input = INPUT {
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
        };
        let sent = unsafe { SendInput(1, &mut input, std::mem::size_of::<INPUT>() as i32) };
        if sent != 1 {
            return Err(crate::error::action_err("key up failed"));
        }
        Ok(())
    }

    fn key_to_vk(key: &str) -> Option<u16> {
        match key.to_uppercase().as_str() {
            "BACK" | "BACKSPACE" => Some(VK_BACK),
            "TAB" => Some(VK_TAB),
            "ENTER" | "RETURN" => Some(VK_RETURN),
            "SHIFT" => Some(VK_SHIFT),
            "CTRL" | "CONTROL" => Some(VK_CONTROL),
            "ALT" | "MENU" => Some(VK_MENU),
            "ESC" | "ESCAPE" => Some(VK_ESCAPE),
            "WIN" | "WINDOWS" | "SUPER" | "LWIN" => Some(0x5B),
            "RWIN" => Some(0x5C),
            "VOLUMEUP" | "VOLUME_UP" => Some(0xAF),
            "VOLUMEDOWN" | "VOLUME_DOWN" => Some(0xAE),
            "VOLUMEMUTE" | "VOLUME_MUTE" | "MUTE" => Some(0xAD),
            "SPACE" => Some(VK_SPACE),
            "LEFT" => Some(VK_LEFT),
            "UP" => Some(VK_UP),
            "RIGHT" => Some(VK_RIGHT),
            "DOWN" => Some(VK_DOWN),
            "DEL" | "DELETE" => Some(VK_DELETE),
            "F1" => Some(0x70),
            "F2" => Some(0x71),
            "F3" => Some(0x72),
            "F4" => Some(0x73),
            "F5" => Some(0x74),
            "F6" => Some(0x75),
            "F7" => Some(0x76),
            "F8" => Some(0x77),
            "F9" => Some(0x78),
            "F10" => Some(0x79),
            "F11" => Some(0x7A),
            "F12" => Some(0x7B),
            "A" => Some(0x41),
            "B" => Some(0x42),
            "C" => Some(0x43),
            "D" => Some(0x44),
            "E" => Some(0x45),
            "F" => Some(0x46),
            "G" => Some(0x47),
            "H" => Some(0x48),
            "I" => Some(0x49),
            "J" => Some(0x4A),
            "K" => Some(0x4B),
            "L" => Some(0x4C),
            "M" => Some(0x4D),
            "N" => Some(0x4E),
            "O" => Some(0x4F),
            "P" => Some(0x50),
            "Q" => Some(0x51),
            "R" => Some(0x52),
            "S" => Some(0x53),
            "T" => Some(0x54),
            "U" => Some(0x55),
            "V" => Some(0x56),
            "W" => Some(0x57),
            "X" => Some(0x58),
            "Y" => Some(0x59),
            "Z" => Some(0x5A),
            "0" => Some(0x30),
            "1" => Some(0x31),
            "2" => Some(0x32),
            "3" => Some(0x33),
            "4" => Some(0x34),
            "5" => Some(0x35),
            "6" => Some(0x36),
            "7" => Some(0x37),
            "8" => Some(0x38),
            "9" => Some(0x39),
            _ => None,
        }
    }

    fn char_to_vk(ch: char) -> Option<u16> {
        match ch {
            'a'..='z' => Some((ch as u8).to_ascii_uppercase() as u16),
            'A'..='Z' => Some(ch as u16),
            '0'..='9' => Some(ch as u16),
            ' ' => Some(VK_SPACE),
            '\r' | '\n' => Some(VK_RETURN),
            '\t' => Some(VK_TAB),
            _ => None,
        }
    }

    use std::sync::atomic::{AtomicBool, Ordering};

    pub struct UiaEngine {
        initialized: AtomicBool,
    }

    impl UiaEngine {
        pub fn new() -> Self {
            Self {
                initialized: AtomicBool::new(false),
            }
        }

        pub fn initialize(&self) -> Result<()> {
            self.initialized.store(true, Ordering::SeqCst);
            info!("UI Automation engine initialized");
            Ok(())
        }

        pub fn find_elements(&self, selector: &ElementSelector) -> Result<Vec<ElementInfo>> {
            let mut results = Vec::new();

            if let Some(ref name) = selector.name {
                let hwnds = find_windows(name, None)?;
                for hwnd in hwnds {
                    let title = get_window_text(hwnd);
                    if title.to_lowercase().contains(&name.to_lowercase()) {
                        let mut rect = RECT {
                            left: 0,
                            top: 0,
                            right: 0,
                            bottom: 0,
                        };
                        unsafe { GetWindowRect(hwnd, &mut rect) };
                        results.push(ElementInfo {
                            id: format!("{:x}", hwnd as u64),
                            name: title.clone(),
                            control_type: "Window".to_string(),
                            bounds: rect_to_dto(rect),
                            is_enabled: true,
                            is_visible: unsafe { IsWindowVisible(hwnd) != 0 },
                            text: Some(title),
                            children: vec![],
                        });
                    }
                }
            }

            if let Some(ref automation_id) = selector.automation_id {
                let hwnds = find_windows(automation_id, None)?;
                for hwnd in hwnds {
                    let class = get_class_name(hwnd);
                    if let Some(ref expected_class) = selector.class_name {
                        if !class
                            .to_lowercase()
                            .contains(&expected_class.to_lowercase())
                        {
                            continue;
                        }
                    }
                    let mut rect = RECT {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    };
                    unsafe { GetWindowRect(hwnd, &mut rect) };
                    results.push(ElementInfo {
                        id: format!("{:x}", hwnd as u64),
                        name: get_window_text(hwnd),
                        control_type: class,
                        bounds: rect_to_dto(rect),
                        is_enabled: true,
                        is_visible: unsafe { IsWindowVisible(hwnd) != 0 },
                        text: None,
                        children: vec![],
                    });
                }
            }

            if let Some(ref class_name) = selector.class_name {
                let class_lower = class_name.to_lowercase();
                let all_hwnds = find_windows("", None)?;
                for hwnd in all_hwnds {
                    let class = get_class_name(hwnd);
                    if class.to_lowercase().contains(&class_lower) {
                        let mut rect = RECT {
                            left: 0,
                            top: 0,
                            right: 0,
                            bottom: 0,
                        };
                        unsafe { GetWindowRect(hwnd, &mut rect) };
                        let title = get_window_text(hwnd);
                        results.push(ElementInfo {
                            id: format!("{:x}", hwnd as u64),
                            name: title.clone(),
                            control_type: class,
                            bounds: rect_to_dto(rect),
                            is_enabled: true,
                            is_visible: unsafe { IsWindowVisible(hwnd) != 0 },
                            text: if title.is_empty() { None } else { Some(title) },
                            children: vec![],
                        });
                    }
                }
            }

            if let Some(index) = selector.index {
                if index < results.len() {
                    let elem = results.swap_remove(index);
                    results.clear();
                    results.push(elem);
                } else {
                    results.clear();
                }
            }

            Ok(results)
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;

    pub struct WindowsUiaBackendInner;

    impl WindowsUiaBackendInner {
        pub fn new() -> Self {
            Self
        }
    }

    pub struct Win32Automation;

    impl Win32Automation {
        pub fn mouse_click(_x: i32, _y: i32, _button: MouseButton) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn mouse_double_click(_x: i32, _y: i32) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn mouse_move(_x: i32, _y: i32) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn mouse_drag(_from_x: i32, _from_y: i32, _to_x: i32, _to_y: i32) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn scroll(_x: i32, _y: i32, _delta_x: i32, _delta_y: i32) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn key_press(_key: &str) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn key_release(_key: &str) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn key_combination(_keys: &[&str]) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn type_text(_text: &str, _interval_ms: u64) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn screenshot(
            _x: Option<i32>,
            _y: Option<i32>,
            _width: Option<u32>,
            _height: Option<u32>,
        ) -> Result<Vec<u8>> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn get_pixel_color(_x: i32, _y: i32) -> Result<(u8, u8, u8)> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn get_active_window() -> Result<WindowTarget> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn find_window(_title: &str, _class: Option<&str>) -> Result<Vec<WindowTarget>> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn focus_window(_window_id: &str) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn get_window_bounds(_window_id: &str) -> Result<Rect> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn resize_window(_window_id: &str, _width: u32, _height: u32) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn move_window(_window_id: &str, _x: i32, _y: i32) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn close_window(_window_id: &str) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn minimize_window(_window_id: &str) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn maximize_window(_window_id: &str) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
        pub fn restore_window(_window_id: &str) -> Result<()> {
            Err(crate::error::unavail_err(
                "Win32 automation not available on this platform",
            ))
        }
    }

    pub struct UiaEngine;

    impl UiaEngine {
        pub fn new() -> Self {
            Self
        }
        pub fn initialize(&self) -> Result<()> {
            Err(crate::error::unavail_err(
                "UI Automation not available on this platform",
            ))
        }
        pub fn find_elements(&self, _selector: &ElementSelector) -> Result<Vec<ElementInfo>> {
            Err(crate::error::unavail_err(
                "UI Automation not available on this platform",
            ))
        }
    }
}

use platform::*;

pub struct WindowsUiaBackend {
    inner: Arc<RwLock<WindowsUiaBackendInner>>,
    uia: UiaEngine,
    name: String,
}

impl WindowsUiaBackend {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(WindowsUiaBackendInner::new())),
            uia: UiaEngine::new(),
            name: "windows-uia".to_string(),
        }
    }
}

impl Default for WindowsUiaBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AutomationBackend for WindowsUiaBackend {
    async fn name(&self) -> &str {
        &self.name
    }

    async fn initialize(&self) -> Result<()> {
        self.inner.write().initialize()?;
        self.uia.initialize()?;
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        info!("Windows UIA backend shutting down");
        Ok(())
    }

    async fn is_available(&self) -> bool {
        cfg!(windows)
    }

    async fn click(&self, x: i32, y: i32, button: MouseButton) -> Result<()> {
        Win32Automation::mouse_click(x, y, button)
    }

    async fn double_click(&self, x: i32, y: i32) -> Result<()> {
        Win32Automation::mouse_double_click(x, y)
    }

    async fn move_mouse(&self, x: i32, y: i32) -> Result<()> {
        Win32Automation::mouse_move(x, y)
    }

    async fn drag(&self, from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> Result<()> {
        Win32Automation::mouse_drag(from_x, from_y, to_x, to_y)
    }

    async fn scroll(&self, x: i32, y: i32, delta_x: i32, delta_y: i32) -> Result<()> {
        Win32Automation::scroll(x, y, delta_x, delta_y)
    }

    async fn type_text(&self, text: &str, interval_ms: u64) -> Result<()> {
        Win32Automation::type_text(text, interval_ms).await
    }

    async fn key_press(&self, key: &str) -> Result<()> {
        Win32Automation::key_press(key)
    }

    async fn key_combination(&self, keys: &[&str]) -> Result<()> {
        Win32Automation::key_combination(keys)
    }

    async fn hold_key(&self, key: &str, duration_ms: u64) -> Result<()> {
        Win32Automation::key_press(key)?;
        tokio::time::sleep(std::time::Duration::from_millis(duration_ms)).await;
        Win32Automation::key_release(key)
    }

    async fn screenshot(
        &self,
        x: Option<i32>,
        y: Option<i32>,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Result<Vec<u8>> {
        Win32Automation::screenshot(x, y, width, height)
    }

    async fn screen_size(&self) -> Result<(u32, u32)> {
        let w = unsafe { platform::GetSystemMetrics(platform::SM_CXVIRTUALSCREEN) };
        let h = unsafe { platform::GetSystemMetrics(platform::SM_CYVIRTUALSCREEN) };
        Ok((w as u32, h as u32))
    }

    async fn get_pixel_color(&self, x: i32, y: i32) -> Result<(u8, u8, u8)> {
        Win32Automation::get_pixel_color(x, y)
    }

    async fn get_active_window(&self) -> Result<WindowTarget> {
        Win32Automation::get_active_window()
    }

    async fn find_window(&self, title: &str, class: Option<&str>) -> Result<Vec<WindowTarget>> {
        Win32Automation::find_window(title, class)
    }

    async fn focus_window(&self, window_id: &str) -> Result<()> {
        Win32Automation::focus_window(window_id)
    }

    async fn get_window_bounds(&self, window_id: &str) -> Result<Rect> {
        Win32Automation::get_window_bounds(window_id)
    }

    async fn resize_window(&self, window_id: &str, width: u32, height: u32) -> Result<()> {
        Win32Automation::resize_window(window_id, width, height)
    }

    async fn move_window(&self, window_id: &str, x: i32, y: i32) -> Result<()> {
        Win32Automation::move_window(window_id, x, y)
    }

    async fn close_window(&self, window_id: &str) -> Result<()> {
        Win32Automation::close_window(window_id)
    }

    async fn minimize_window(&self, window_id: &str) -> Result<()> {
        Win32Automation::minimize_window(window_id)
    }

    async fn maximize_window(&self, window_id: &str) -> Result<()> {
        Win32Automation::maximize_window(window_id)
    }

    async fn restore_window(&self, window_id: &str) -> Result<()> {
        Win32Automation::restore_window(window_id)
    }

    async fn find_element(&self, selector: &ElementSelector) -> Result<Vec<ElementInfo>> {
        self.uia.find_elements(selector)
    }

    async fn get_element_text(&self, element_id: &str) -> Result<String> {
        let hwnd = parse_hwnd(element_id)?;
        Ok(get_window_text(hwnd))
    }

    async fn click_element(&self, element_id: &str) -> Result<()> {
        let bounds = Win32Automation::get_window_bounds(element_id)?;
        let (cx, cy) = bounds.center();
        Win32Automation::mouse_click(cx, cy, MouseButton::Left)
    }

    async fn get_element_bounds(&self, element_id: &str) -> Result<Rect> {
        Win32Automation::get_window_bounds(element_id)
    }

    async fn wait_for_element(
        &self,
        selector: &ElementSelector,
        timeout_ms: u64,
    ) -> Result<ElementInfo> {
        let start = std::time::Instant::now();
        loop {
            let elements = self.uia.find_elements(selector)?;
            if let Some(elem) = elements.into_iter().next() {
                return Ok(elem);
            }
            if start.elapsed().as_millis() as u64 >= timeout_ms {
                return Err(crate::error::timeout_err(format!(
                    "element not found within {}ms",
                    timeout_ms
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    async fn ocr_region(&self, _image: &[u8], _language: Option<&str>) -> Result<String> {
        Err(crate::error::unsupported_err(
            "OCR not implemented in Windows UIA backend",
        ))
    }

    async fn find_text_on_screen(&self, _text: &str, _region: Option<Rect>) -> Result<Vec<Rect>> {
        Err(crate::error::unsupported_err(
            "text search not implemented in Windows UIA backend",
        ))
    }

    async fn verify_state(&self, _expected: &StateVerification) -> Result<bool> {
        Err(crate::error::unsupported_err(
            "verification delegated to VerificationEngine",
        ))
    }

    async fn recover(&self, _error: &str) -> Result<bool> {
        Err(crate::error::unsupported_err(
            "recovery delegated to RecoveryEngine",
        ))
    }

    async fn get_backend_capabilities(&self) -> Vec<AutomationCapability> {
        vec![
            AutomationCapability::Mouse,
            AutomationCapability::Keyboard,
            AutomationCapability::ScreenCapture,
            AutomationCapability::WindowManagement,
            AutomationCapability::ElementDetection,
        ]
    }
}
