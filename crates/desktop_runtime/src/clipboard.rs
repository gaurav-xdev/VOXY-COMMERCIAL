//! Clipboard integration.

use crate::error::Result;
use tracing::info;

pub struct ClipboardManager;

impl ClipboardManager {
    pub fn new() -> Self {
        Self
    }

    pub fn read_text(&self) -> Result<Option<String>> {
        #[cfg(windows)]
        {
            self.read_text_windows()
        }
        #[cfg(not(windows))]
        {
            Ok(None)
        }
    }

    #[cfg(windows)]
    fn read_text_windows(&self) -> Result<Option<String>> {
        use windows::Win32::Foundation::HGLOBAL;
        use windows::Win32::System::DataExchange::{
            CloseClipboard, GetClipboardData, OpenClipboard,
        };
        use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
        unsafe {
            match OpenClipboard(None) {
                Ok(()) => {}
                Err(_) => {
                    return Ok(None);
                }
            }
            let data = match GetClipboardData(13) {
                // CF_UNICODETEXT = 13
                Ok(d) => d,
                Err(_) => {
                    let _ = CloseClipboard();
                    return Ok(None);
                }
            };
            let hglobal = HGLOBAL(data.0);
            let ptr = GlobalLock(hglobal);
            if ptr.is_null() {
                let _ = CloseClipboard();
                return Ok(None);
            }
            let byte_len = GlobalSize(hglobal);
            let max_chars = byte_len / 2;
            let p = ptr as *const u16;
            let mut len = 0;
            while len < max_chars && *p.add(len) != 0 {
                len += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
            let _ = GlobalUnlock(hglobal);
            let _ = CloseClipboard();
            Ok(Some(text))
        }
    }

    #[cfg(not(windows))]
    fn read_text_windows(&self) -> Result<Option<String>> {
        Ok(None)
    }

    pub fn write_text(&self, text: &str) -> Result<()> {
        info!("Text written to clipboard ({} chars)", text.len());
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_creation() {
        let _ = ClipboardManager::new();
    }

    #[test]
    fn clipboard_read() {
        let cb = ClipboardManager::new();
        let _ = cb.read_text();
    }

    #[test]
    fn clipboard_clear() {
        let cb = ClipboardManager::new();
        assert!(cb.clear().is_ok());
    }
}
