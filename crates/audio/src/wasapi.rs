use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use cpal::traits::HostTrait;
use cpal::traits::StreamTrait;
use parking_lot::Mutex;
use tracing::{info, warn};

use crate::buffer::SpscRingBuffer;
use crate::config::{AudioRuntimeConfig, AudioStreamConfig};
use crate::device::AudioDeviceInfo;
use crate::error::{AudioError, Result};
use crate::stream::{AudioInputStream, AudioOutputStream, AudioPacket, AudioPacketStream};

use crate::backend::AudioBackend;
use crate::device::AudioDeviceManager;

mod cpal_helpers {
    pub fn device_name(device: &cpal::Device) -> String {
        use cpal::traits::DeviceTrait;
        device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "Unknown".to_string())
    }

    pub fn supported_input_configs(device: &cpal::Device) -> Vec<cpal::SupportedStreamConfigRange> {
        use cpal::traits::DeviceTrait;
        device
            .supported_input_configs()
            .map(|c| c.collect())
            .unwrap_or_default()
    }

    pub fn supported_output_configs(
        device: &cpal::Device,
    ) -> Vec<cpal::SupportedStreamConfigRange> {
        use cpal::traits::DeviceTrait;
        device
            .supported_output_configs()
            .map(|c| c.collect())
            .unwrap_or_default()
    }

    pub fn build_input_stream(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        data_callback: impl FnMut(&[f32], &cpal::InputCallbackInfo) + Send + 'static,
        error_callback: impl FnMut(cpal::Error) + Send + 'static,
    ) -> std::result::Result<cpal::Stream, cpal::Error> {
        use cpal::traits::DeviceTrait;
        device.build_input_stream(config, data_callback, error_callback, None)
    }

    pub fn build_output_stream(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        data_callback: impl FnMut(&mut [f32], &cpal::OutputCallbackInfo) + Send + 'static,
        error_callback: impl FnMut(cpal::Error) + Send + 'static,
    ) -> std::result::Result<cpal::Stream, cpal::Error> {
        use cpal::traits::DeviceTrait;
        device.build_output_stream(config, data_callback, error_callback, None)
    }
}

fn negotiate_input_config(
    device: &cpal::Device,
    desired: &AudioStreamConfig,
) -> cpal::StreamConfig {
    use cpal::traits::DeviceTrait;
    let supported: Option<Vec<cpal::SupportedStreamConfigRange>> =
        device.supported_input_configs().ok().map(|c| c.collect());
    negotiate(supported, desired)
}

fn negotiate_output_config(
    device: &cpal::Device,
    desired: &AudioStreamConfig,
) -> cpal::StreamConfig {
    use cpal::traits::DeviceTrait;
    let supported: Option<Vec<cpal::SupportedStreamConfigRange>> =
        device.supported_output_configs().ok().map(|c| c.collect());
    negotiate(supported, desired)
}

fn negotiate(
    supported: Option<Vec<cpal::SupportedStreamConfigRange>>,
    desired: &AudioStreamConfig,
) -> cpal::StreamConfig {
    let desired_channels = desired.channels as u16;
    let desired_rate = desired.sample_rate;

    let configs = match supported {
        Some(c) if !c.is_empty() => c,
        _ => {
            warn!(
                "No supported configs queried; using desired {}Hz {}ch",
                desired_rate, desired_channels
            );
            return cpal::StreamConfig {
                channels: desired_channels,
                sample_rate: desired_rate,
                buffer_size: cpal::BufferSize::Default,
            };
        }
    };

    let channel_matches: Vec<_> = configs
        .iter()
        .filter(|c| c.channels() == desired_channels)
        .collect();

    let candidates: Vec<&cpal::SupportedStreamConfigRange> = if channel_matches.is_empty() {
        warn!(
            "Device does not support {}ch; falling back to first available config",
            desired_channels
        );
        configs.iter().collect()
    } else {
        channel_matches
    };

    for cfg in &candidates {
        if cfg.min_sample_rate() <= desired_rate && desired_rate <= cfg.max_sample_rate() {
            let negotiated_channels = cfg.channels();
            info!(
                "Negotiated config: {}Hz {}ch (exact sample rate match)",
                desired_rate, negotiated_channels
            );
            return cpal::StreamConfig {
                channels: negotiated_channels,
                sample_rate: desired_rate,
                buffer_size: cpal::BufferSize::Default,
            };
        }
    }

    let fallback = candidates.first().copied().unwrap_or(&configs[0]);
    let min_sr = fallback.min_sample_rate();
    let max_sr = fallback.max_sample_rate();
    let negotiated_rate = desired_rate.clamp(min_sr, max_sr);
    let negotiated_channels = fallback.channels();

    warn!(
        "Negotiated config: {}Hz {}ch (fallback from desired {}Hz {}ch)",
        negotiated_rate, negotiated_channels, desired_rate, desired_channels
    );

    cpal::StreamConfig {
        channels: negotiated_channels,
        sample_rate: negotiated_rate,
        buffer_size: cpal::BufferSize::Default,
    }
}

pub struct WasapiBackend {
    name: String,
    available: AtomicBool,
    input_cache: Arc<Mutex<Vec<AudioDeviceInfo>>>,
    output_cache: Arc<Mutex<Vec<AudioDeviceInfo>>>,
}

impl Default for WasapiBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl WasapiBackend {
    pub fn new() -> Self {
        Self {
            name: "wasapi".to_string(),
            available: AtomicBool::new(true),
            input_cache: Arc::new(Mutex::new(Vec::new())),
            output_cache: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn cpal_device_to_info(
        device: &cpal::Device,
        is_default: bool,
        is_input: bool,
    ) -> AudioDeviceInfo {
        let name = cpal_helpers::device_name(device);
        let device_type = if is_input {
            voxy_hardware::DeviceType::Microphone
        } else {
            voxy_hardware::DeviceType::Speaker
        };

        let mut supported_sample_rates = Vec::new();
        let mut supported_channels = Vec::new();

        let configs = if is_input {
            cpal_helpers::supported_input_configs(device)
        } else {
            cpal_helpers::supported_output_configs(device)
        };

        for config in &configs {
            let sr = config.min_sample_rate();
            if !supported_sample_rates.contains(&sr) {
                supported_sample_rates.push(sr);
            }
            let max_sr = config.max_sample_rate();
            if !supported_sample_rates.contains(&max_sr) {
                supported_sample_rates.push(max_sr);
            }
            let ch = config.channels() as u8;
            if !supported_channels.contains(&ch) {
                supported_channels.push(ch);
            }
        }

        supported_sample_rates.sort();
        supported_sample_rates.dedup();
        supported_channels.sort();
        supported_channels.dedup();

        if supported_sample_rates.is_empty() {
            supported_sample_rates = vec![8000, 16000, 22050, 44100, 48000];
        }
        if supported_channels.is_empty() {
            supported_channels = vec![1, 2];
        }

        let id = format!("cpal-{}-{}", if is_input { "in" } else { "out" }, name);

        AudioDeviceInfo {
            id,
            name,
            device_type,
            status: voxy_hardware::DeviceStatus::Available,
            supported_sample_rates,
            supported_channels,
            is_default,
        }
    }

    async fn refresh_cache(&self) {
        let host = cpal::default_host();

        let mut inputs = Vec::new();
        if let Some(default_input) = host.default_input_device() {
            let is_default = true;
            inputs.push(Self::cpal_device_to_info(&default_input, is_default, true));
        }

        if let Ok(input_devices) = host.input_devices() {
            for device in input_devices {
                let info = Self::cpal_device_to_info(&device, false, true);
                if !inputs.iter().any(|d| d.id == info.id) {
                    inputs.push(info);
                }
            }
        }
        *self.input_cache.lock() = inputs;

        let mut outputs = Vec::new();
        if let Some(default_output) = host.default_output_device() {
            let is_default = true;
            outputs.push(Self::cpal_device_to_info(
                &default_output,
                is_default,
                false,
            ));
        }

        if let Ok(output_devices) = host.output_devices() {
            for device in output_devices {
                let info = Self::cpal_device_to_info(&device, false, false);
                if !outputs.iter().any(|d| d.id == info.id) {
                    outputs.push(info);
                }
            }
        }
        *self.output_cache.lock() = outputs;
    }
}

#[async_trait]
impl AudioBackend for WasapiBackend {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_available(&self) -> bool {
        self.available.load(Ordering::Relaxed)
    }

    async fn enumerate_inputs(&self) -> Result<Vec<AudioDeviceInfo>> {
        if self.input_cache.lock().is_empty() {
            self.refresh_cache().await;
        }
        Ok(self.input_cache.lock().clone())
    }

    async fn enumerate_outputs(&self) -> Result<Vec<AudioDeviceInfo>> {
        if self.output_cache.lock().is_empty() {
            self.refresh_cache().await;
        }
        Ok(self.output_cache.lock().clone())
    }

    async fn default_input(&self) -> Result<AudioDeviceInfo> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| AudioError::DeviceNotFound("No default input device".into()))?;
        Ok(Self::cpal_device_to_info(&device, true, true))
    }

    async fn default_output(&self) -> Result<AudioDeviceInfo> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AudioError::DeviceNotFound("No default output device".into()))?;
        Ok(Self::cpal_device_to_info(&device, true, false))
    }

    async fn open_input(&self, config: &AudioStreamConfig) -> Result<Box<dyn AudioInputStream>> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| AudioError::DeviceNotFound("No input device".into()))?;

        let stream = CpalInputStream::new(device, config)?;
        Ok(Box::new(stream))
    }

    async fn open_output(&self, config: &AudioStreamConfig) -> Result<Box<dyn AudioOutputStream>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AudioError::DeviceNotFound("No output device".into()))?;

        let stream = CpalOutputStream::new(device, config)?;
        Ok(Box::new(stream))
    }
}

struct CpalInputStreamInner {
    _device: cpal::Device,
    stream: Mutex<Option<cpal::Stream>>,
    sample_rate: u32,
    channels: u8,
    ring: Arc<SpscRingBuffer>,
    open: AtomicBool,
    packet_seq: AtomicU64,
}

pub struct CpalInputStream {
    inner: Arc<CpalInputStreamInner>,
}

impl CpalInputStream {
    pub fn new(device: cpal::Device, config: &AudioStreamConfig) -> Result<Self> {
        let stream_config = negotiate_input_config(&device, config);
        let sample_rate = stream_config.sample_rate;
        let channels = stream_config.channels as u8;

        let ring = Arc::new(SpscRingBuffer::new(131072));
        let ring_cb = ring.clone();

        let stream = cpal_helpers::build_input_stream(
            &device,
            stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                ring_cb.write(data);
            },
            move |err| {
                warn!("Input stream error: {}", err);
            },
        )
        .map_err(|e| AudioError::PlatformError(format!("Failed to build input stream: {e}")))?;

        stream
            .play()
            .map_err(|e| AudioError::PlatformError(format!("Failed to start input stream: {e}")))?;

        info!("CpalInputStream opened: {sample_rate}Hz {channels}ch");

        Ok(Self {
            inner: Arc::new(CpalInputStreamInner {
                _device: device,
                stream: Mutex::new(Some(stream)),
                sample_rate,
                channels,
                ring,
                open: AtomicBool::new(true),
                packet_seq: AtomicU64::new(0),
            }),
        })
    }
}

#[async_trait]
impl AudioInputStream for CpalInputStream {
    async fn open(&mut self, _config: &AudioStreamConfig) -> Result<()> {
        self.inner.open.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        self.inner.open.store(false, Ordering::SeqCst);
        self.inner.ring.close();
        *self.inner.stream.lock() = None;
        Ok(())
    }

    async fn read(&mut self, frames: usize) -> Result<AudioPacket> {
        if !self.inner.open.load(Ordering::SeqCst) {
            return Err(AudioError::NotInitialized);
        }

        let samples_needed = frames * self.inner.channels as usize;
        let mut data = vec![0.0f32; samples_needed];
        let timeout = tokio::time::Duration::from_millis(100);
        let start = std::time::Instant::now();

        loop {
            let read = self.inner.ring.read(&mut data);
            if read >= samples_needed {
                data.truncate(read);
                let seq = self.inner.packet_seq.fetch_add(1, Ordering::Relaxed);
                let mut packet =
                    AudioPacket::new(data, self.inner.sample_rate, self.inner.channels);
                packet.sequence = seq;
                return Ok(packet);
            }

            if start.elapsed() > timeout {
                if read > 0 {
                    data.truncate(read);
                } else {
                    data.resize(samples_needed, 0.0);
                }
                let seq = self.inner.packet_seq.fetch_add(1, Ordering::Relaxed);
                let mut packet =
                    AudioPacket::new(data, self.inner.sample_rate, self.inner.channels);
                packet.sequence = seq;
                return Ok(packet);
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        }
    }

    async fn stream(&mut self) -> Box<dyn AudioPacketStream> {
        Box::new(CpalCaptureStream {
            inner: self.inner.clone(),
            stopped: false,
        })
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate
    }
    fn channels(&self) -> u8 {
        self.inner.channels
    }
    fn latency_ms(&self) -> f64 {
        10.0
    }
    fn is_open(&self) -> bool {
        self.inner.open.load(Ordering::SeqCst)
    }
    fn device_id(&self) -> Option<&str> {
        None
    }
}

struct CpalCaptureStream {
    inner: Arc<CpalInputStreamInner>,
    stopped: bool,
}

#[async_trait]
impl AudioPacketStream for CpalCaptureStream {
    async fn next(&mut self) -> Option<AudioPacket> {
        if self.stopped || !self.inner.open.load(Ordering::SeqCst) {
            return None;
        }

        let frames = self.inner.sample_rate as usize / 50;
        let samples_needed = frames * self.inner.channels as usize;

        loop {
            let mut data = vec![0.0f32; samples_needed];
            let read = self.inner.ring.read(&mut data);
            if read >= samples_needed {
                data.truncate(read);
                let seq = self.inner.packet_seq.fetch_add(1, Ordering::Relaxed);
                let mut packet =
                    AudioPacket::new(data, self.inner.sample_rate, self.inner.channels);
                packet.sequence = seq;
                return Some(packet);
            }

            if self.inner.ring.is_closed() {
                return None;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        }
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate
    }
    fn channels(&self) -> u8 {
        self.inner.channels
    }
    fn is_complete(&self) -> bool {
        self.stopped
    }
}

struct CpalOutputStreamInner {
    _device: cpal::Device,
    stream: Mutex<Option<cpal::Stream>>,
    sample_rate: u32,
    channels: u8,
    ring: Arc<SpscRingBuffer>,
    open: AtomicBool,
    packet_seq: AtomicU64,
}

pub struct CpalOutputStream {
    inner: Arc<CpalOutputStreamInner>,
}

impl CpalOutputStream {
    pub fn new(device: cpal::Device, config: &AudioStreamConfig) -> Result<Self> {
        let stream_config = negotiate_output_config(&device, config);
        let ring = Arc::new(SpscRingBuffer::new(262144));

        // Try negotiated config first, fall back to device default if it fails
        let (stream, sample_rate, channels) = match Self::try_build(&device, &stream_config, &ring)
        {
            Ok(s) => (s, stream_config.sample_rate, stream_config.channels as u8),
            Err(e) => {
                warn!(
                    "Negotiated output config {}Hz {}ch rejected by WASAPI; falling back to device default: {}",
                    stream_config.sample_rate, stream_config.channels, e
                );
                use cpal::traits::DeviceTrait;
                let default_config = device.default_output_config().map_err(|e| {
                    AudioError::PlatformError(format!("Failed to get default output config: {e}"))
                })?;
                let fallback_config: cpal::StreamConfig = default_config.into();
                let sr = fallback_config.sample_rate;
                let ch = fallback_config.channels;
                let s = Self::try_build(&device, &fallback_config, &ring)?;
                (s, sr, ch as u8)
            }
        };

        stream.play().map_err(|e| {
            AudioError::PlatformError(format!("Failed to start output stream: {e}"))
        })?;

        info!("CpalOutputStream opened: {sample_rate}Hz {channels}ch");

        Ok(Self {
            inner: Arc::new(CpalOutputStreamInner {
                _device: device,
                stream: Mutex::new(Some(stream)),
                sample_rate,
                channels,
                ring,
                open: AtomicBool::new(true),
                packet_seq: AtomicU64::new(0),
            }),
        })
    }

    fn try_build(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        ring: &Arc<SpscRingBuffer>,
    ) -> Result<cpal::Stream> {
        let ring_cb = ring.clone();
        cpal_helpers::build_output_stream(
            device,
            *config,
            move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let read = ring_cb.read(output);
                if read < output.len() {
                    output[read..].fill(0.0);
                }
            },
            move |err| {
                warn!("Output stream error: {}", err);
            },
        )
        .map_err(|e| AudioError::PlatformError(format!("Failed to build output stream: {e}")))
    }
}

#[async_trait]
impl AudioOutputStream for CpalOutputStream {
    async fn open(&mut self, _config: &AudioStreamConfig) -> Result<()> {
        self.inner.open.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        self.inner.open.store(false, Ordering::SeqCst);
        self.inner.ring.close();
        *self.inner.stream.lock() = None;
        Ok(())
    }

    async fn write(&mut self, packet: &AudioPacket) -> Result<()> {
        if !self.inner.open.load(Ordering::SeqCst) {
            return Err(AudioError::NotInitialized);
        }

        let _seq = self.inner.packet_seq.fetch_add(1, Ordering::Relaxed);

        let mut remaining = &packet.data[..];
        let timeout = tokio::time::Duration::from_millis(200);
        let start = std::time::Instant::now();

        while !remaining.is_empty() {
            let written = self.inner.ring.write(remaining);
            remaining = &remaining[written..];

            if !remaining.is_empty() {
                if start.elapsed() > timeout {
                    warn!("Output write timeout, dropping {} samples", remaining.len());
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
            }
        }

        Ok(())
    }

    async fn play(&mut self, mut stream: Box<dyn AudioPacketStream>) -> Result<()> {
        if !self.inner.open.load(Ordering::SeqCst) {
            return Err(AudioError::NotInitialized);
        }

        while let Some(chunk) = stream.next().await {
            if !self.inner.open.load(Ordering::SeqCst) {
                break;
            }
            self.write(&chunk).await?;
        }

        Ok(())
    }

    async fn flush(&mut self) -> Result<()> {
        while !self.inner.ring.is_empty() && self.inner.open.load(Ordering::SeqCst) {
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        }
        Ok(())
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate
    }
    fn channels(&self) -> u8 {
        self.inner.channels
    }
    fn latency_ms(&self) -> f64 {
        10.0
    }
    fn is_open(&self) -> bool {
        self.inner.open.load(Ordering::SeqCst)
    }
    fn device_id(&self) -> Option<&str> {
        None
    }
}

pub struct WasapiDeviceManager {
    inner: Arc<WasapiBackend>,
    initialized: AtomicBool,
    #[allow(dead_code)]
    config: Mutex<Option<AudioRuntimeConfig>>,
}

impl Default for WasapiDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WasapiDeviceManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(WasapiBackend::new()),
            initialized: AtomicBool::new(false),
            config: Mutex::new(None),
        }
    }
}

#[async_trait]
impl AudioDeviceManager for WasapiDeviceManager {
    async fn initialize(&self, config: &AudioRuntimeConfig) -> Result<()> {
        if self.initialized.load(Ordering::SeqCst) {
            return Err(AudioError::AlreadyInitialized);
        }
        self.inner.refresh_cache().await;
        self.initialized.store(true, Ordering::SeqCst);
        *self.config.lock() = Some(config.clone());
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        if !self.initialized.load(Ordering::SeqCst) {
            return Err(AudioError::NotInitialized);
        }
        self.initialized.store(false, Ordering::SeqCst);
        *self.config.lock() = None;
        Ok(())
    }

    async fn list_inputs(&self) -> Result<Vec<AudioDeviceInfo>> {
        self.inner.enumerate_inputs().await
    }

    async fn list_outputs(&self) -> Result<Vec<AudioDeviceInfo>> {
        self.inner.enumerate_outputs().await
    }

    async fn default_input(&self) -> Result<AudioDeviceInfo> {
        self.inner.default_input().await
    }

    async fn default_output(&self) -> Result<AudioDeviceInfo> {
        self.inner.default_output().await
    }

    async fn open_input(&self, config: &AudioStreamConfig) -> Result<Box<dyn AudioInputStream>> {
        self.inner.open_input(config).await
    }

    async fn open_output(&self, config: &AudioStreamConfig) -> Result<Box<dyn AudioOutputStream>> {
        self.inner.open_output(config).await
    }

    async fn get_device(&self, id: &str) -> Result<AudioDeviceInfo> {
        let inputs = self.list_inputs().await?;
        for d in &inputs {
            if d.id == id {
                return Ok(d.clone());
            }
        }
        let outputs = self.list_outputs().await?;
        for d in &outputs {
            if d.id == id {
                return Ok(d.clone());
            }
        }
        Err(AudioError::DeviceNotFound(id.to_string()))
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }
}
