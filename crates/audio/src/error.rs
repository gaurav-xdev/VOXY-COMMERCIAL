#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("Device not found: {0}")]
    DeviceNotFound(String),
    #[error("Device busy: {0}")]
    DeviceBusy(String),
    #[error("Stream error: {0}")]
    StreamError(String),
    #[error("Buffer overflow")]
    BufferOverflow,
    #[error("Buffer underflow")]
    BufferUnderflow,
    #[error("Unsupported sample rate: {0}")]
    UnsupportedSampleRate(u32),
    #[error("Unsupported channel count: {0}")]
    UnsupportedChannels(u8),
    #[error("DSP error: {0}")]
    DspError(String),
    #[error("Platform error: {0}")]
    PlatformError(String),
    #[error("Not initialized")]
    NotInitialized,
    #[error("Already initialized")]
    AlreadyInitialized,
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error(transparent)]
    Hardware(#[from] voxy_hardware::HardwareError),
}

pub type Result<T> = std::result::Result<T, AudioError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_device_not_found() {
        let err = AudioError::DeviceNotFound("mic1".to_string());
        assert_eq!(format!("{}", err), "Device not found: mic1");
    }

    #[test]
    fn test_error_display_device_busy() {
        let err = AudioError::DeviceBusy("spk1".to_string());
        assert_eq!(format!("{}", err), "Device busy: spk1");
    }

    #[test]
    fn test_error_display_buffer_overflow() {
        let err = AudioError::BufferOverflow;
        assert_eq!(format!("{}", err), "Buffer overflow");
    }

    #[test]
    fn test_error_display_buffer_underflow() {
        let err = AudioError::BufferUnderflow;
        assert_eq!(format!("{}", err), "Buffer underflow");
    }

    #[test]
    fn test_error_display_unsupported_sample_rate() {
        let err = AudioError::UnsupportedSampleRate(96000);
        assert_eq!(format!("{}", err), "Unsupported sample rate: 96000");
    }

    #[test]
    fn test_error_display_unsupported_channels() {
        let err = AudioError::UnsupportedChannels(6);
        assert_eq!(format!("{}", err), "Unsupported channel count: 6");
    }

    #[test]
    fn test_error_display_dsp_error() {
        let err = AudioError::DspError("overflow".to_string());
        assert_eq!(format!("{}", err), "DSP error: overflow");
    }

    #[test]
    fn test_error_display_platform_error() {
        let err = AudioError::PlatformError("not supported".to_string());
        assert_eq!(format!("{}", err), "Platform error: not supported");
    }

    #[test]
    fn test_error_display_not_initialized() {
        let err = AudioError::NotInitialized;
        assert_eq!(format!("{}", err), "Not initialized");
    }

    #[test]
    fn test_error_display_already_initialized() {
        let err = AudioError::AlreadyInitialized;
        assert_eq!(format!("{}", err), "Already initialized");
    }

    #[test]
    fn test_error_display_config_error() {
        let err = AudioError::ConfigError("bad config".to_string());
        assert_eq!(format!("{}", err), "Configuration error: bad config");
    }

    #[test]
    fn test_error_display_stream_error() {
        let err = AudioError::StreamError("broken pipe".to_string());
        assert_eq!(format!("{}", err), "Stream error: broken pipe");
    }

    #[test]
    fn test_error_error_trait() {
        use std::error::Error;
        let err = AudioError::DeviceNotFound("test".to_string());
        assert!(err.source().is_none());
    }

    #[test]
    fn test_error_from_hardware() {
        let hw_err = voxy_hardware::HardwareError::DeviceNotFound("mic".to_string());
        let err: AudioError = hw_err.into();
        assert_eq!(format!("{}", err), "Device not found: mic");
    }

    #[test]
    fn test_result_type() {
        let ok: Result<i32> = Ok(42);
        assert!(ok.is_ok());
    }
}
