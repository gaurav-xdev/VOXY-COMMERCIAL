//! Windows low-latency audio capture and playback via WASAPI / cpal.
//!
//! Provides:
//! - Device enumeration and default device resolution
//! - Format negotiation and automatic downsampling to 16 kHz mono float (STT/VAD standard)
//! - Low-latency capture with real-time RMS energy telemetry
//! - High-speed playback queue with immediate barge-in purge capability (`clear_playback`)
//! - Thread-safe bounded ring buffers

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;

pub mod conditioning;
pub use conditioning::{AudioConditioner, ConditioningConfig};

pub mod beamforming;
pub use beamforming::{
    BeamformerConfig, ChannelTopology, DirectionOfArrival, MicrophoneArrayProcessor,
};

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("Audio device not found: {0}")]
    DeviceNotFound(String),
    #[error("Unsupported sample rate: {0}")]
    UnsupportedSampleRate(u32),
    #[error("Buffer underflow")]
    BufferUnderflow,
    #[error("Device busy: {0}")]
    DeviceBusy(String),
    #[error("Failed to build input stream: {0}")]
    InputStreamError(String),
    #[error("Failed to build output stream: {0}")]
    OutputStreamError(String),
    #[error("CPAL host error: {0}")]
    HostError(String),
    #[error("Stream play error: {0}")]
    StreamPlayError(String),
}

/// Standard audio configuration for the OSMOO voice pipeline.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioConfig {
    /// Sample rate emitted by capture (normalized to 16kHz for STT/VAD).
    pub target_sample_rate: u32,
    /// Number of channels emitted (normalized to 1 mono channel).
    pub target_channels: u16,
    /// Chunk duration in milliseconds for VAD analysis (default 20ms = 320 samples @ 16kHz).
    pub chunk_duration_ms: u32,
    /// Preferred input device name (None for Windows default).
    pub input_device_name: Option<String>,
    /// Preferred output device name (None for Windows default).
    pub output_device_name: Option<String>,
    /// Audio conditioning (high-pass, noise gate, pre-emphasis, limiter).
    pub conditioning: ConditioningConfig,
    /// Advanced spatial beamforming and speech enhancement configuration.
    pub beamforming: BeamformerConfig,
    /// When false (default, production requirement), stream creation failure returns an explicit Error.
    /// When true (tests or explicit simulation), fallback to virtual handle is allowed.
    pub allow_virtual_fallback: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            target_sample_rate: 16000,
            target_channels: 1,
            chunk_duration_ms: 20,
            input_device_name: None,
            output_device_name: None,
            conditioning: ConditioningConfig::default(),
            beamforming: BeamformerConfig::default(),
            allow_virtual_fallback: false,
        }
    }
}

/// Calculate Root-Mean-Square (RMS) audio energy in range [0.0, 1.0].
pub fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt().clamp(0.0, 1.0)
}

/// Downsample and downmix interleaved audio samples to 16kHz mono.
pub fn normalize_to_mono_16k(
    input_samples: &[f32],
    src_sample_rate: u32,
    src_channels: u16,
    target_sample_rate: u32,
) -> Vec<f32> {
    if input_samples.is_empty() {
        return Vec::new();
    }

    // 1. Downmix to mono
    let mono: Vec<f32> = if src_channels > 1 {
        let ch = src_channels as usize;
        input_samples
            .chunks_exact(ch)
            .map(|frame| frame.iter().sum::<f32>() / ch as f32)
            .collect()
    } else {
        input_samples.to_vec()
    };

    // 2. Resample if necessary
    if src_sample_rate == target_sample_rate {
        return mono;
    }

    resample_mono(&mono, src_sample_rate, target_sample_rate)
}

/// Resample single-channel mono buffer to target sample rate with linear interpolation.
pub fn resample_mono(samples: &[f32], src_rate: u32, target_rate: u32) -> Vec<f32> {
    if samples.is_empty() || src_rate == target_rate {
        return samples.to_vec();
    }

    let ratio = target_rate as f64 / src_rate as f64;
    let out_len = ((samples.len() as f64) * ratio).round() as usize;
    let mut output = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_idx = (i as f64) / ratio;
        let idx0 = src_idx.floor() as usize;
        let frac = (src_idx - idx0 as f64) as f32;

        if idx0 + 1 < samples.len() {
            let s0 = samples[idx0];
            let s1 = samples[idx0 + 1];
            output.push(s0 + frac * (s1 - s0));
        } else if idx0 < samples.len() {
            output.push(samples[idx0]);
        }
    }

    output
}

/// Process multichannel audio with adaptive spatial beamforming (if channels >= 2)
/// or single-channel spectral enhancement & dereverberation (if mono channel).
pub fn process_multichannel_audio_with_beamforming(
    input_samples: &[f32],
    src_sample_rate: u32,
    src_channels: u16,
    target_sample_rate: u32,
    processor: &mut MicrophoneArrayProcessor,
) -> Vec<f32> {
    if input_samples.is_empty() {
        return Vec::new();
    }

    let enhanced_mono = processor.process_to_mono(input_samples, src_channels);

    if src_sample_rate == target_sample_rate {
        enhanced_mono
    } else {
        resample_mono(&enhanced_mono, src_sample_rate, target_sample_rate)
    }
}

/// Thread-safe bounded ring buffer supporting instant purge on barge-in.
#[derive(Clone)]
pub struct AudioRingBuffer {
    data: Arc<Mutex<Vec<f32>>>,
    max_capacity: usize,
}

impl AudioRingBuffer {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            data: Arc::new(Mutex::new(Vec::with_capacity(max_capacity))),
            max_capacity,
        }
    }

    pub fn push(&self, samples: &[f32]) {
        let mut lock = self.data.lock();
        let current_len = lock.len();
        let to_add = samples.len();

        if current_len + to_add > self.max_capacity {
            let overflow = (current_len + to_add) - self.max_capacity;
            let drain_count = overflow.min(current_len);
            lock.drain(0..drain_count);
        }
        lock.extend_from_slice(samples);
    }

    pub fn pop(&self, count: usize) -> Vec<f32> {
        let mut lock = self.data.lock();
        let take = count.min(lock.len());
        lock.drain(0..take).collect()
    }

    pub fn take_all(&self) -> Vec<f32> {
        let mut lock = self.data.lock();
        std::mem::take(&mut *lock)
    }

    /// Instantaneous purge used for barge-in cancellation when user speaks.
    pub fn clear(&self) {
        let mut lock = self.data.lock();
        lock.clear();
    }

    pub fn len(&self) -> usize {
        self.data.lock().len()
    }

    pub fn available_samples(&self) -> usize {
        self.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.lock().is_empty()
    }
}

/// Device manager for querying and selecting audio endpoints on Windows.
pub struct AudioDeviceManager {
    host: cpal::Host,
}

impl AudioDeviceManager {
    pub fn new() -> Result<Self, AudioError> {
        let host = cpal::default_host();
        Ok(Self { host })
    }

    pub fn list_input_devices(&self) -> Vec<String> {
        self.host
            .input_devices()
            .map(|devices| {
                devices
                    .map(|d| {
                        d.description()
                            .map(|desc| desc.name().to_string())
                            .unwrap_or_else(|_| d.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn list_output_devices(&self) -> Vec<String> {
        self.host
            .output_devices()
            .map(|devices| {
                devices
                    .map(|d| {
                        d.description()
                            .map(|desc| desc.name().to_string())
                            .unwrap_or_else(|_| d.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn default_input_device_name(&self) -> Option<String> {
        self.host.default_input_device().map(|d| {
            d.description()
                .map(|desc| desc.name().to_string())
                .unwrap_or_else(|_| d.to_string())
        })
    }

    pub fn default_output_device_name(&self) -> Option<String> {
        self.host.default_output_device().map(|d| {
            d.description()
                .map(|desc| desc.name().to_string())
                .unwrap_or_else(|_| d.to_string())
        })
    }
}

/// Active audio capture handle that delivers normalized 16kHz mono audio frames.
pub struct AudioCaptureHandle {
    _stream: Option<cpal::Stream>,
    is_active: Arc<AtomicBool>,
    is_muted: Arc<AtomicBool>,
    is_speaker_playing: Arc<AtomicBool>,
    last_error: Arc<Mutex<Option<String>>>,
}

impl AudioCaptureHandle {
    /// Create a virtual handle for tests or simulation without opening physical hardware.
    pub fn new_virtual(is_active: bool, is_muted: bool, is_speaker_playing: bool) -> Self {
        Self {
            _stream: None,
            is_active: Arc::new(AtomicBool::new(is_active)),
            is_muted: Arc::new(AtomicBool::new(is_muted)),
            is_speaker_playing: Arc::new(AtomicBool::new(is_speaker_playing)),
            last_error: Arc::new(Mutex::new(None)),
        }
    }

    /// True if capture is running. Returns false if no physical stream exists unless virtual handle is active.
    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::SeqCst) && self._stream.is_some()
    }

    /// Explicit check for whether this handle wraps a real hardware stream.
    pub fn has_physical_stream(&self) -> bool {
        self._stream.is_some()
    }

    /// Check if the stream has experienced an unexpected hardware disconnect/dropout.
    pub fn has_dropped_out(&self) -> bool {
        self._stream.is_some()
            && !self.is_active.load(Ordering::SeqCst)
            && self.last_error.lock().is_some()
    }

    /// Last recorded hardware or OS audio error message, if any.
    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().clone()
    }

    pub fn set_muted(&self, muted: bool) {
        self.is_muted.store(muted, Ordering::SeqCst);
    }

    pub fn is_muted(&self) -> bool {
        self.is_muted.load(Ordering::SeqCst)
    }

    pub fn set_speaker_playing(&self, playing: bool) {
        self.is_speaker_playing.store(playing, Ordering::SeqCst);
    }

    pub fn is_speaker_playing(&self) -> bool {
        self.is_speaker_playing.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.is_active.store(false, Ordering::SeqCst);
    }
}

/// Active audio playback handle supporting real-time streaming and instant barge-in cancellation.
pub struct AudioPlaybackHandle {
    _stream: Option<cpal::Stream>,
    buffer: AudioRingBuffer,
    is_playing: Arc<AtomicBool>,
    /// Output device sample rate — callers must resample queued audio to it.
    sample_rate: u32,
    last_error: Arc<Mutex<Option<String>>>,
}

impl AudioPlaybackHandle {
    /// Create a virtual handle for tests or simulation without opening physical hardware.
    pub fn new_virtual(sample_rate: u32) -> Self {
        Self {
            _stream: None,
            buffer: AudioRingBuffer::new(sample_rate as usize * 10),
            is_playing: Arc::new(AtomicBool::new(false)),
            sample_rate,
            last_error: Arc::new(Mutex::new(None)),
        }
    }

    /// Explicit check for whether this handle wraps a real hardware stream.
    pub fn has_physical_stream(&self) -> bool {
        self._stream.is_some()
    }

    /// Check if the stream has experienced an unexpected hardware disconnect/dropout.
    pub fn has_dropped_out(&self) -> bool {
        self._stream.is_some() && !self.is_playing.load(Ordering::SeqCst) && !self.buffer.is_empty()
    }

    /// Last recorded playback device error, if any.
    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().clone()
    }

    pub fn queue_samples(&self, samples: &[f32]) {
        self.buffer.push(samples);
        self.is_playing.store(true, Ordering::SeqCst);
    }

    /// Output device sample rate (Hz). Queued samples must already be at this rate:
    /// the playback loop pops raw counts without resampling.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Instant barge-in stop: cleans out all pending audio in flight immediately.
    pub fn clear_playback(&self) {
        self.buffer.clear();
        self.is_playing.store(false, Ordering::SeqCst);
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::SeqCst) && !self.buffer.is_empty()
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }
}

/// Unified audio runtime providing capture and playback streams.
pub struct AudioRuntime {
    config: AudioConfig,
    manager: AudioDeviceManager,
}

impl AudioRuntime {
    pub fn new(config: AudioConfig) -> Result<Self, AudioError> {
        let manager = AudioDeviceManager::new()?;
        Ok(Self { config, manager })
    }

    /// Starts microphone capture and streams normalized 16kHz mono frames into `frame_tx`.
    pub fn start_capture<F>(&self, on_frame: F) -> Result<AudioCaptureHandle, AudioError>
    where
        F: Fn(Vec<f32>, f32) + Send + Sync + 'static,
    {
        let device = match &self.config.input_device_name {
            Some(name) => self
                .manager
                .host
                .input_devices()
                .map_err(|e| AudioError::HostError(e.to_string()))?
                .find(|d| {
                    d.description()
                        .map(|desc| desc.name() == name)
                        .unwrap_or(false)
                        || d.to_string() == *name
                })
                .ok_or_else(|| AudioError::DeviceNotFound(name.clone()))?,
            None => self
                .manager
                .host
                .default_input_device()
                .ok_or_else(|| AudioError::DeviceNotFound("Default input device".into()))?,
        };

        let supported_config = device
            .default_input_config()
            .map_err(|e| AudioError::InputStreamError(e.to_string()))?;

        let sample_rate = supported_config.sample_rate();
        let channels = supported_config.channels();
        let target_sr = self.config.target_sample_rate;

        let is_active = Arc::new(AtomicBool::new(true));
        let is_muted = Arc::new(AtomicBool::new(false));
        let is_speaker_playing = Arc::new(AtomicBool::new(false));
        let last_error = Arc::new(Mutex::new(None));

        let active_clone = is_active.clone();
        let muted_clone = is_muted.clone();
        let speaker_clone_f32 = is_speaker_playing.clone();
        let speaker_clone_i16 = is_speaker_playing.clone();
        let speaker_clone_u16 = is_speaker_playing.clone();
        let speaker_clone_i32 = is_speaker_playing.clone();
        let on_frame = Arc::new(on_frame);

        let mut conditioner_f32 = AudioConditioner::new(self.config.conditioning.clone());
        let mut conditioner_i16 = AudioConditioner::new(self.config.conditioning.clone());
        let mut conditioner_u16 = AudioConditioner::new(self.config.conditioning.clone());
        let mut conditioner_i32 = AudioConditioner::new(self.config.conditioning.clone());

        let beam_cfg = BeamformerConfig {
            sample_rate,
            ..self.config.beamforming.clone()
        };
        let mut mic_processor_f32 = MicrophoneArrayProcessor::new(channels, beam_cfg.clone());
        let mut mic_processor_i16 = MicrophoneArrayProcessor::new(channels, beam_cfg.clone());
        let mut mic_processor_u16 = MicrophoneArrayProcessor::new(channels, beam_cfg.clone());
        let mut mic_processor_i32 = MicrophoneArrayProcessor::new(channels, beam_cfg);

        let stream_config: cpal::StreamConfig = supported_config.into();
        let stream_result = match supported_config.sample_format() {
            cpal::SampleFormat::F32 => {
                let active_err = active_clone.clone();
                let last_err = last_error.clone();
                device.build_input_stream(
                    stream_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if !active_clone.load(Ordering::SeqCst)
                            || muted_clone.load(Ordering::SeqCst)
                        {
                            return;
                        }
                        let mut mono_16k = if channels > 1 {
                            process_multichannel_audio_with_beamforming(
                                data,
                                sample_rate,
                                channels,
                                target_sr,
                                &mut mic_processor_f32,
                            )
                        } else {
                            normalize_to_mono_16k(data, sample_rate, channels, target_sr)
                        };
                        conditioner_f32
                            .set_speaker_active(speaker_clone_f32.load(Ordering::SeqCst));
                        conditioner_f32.process(&mut mono_16k);
                        let rms = calculate_rms(&mono_16k);
                        on_frame(mono_16k, rms);
                    },
                    move |err| {
                        let err_str = err.to_string();
                        tracing::error!(
                            "Audio capture error (device disconnected/interrupted): {err_str}"
                        );
                        *last_err.lock() = Some(err_str);
                        active_err.store(false, Ordering::SeqCst);
                    },
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let active_err = active_clone.clone();
                let last_err = last_error.clone();
                device.build_input_stream(
                    stream_config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if !active_clone.load(Ordering::SeqCst)
                            || muted_clone.load(Ordering::SeqCst)
                        {
                            return;
                        }
                        let f32_data: Vec<f32> =
                            data.iter().map(|&s| (s as f32) / 32768.0).collect();
                        let mut mono_16k = if channels > 1 {
                            process_multichannel_audio_with_beamforming(
                                &f32_data,
                                sample_rate,
                                channels,
                                target_sr,
                                &mut mic_processor_i16,
                            )
                        } else {
                            normalize_to_mono_16k(&f32_data, sample_rate, channels, target_sr)
                        };
                        conditioner_i16
                            .set_speaker_active(speaker_clone_i16.load(Ordering::SeqCst));
                        conditioner_i16.process(&mut mono_16k);
                        let rms = calculate_rms(&mono_16k);
                        on_frame(mono_16k, rms);
                    },
                    move |err| {
                        let err_str = err.to_string();
                        tracing::error!(
                            "Audio capture error (device disconnected/interrupted): {err_str}"
                        );
                        *last_err.lock() = Some(err_str);
                        active_err.store(false, Ordering::SeqCst);
                    },
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let active_err = active_clone.clone();
                let last_err = last_error.clone();
                device.build_input_stream(
                    stream_config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        if !active_clone.load(Ordering::SeqCst)
                            || muted_clone.load(Ordering::SeqCst)
                        {
                            return;
                        }
                        let f32_data: Vec<f32> = data
                            .iter()
                            .map(|&s| ((s as f32) - 32768.0) / 32768.0)
                            .collect();
                        let mut mono_16k = if channels > 1 {
                            process_multichannel_audio_with_beamforming(
                                &f32_data,
                                sample_rate,
                                channels,
                                target_sr,
                                &mut mic_processor_u16,
                            )
                        } else {
                            normalize_to_mono_16k(&f32_data, sample_rate, channels, target_sr)
                        };
                        conditioner_u16
                            .set_speaker_active(speaker_clone_u16.load(Ordering::SeqCst));
                        conditioner_u16.process(&mut mono_16k);
                        let rms = calculate_rms(&mono_16k);
                        on_frame(mono_16k, rms);
                    },
                    move |err| {
                        let err_str = err.to_string();
                        tracing::error!(
                            "Audio capture error (device disconnected/interrupted): {err_str}"
                        );
                        *last_err.lock() = Some(err_str);
                        active_err.store(false, Ordering::SeqCst);
                    },
                    None,
                )
            }
            cpal::SampleFormat::I32 => {
                let active_err = active_clone.clone();
                let last_err = last_error.clone();
                device.build_input_stream(
                    stream_config,
                    move |data: &[i32], _: &cpal::InputCallbackInfo| {
                        if !active_clone.load(Ordering::SeqCst)
                            || muted_clone.load(Ordering::SeqCst)
                        {
                            return;
                        }
                        let f32_data: Vec<f32> =
                            data.iter().map(|&s| (s as f32) / 2147483648.0).collect();
                        let mut mono_16k = if channels > 1 {
                            process_multichannel_audio_with_beamforming(
                                &f32_data,
                                sample_rate,
                                channels,
                                target_sr,
                                &mut mic_processor_i32,
                            )
                        } else {
                            normalize_to_mono_16k(&f32_data, sample_rate, channels, target_sr)
                        };
                        conditioner_i32
                            .set_speaker_active(speaker_clone_i32.load(Ordering::SeqCst));
                        conditioner_i32.process(&mut mono_16k);
                        let rms = calculate_rms(&mono_16k);
                        on_frame(mono_16k, rms);
                    },
                    move |err| {
                        let err_str = err.to_string();
                        tracing::error!(
                            "Audio capture error (device disconnected/interrupted): {err_str}"
                        );
                        *last_err.lock() = Some(err_str);
                        active_err.store(false, Ordering::SeqCst);
                    },
                    None,
                )
            }
            _ => Err(cpal::ErrorKind::UnsupportedConfig.into()),
        };

        match stream_result {
            Ok(stream) => {
                stream
                    .play()
                    .map_err(|e| AudioError::StreamPlayError(e.to_string()))?;
                Ok(AudioCaptureHandle {
                    _stream: Some(stream),
                    is_active,
                    is_muted,
                    is_speaker_playing,
                    last_error,
                })
            }
            Err(e) => {
                if self.config.allow_virtual_fallback {
                    tracing::warn!("Failed to open physical audio capture stream: {e}. Falling back to virtual handle.");
                    Ok(AudioCaptureHandle {
                        _stream: None,
                        is_active,
                        is_muted,
                        is_speaker_playing,
                        last_error,
                    })
                } else {
                    tracing::error!("Failed to open physical audio capture stream: {e}");
                    Err(AudioError::InputStreamError(e.to_string()))
                }
            }
        }
    }

    /// Starts audio playback and returns a handle to queue chunks or immediately purge sound.
    pub fn start_playback(&self) -> Result<AudioPlaybackHandle, AudioError> {
        let device = match &self.config.output_device_name {
            Some(name) => self
                .manager
                .host
                .output_devices()
                .map_err(|e| AudioError::HostError(e.to_string()))?
                .find(|d| {
                    d.description()
                        .map(|desc| desc.name() == name)
                        .unwrap_or(false)
                        || d.to_string() == *name
                })
                .ok_or_else(|| AudioError::DeviceNotFound(name.clone()))?,
            None => self
                .manager
                .host
                .default_output_device()
                .ok_or_else(|| AudioError::DeviceNotFound("Default output device".into()))?,
        };

        let supported_config = device
            .default_output_config()
            .map_err(|e| AudioError::OutputStreamError(e.to_string()))?;

        let sample_rate = supported_config.sample_rate();
        let channels = supported_config.channels();

        let ring_buffer = AudioRingBuffer::new(sample_rate as usize * 10);
        let is_playing = Arc::new(AtomicBool::new(false));
        let last_error = Arc::new(Mutex::new(None));

        let buf_clone = ring_buffer.clone();
        let playing_clone = is_playing.clone();
        let last_err_clone = last_error.clone();

        let stream_config: cpal::StreamConfig = supported_config.into();
        let stream_result = match supported_config.sample_format() {
            cpal::SampleFormat::F32 => {
                let last_err_playback = last_err_clone;
                device.build_output_stream(
                    stream_config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let needed = data.len();
                        let samples = buf_clone.pop(needed / channels as usize);

                        if samples.is_empty() {
                            data.fill(0.0);
                            playing_clone.store(false, Ordering::SeqCst);
                            return;
                        }

                        // Interleave into channels
                        for (s_idx, frame) in data.chunks_mut(channels as usize).enumerate() {
                            let val = if s_idx < samples.len() {
                                samples[s_idx]
                            } else {
                                0.0
                            };
                            for sample in frame.iter_mut() {
                                *sample = val;
                            }
                        }
                    },
                    move |err| {
                        let err_str = err.to_string();
                        tracing::error!("Audio playback error: {err_str}");
                        *last_err_playback.lock() = Some(err_str);
                    },
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let last_err_playback = last_err_clone;
                device.build_output_stream(
                    stream_config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        let needed = data.len();
                        let samples = buf_clone.pop(needed / channels as usize);

                        if samples.is_empty() {
                            data.fill(0);
                            playing_clone.store(false, Ordering::SeqCst);
                            return;
                        }

                        // Interleave into channels with 16-bit PCM conversion
                        for (s_idx, frame) in data.chunks_mut(channels as usize).enumerate() {
                            let val = if s_idx < samples.len() {
                                samples[s_idx]
                            } else {
                                0.0
                            };
                            let sample_i16 = (val.clamp(-1.0, 1.0) * 32767.0) as i16;
                            for sample in frame.iter_mut() {
                                *sample = sample_i16;
                            }
                        }
                    },
                    move |err| {
                        let err_str = err.to_string();
                        tracing::error!("Audio playback error: {err_str}");
                        *last_err_playback.lock() = Some(err_str);
                    },
                    None,
                )
            }
            _ => Err(cpal::ErrorKind::UnsupportedConfig.into()),
        };

        match stream_result {
            Ok(stream) => {
                stream
                    .play()
                    .map_err(|e| AudioError::StreamPlayError(e.to_string()))?;
                Ok(AudioPlaybackHandle {
                    _stream: Some(stream),
                    buffer: ring_buffer,
                    is_playing,
                    sample_rate,
                    last_error,
                })
            }
            Err(e) => {
                if self.config.allow_virtual_fallback {
                    tracing::warn!("Failed to open physical audio playback stream: {e}. Falling back to virtual handle.");
                    Ok(AudioPlaybackHandle {
                        _stream: None,
                        buffer: ring_buffer,
                        is_playing,
                        sample_rate,
                        last_error,
                    })
                } else {
                    tracing::error!("Failed to open physical audio playback stream: {e}");
                    Err(AudioError::OutputStreamError(e.to_string()))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rms_calculation() {
        let silence = vec![0.0; 160];
        assert_eq!(calculate_rms(&silence), 0.0);

        let constant = vec![0.5; 160];
        assert!((calculate_rms(&constant) - 0.5).abs() < 1e-4);

        let clamped = vec![2.0; 160];
        assert_eq!(calculate_rms(&clamped), 1.0);
    }

    #[test]
    fn test_ring_buffer_push_pop_and_clear() {
        let ring = AudioRingBuffer::new(100);
        assert!(ring.is_empty());

        let samples = vec![0.1, 0.2, 0.3, 0.4];
        ring.push(&samples);
        assert_eq!(ring.len(), 4);

        let popped = ring.pop(2);
        assert_eq!(popped, vec![0.1, 0.2]);
        assert_eq!(ring.len(), 2);

        // Instant barge-in cancellation purge
        ring.clear();
        assert!(ring.is_empty());
        assert_eq!(ring.len(), 0);
    }

    #[test]
    fn test_mono_normalization_and_resampling() {
        // Stereo 48kHz -> Mono 16kHz (ratio 1/3)
        let stereo_48k = vec![0.2, 0.4, 0.2, 0.4, 0.2, 0.4]; // 3 frames
        let mono_16k = normalize_to_mono_16k(&stereo_48k, 48000, 2, 16000);
        assert_eq!(mono_16k.len(), 1);
        assert!((mono_16k[0] - 0.3).abs() < 1e-4);
    }

    #[test]
    fn test_audio_capture_handle_physical_stream_semantics() {
        let handle_virtual = AudioCaptureHandle {
            _stream: None,
            is_active: Arc::new(AtomicBool::new(true)),
            is_muted: Arc::new(AtomicBool::new(false)),
            is_speaker_playing: Arc::new(AtomicBool::new(false)),
            last_error: Arc::new(Mutex::new(None)),
        };
        // Virtual handle with None stream must report is_active = false and has_physical_stream = false
        assert!(!handle_virtual.is_active());
        assert!(!handle_virtual.has_physical_stream());
        assert!(!handle_virtual.has_dropped_out());
        assert_eq!(handle_virtual.last_error(), None);

        handle_virtual.set_muted(true);
        assert!(handle_virtual.is_muted());

        let playback_virtual = AudioPlaybackHandle::new_virtual(16000);
        assert!(!playback_virtual.has_physical_stream());
        assert_eq!(playback_virtual.sample_rate(), 16000);
        assert_eq!(playback_virtual.last_error(), None);
    }
}
