//! Platform abstraction traits.

use async_trait::async_trait;

use crate::{
    AudioDevice, DisplayInfo, FileInfo, NetworkInfo, PlatformInfo, ProcessInfo, Result, WindowInfo,
};

/// Core platform trait — all platform implementations must provide this.
#[async_trait]
pub trait Platform: Send + Sync {
    /// Get platform information.
    fn info(&self) -> PlatformInfo;

    /// Get the platform name.
    fn name(&self) -> &str;

    /// Initialize the platform.
    async fn initialize(&mut self) -> Result<()>;

    /// Shutdown the platform.
    async fn shutdown(&mut self) -> Result<()>;
}

/// Window management platform interface.
#[async_trait]
pub trait WindowPlatform: Send + Sync {
    /// List all windows.
    async fn list_windows(&self) -> Result<Vec<WindowInfo>>;

    /// Get the foreground window.
    async fn foreground_window(&self) -> Result<Option<WindowInfo>>;

    /// Focus a window by ID.
    async fn focus_window(&self, id: u64) -> Result<()>;

    /// Close a window by ID.
    async fn close_window(&self, id: u64) -> Result<()>;

    /// Minimize a window by ID.
    async fn minimize_window(&self, _id: u64) -> Result<()> {
        Ok(())
    }

    /// Maximize a window by ID.
    async fn maximize_window(&self, _id: u64) -> Result<()> {
        Ok(())
    }

    /// Restore a window by ID.
    async fn restore_window(&self, _id: u64) -> Result<()> {
        Ok(())
    }

    /// Resize a window by ID.
    async fn resize_window(&self, _id: u64, _width: u32, _height: u32) -> Result<()> {
        Ok(())
    }

    /// Move a window by ID.
    async fn move_window(&self, _id: u64, _x: i32, _y: i32) -> Result<()> {
        Ok(())
    }
}

/// Input simulation platform interface.
#[async_trait]
pub trait InputPlatform: Send + Sync {
    /// Simulate a mouse click.
    async fn mouse_click(&self, x: i32, y: i32) -> Result<()>;

    /// Simulate keyboard input.
    async fn keyboard_type(&self, text: &str) -> Result<()>;

    /// Simulate a key press.
    async fn key_press(&self, key: &str) -> Result<()>;

    /// Get the current mouse position.
    async fn mouse_position(&self) -> Result<(i32, i32)>;
}

/// Display/screen platform interface.
#[async_trait]
pub trait DisplayPlatform: Send + Sync {
    /// List all displays.
    async fn list_displays(&self) -> Result<Vec<DisplayInfo>>;

    /// Capture a screenshot of a display.
    async fn screenshot(&self, display_id: u32) -> Result<Vec<u8>>;

    /// Get the display DPI.
    async fn display_dpi(&self, display_id: u32) -> Result<(f64, f64)>;
}

/// Audio platform interface.
#[async_trait]
pub trait AudioPlatform: Send + Sync {
    /// List audio input devices.
    async fn input_devices(&self) -> Result<Vec<AudioDevice>>;

    /// List audio output devices.
    async fn output_devices(&self) -> Result<Vec<AudioDevice>>;

    /// Get the default input device.
    async fn default_input_device(&self) -> Result<Option<AudioDevice>>;

    /// Get the default output device.
    async fn default_output_device(&self) -> Result<Option<AudioDevice>>;
}

/// File system platform interface.
#[async_trait]
pub trait FileSystemPlatform: Send + Sync {
    /// Get file info.
    async fn file_info(&self, path: &str) -> Result<FileInfo>;

    /// List directory contents.
    async fn list_dir(&self, path: &str) -> Result<Vec<FileInfo>>;

    /// Check if a path exists.
    async fn path_exists(&self, path: &str) -> bool;

    /// Get the home directory.
    async fn home_dir(&self) -> Result<String>;

    /// Get the config directory.
    async fn config_dir(&self) -> Result<String>;

    /// Get the data directory.
    async fn data_dir(&self) -> Result<String>;
}

/// Network platform interface.
#[async_trait]
pub trait NetworkPlatform: Send + Sync {
    /// Get network information.
    async fn network_info(&self) -> Result<NetworkInfo>;

    /// Check if the network is available.
    async fn is_online(&self) -> bool;
}

/// Process management platform interface.
#[async_trait]
pub trait ProcessPlatform: Send + Sync {
    /// List running processes.
    async fn list_processes(&self) -> Result<Vec<ProcessInfo>>;

    /// Get process info by PID.
    async fn process_info(&self, pid: u32) -> Result<Option<ProcessInfo>>;

    /// Get the current process ID.
    fn current_pid(&self) -> u32;
}
