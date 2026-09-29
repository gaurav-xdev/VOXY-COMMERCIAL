pub mod compatibility;
pub mod config;
pub mod device;
pub mod error;
pub mod event;
pub mod hardware_detector;
pub mod model_lifecycle;
pub mod model_scanner;
pub mod traits;
pub mod vram_manager;

pub use compatibility::*;
pub use config::HardwareConfig;
pub use device::{DeviceInfo, DeviceStatus, DeviceType};
pub use error::{HardwareError, Result};
pub use event::HardwareEvent;
pub use hardware_detector::*;
pub use model_lifecycle::*;
pub use model_scanner::*;
pub use traits::*;
pub use vram_manager::*;

pub mod prelude {
    pub use crate::compatibility::*;
    pub use crate::config::{AudioConfig, HardwareConfig, VideoConfig};
    pub use crate::device::{DeviceInfo, DeviceStatus, DeviceType};
    pub use crate::error::{HardwareError, Result};
    pub use crate::event::HardwareEvent;
    pub use crate::hardware_detector::*;
    pub use crate::model_lifecycle::*;
    pub use crate::model_scanner::*;
    pub use crate::traits::{Camera, HardwareInfo, HardwareMonitor, Microphone, Speaker};
    pub use crate::vram_manager::*;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AudioConfig, VideoConfig};
    use crate::device::{DeviceStatus, DeviceType};
    use crate::error::HardwareError;

    #[test]
    fn test_audio_config_default() {
        let cfg = AudioConfig::default();
        assert_eq!(cfg.sample_rate, 16000);
        assert_eq!(cfg.channels, 1);
        assert_eq!(cfg.bit_depth, 16);
        assert_eq!(cfg.buffer_size_ms, 100);
        assert!(cfg.device_id.is_none());
    }

    #[test]
    fn test_audio_config_construction() {
        let cfg = AudioConfig {
            sample_rate: 44100,
            channels: 2,
            bit_depth: 24,
            buffer_size_ms: 50,
            device_id: Some("mic1".to_string()),
        };
        assert_eq!(cfg.sample_rate, 44100);
        assert_eq!(cfg.channels, 2);
        assert_eq!(cfg.device_id.unwrap(), "mic1");
    }

    #[test]
    fn test_video_config_default() {
        let cfg = VideoConfig::default();
        assert_eq!(cfg.width, 640);
        assert_eq!(cfg.height, 480);
        assert_eq!(cfg.fps, 30);
        assert!(cfg.device_id.is_none());
    }

    #[test]
    fn test_hardware_config_default() {
        let cfg = HardwareConfig::default();
        assert_eq!(cfg.audio_input.sample_rate, 16000);
        assert_eq!(cfg.video.width, 640);
        assert!(!cfg.enable_hardware_monitoring);
        assert_eq!(cfg.monitor_interval_ms, 5000);
    }

    #[test]
    fn test_hardware_event_display() {
        let event = HardwareEvent::DeviceConnected {
            device_id: "cam1".to_string(),
            device_type: "Camera".to_string(),
        };
        let s = format!("{}", event);
        assert!(s.contains("Device connected"));
        assert!(s.contains("cam1"));
    }

    #[test]
    fn test_device_type_display() {
        assert_eq!(format!("{}", DeviceType::Microphone), "Microphone");
        assert_eq!(format!("{}", DeviceType::Speaker), "Speaker");
        assert_eq!(format!("{}", DeviceType::Camera), "Camera");
    }

    #[test]
    fn test_device_status_display() {
        assert_eq!(format!("{}", DeviceStatus::Available), "Available");
        assert_eq!(format!("{}", DeviceStatus::Busy), "Busy");
        assert_eq!(format!("{}", DeviceStatus::Disconnected), "Disconnected");
    }

    #[test]
    fn test_device_type_equality() {
        assert_eq!(DeviceType::Microphone, DeviceType::Microphone);
        assert_ne!(DeviceType::Microphone, DeviceType::Camera);
    }

    #[test]
    fn test_device_status_equality() {
        assert_eq!(DeviceStatus::Available, DeviceStatus::Available);
        assert_ne!(DeviceStatus::Available, DeviceStatus::Busy);
    }

    #[test]
    fn test_hardware_info_construction() {
        let info = HardwareInfo {
            audio_inputs: vec!["mic1".to_string()],
            audio_outputs: vec!["spk1".to_string()],
            cameras: vec!["cam1".to_string()],
            audio_input_available: true,
            audio_output_available: false,
            camera_available: true,
        };
        assert_eq!(info.audio_inputs.len(), 1);
        assert!(info.audio_input_available);
        assert!(!info.audio_output_available);
        assert!(info.camera_available);
    }

    #[test]
    fn test_hardware_error_display() {
        let err = HardwareError::DeviceNotFound("mic1".to_string());
        assert_eq!(format!("{}", err), "Device not found: mic1");

        let err = HardwareError::DeviceBusy("spk1".to_string());
        assert_eq!(format!("{}", err), "Device busy: spk1");

        let err = HardwareError::PermissionDenied("access denied".to_string());
        assert_eq!(format!("{}", err), "Permission denied: access denied");
    }

    #[test]
    fn test_hardware_error_error_trait() {
        use std::error::Error;
        let err = HardwareError::DeviceNotFound("test".to_string());
        assert!(err.source().is_none());
    }

    #[test]
    fn test_device_info_construction() {
        let info = DeviceInfo {
            id: "dev1".to_string(),
            name: "Test Device".to_string(),
            device_type: DeviceType::Camera,
            status: DeviceStatus::Available,
        };
        assert_eq!(info.id, "dev1");
        assert_eq!(info.name, "Test Device");
        assert_eq!(info.device_type, DeviceType::Camera);
        assert_eq!(info.status, DeviceStatus::Available);
    }
}
