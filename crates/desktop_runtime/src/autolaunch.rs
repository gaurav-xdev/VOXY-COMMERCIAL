//! Auto-launch on boot via Windows registry.

use crate::error::{Result, RuntimeError};
use tracing::info;

pub struct AutoLauncher {
    app_name: String,
    run_key: String,
}

impl AutoLauncher {
    pub fn new(app_name: &str) -> Self {
        Self {
            app_name: app_name.to_string(),
            run_key: "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run".to_string(),
        }
    }

    pub fn enable(&self) -> Result<()> {
        let exe_path = self.get_exe_path()?;
        let cmd = format!("\"{}\" --autostart", exe_path);
        #[cfg(windows)]
        {
            self.set_registry_value(&cmd)?;
        }
        info!(
            "Auto-launch enabled for {} with command: {}",
            self.app_name, cmd
        );
        Ok(())
    }

    pub fn disable(&self) -> Result<()> {
        #[cfg(windows)]
        {
            self.delete_registry_value()?;
        }
        info!("Auto-launch disabled for {}", self.app_name);
        Ok(())
    }

    pub fn is_enabled(&self) -> bool {
        #[cfg(windows)]
        {
            self.get_registry_value().is_some()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    fn get_exe_path(&self) -> Result<String> {
        std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|e| RuntimeError::Registry(format!("Failed to get exe path: {}", e)))
    }

    #[cfg(windows)]
    fn set_registry_value(&self, value: &str) -> Result<()> {
        use windows::Win32::System::Registry::{
            RegCreateKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_ALL_ACCESS,
            REG_OPTION_NON_VOLATILE, REG_SZ,
        };

        unsafe {
            let key_wide: Vec<u16> = self
                .run_key
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let value_name_wide: Vec<u16> = self
                .app_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let value_bytes = {
                let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
                let mut buf = vec![0u8; wide.len() * 2];
                for (i, w) in wide.iter().enumerate() {
                    buf[i * 2] = (w & 0xFF) as u8;
                    buf[i * 2 + 1] = ((w >> 8) & 0xFF) as u8;
                }
                buf
            };

            let mut hkey = Default::default();
            let result = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                windows::core::PCWSTR::from_raw(key_wide.as_ptr()),
                0,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_ALL_ACCESS,
                None,
                &mut hkey,
                None,
            );
            if result.is_err() {
                return Err(RuntimeError::Registry(format!(
                    "RegCreateKeyExW failed: {:?}",
                    result
                )));
            }

            let result = RegSetValueExW(
                hkey,
                windows::core::PCWSTR::from_raw(value_name_wide.as_ptr()),
                0,
                REG_SZ,
                Some(&value_bytes),
            );
            let _ = windows::Win32::System::Registry::RegCloseKey(hkey);

            if result.is_err() {
                return Err(RuntimeError::Registry(format!(
                    "RegSetValueExW failed: {:?}",
                    result
                )));
            }
            Ok(())
        }
    }

    #[cfg(not(windows))]
    fn set_registry_value(&self, _value: &str) -> Result<()> {
        Ok(())
    }

    #[cfg(windows)]
    fn delete_registry_value(&self) -> Result<()> {
        use windows::Win32::System::Registry::{
            RegDeleteValueW, RegOpenKeyExW, HKEY_CURRENT_USER, KEY_ALL_ACCESS,
        };
        unsafe {
            let key_wide: Vec<u16> = self
                .run_key
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let value_name_wide: Vec<u16> = self
                .app_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut hkey = Default::default();
            let result = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                windows::core::PCWSTR::from_raw(key_wide.as_ptr()),
                0,
                KEY_ALL_ACCESS,
                &mut hkey,
            );
            if result.is_err() {
                return Ok(());
            }
            let _ = RegDeleteValueW(
                hkey,
                windows::core::PCWSTR::from_raw(value_name_wide.as_ptr()),
            );
            let _ = windows::Win32::System::Registry::RegCloseKey(hkey);
        }
        Ok(())
    }

    #[cfg(not(windows))]
    fn delete_registry_value(&self) -> Result<()> {
        Ok(())
    }

    #[cfg(windows)]
    fn get_registry_value(&self) -> Option<String> {
        use windows::Win32::System::Registry::{
            RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, REG_SZ,
        };
        unsafe {
            let key_wide: Vec<u16> = self
                .run_key
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let value_name_wide: Vec<u16> = self
                .app_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut hkey = Default::default();
            let result = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                windows::core::PCWSTR::from_raw(key_wide.as_ptr()),
                0,
                KEY_READ,
                &mut hkey,
            );
            if result.is_err() {
                return None;
            }
            let mut buffer = [0u16; 512];
            let mut buffer_size = (buffer.len() * 2) as u32;
            let mut reg_type = REG_SZ;
            let result = RegQueryValueExW(
                hkey,
                windows::core::PCWSTR::from_raw(value_name_wide.as_ptr()),
                None,
                Some(&mut reg_type),
                Some(buffer.as_mut_ptr() as *mut u8),
                Some(&mut buffer_size),
            );
            let _ = windows::Win32::System::Registry::RegCloseKey(hkey);
            if result.is_err() {
                return None;
            }
            let len = (buffer_size / 2) as usize;
            if len > 0 && buffer[len - 1] == 0 {
                Some(String::from_utf16_lossy(&buffer[..len - 1]))
            } else {
                Some(String::from_utf16_lossy(&buffer[..len]))
            }
        }
    }

    #[cfg(not(windows))]
    fn get_registry_value(&self) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autolaunch_creation() {
        let al = AutoLauncher::new("VOXY");
        assert_eq!(al.app_name, "VOXY");
    }

    #[test]
    fn autolaunch_exe_path() {
        let al = AutoLauncher::new("VOXY");
        assert!(al.get_exe_path().is_ok());
    }
}
