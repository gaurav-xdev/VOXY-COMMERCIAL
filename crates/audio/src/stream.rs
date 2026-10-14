use chrono::Utc;

use crate::config::AudioStreamConfig;
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct AudioPacket {
    pub data: Vec<f32>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub sequence: u64,
    pub channels: u8,
    pub sample_rate: u32,
    pub duration_ms: f64,
    pub is_silence: bool,
    pub peak_level: f32,
    pub rms_level: f32,
}

impl AudioPacket {
    pub fn new(data: Vec<f32>, sample_rate: u32, channels: u8) -> Self {
        let len = data.len();
        let frame_count = if channels > 0 {
            len / channels as usize
        } else {
            0
        };
        let duration_ms = if sample_rate > 0 {
            (frame_count as f64 / sample_rate as f64) * 1000.0
        } else {
            0.0
        };
        let (peak_level, rms_level) = compute_levels(&data);

        Self {
            timestamp: Utc::now(),
            sequence: 0,
            channels,
            sample_rate,
            duration_ms,
            is_silence: peak_level < 1e-6,
            peak_level,
            rms_level,
            data,
        }
    }

    pub fn silence(duration_frames: usize, sample_rate: u32, channels: u8) -> Self {
        let total_samples = duration_frames * channels as usize;
        let data = vec![0.0; total_samples];
        let duration_ms = if sample_rate > 0 {
            (duration_frames as f64 / sample_rate as f64) * 1000.0
        } else {
            0.0
        };

        Self {
            data,
            timestamp: Utc::now(),
            sequence: 0,
            channels,
            sample_rate,
            duration_ms,
            is_silence: true,
            peak_level: 0.0,
            rms_level: 0.0,
        }
    }

    pub fn duration_ms(&self) -> f64 {
        self.duration_ms
    }

    pub fn is_silent(&self, threshold: f32) -> bool {
        self.peak_level < threshold
    }
}

fn compute_levels(data: &[f32]) -> (f32, f32) {
    if data.is_empty() {
        return (0.0, 0.0);
    }

    let mut peak = 0.0f32;
    let mut sum_sq = 0.0f64;
    for &s in data {
        let abs = s.abs();
        if abs > peak {
            peak = abs;
        }
        sum_sq += (s as f64) * (s as f64);
    }
    let rms = (sum_sq / data.len() as f64).sqrt() as f32;

    (peak, rms)
}

#[async_trait::async_trait]
pub trait AudioPacketStream: Send + Sync {
    async fn next(&mut self) -> Option<AudioPacket>;
    fn sample_rate(&self) -> u32;
    fn channels(&self) -> u8;
    fn is_complete(&self) -> bool;
}

#[async_trait::async_trait]
pub trait AudioInputStream: Send + Sync {
    async fn open(&mut self, config: &AudioStreamConfig) -> Result<()>;
    async fn close(&mut self) -> Result<()>;
    async fn read(&mut self, frames: usize) -> Result<AudioPacket>;
    async fn stream(&mut self) -> Box<dyn AudioPacketStream>;
    fn sample_rate(&self) -> u32;
    fn channels(&self) -> u8;
    fn latency_ms(&self) -> f64;
    fn is_open(&self) -> bool;
    fn device_id(&self) -> Option<&str>;
}

#[async_trait::async_trait]
pub trait AudioOutputStream: Send + Sync {
    async fn open(&mut self, config: &AudioStreamConfig) -> Result<()>;
    async fn close(&mut self) -> Result<()>;
    async fn write(&mut self, packet: &AudioPacket) -> Result<()>;
    async fn play(&mut self, stream: Box<dyn AudioPacketStream>) -> Result<()>;
    async fn flush(&mut self) -> Result<()>;
    /// Immediately clear and drain any buffered pending audio samples.
    async fn clear(&mut self) -> Result<()> {
        Ok(())
    }
    fn sample_rate(&self) -> u32;
    fn channels(&self) -> u8;
    fn latency_ms(&self) -> f64;
    fn is_open(&self) -> bool;
    fn device_id(&self) -> Option<&str>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_packet_new() {
        let data = vec![0.1, 0.2, -0.3, 0.4];
        let packet = AudioPacket::new(data, 16000, 1);
        assert_eq!(packet.sample_rate, 16000);
        assert_eq!(packet.channels, 1);
        assert!(packet.duration_ms > 0.0);
        assert!(packet.peak_level > 0.0);
        assert!(packet.rms_level > 0.0);
        assert!(!packet.is_silence);
    }

    #[test]
    fn test_audio_packet_silence() {
        let packet = AudioPacket::silence(1600, 16000, 1);
        assert_eq!(packet.sample_rate, 16000);
        assert_eq!(packet.channels, 1);
        assert!(packet.duration_ms > 0.0);
        assert!(packet.is_silence);
        assert_eq!(packet.peak_level, 0.0);
        assert_eq!(packet.rms_level, 0.0);
    }

    #[test]
    fn test_audio_packet_duration_ms() {
        let data = vec![0.0; 16000];
        let packet = AudioPacket::new(data, 16000, 1);
        let dur = packet.duration_ms();
        assert!((dur - 1000.0).abs() < 0.001);
    }

    #[test]
    fn test_audio_packet_is_silent() {
        let packet = AudioPacket::silence(1600, 16000, 1);
        assert!(packet.is_silent(0.01));

        let data = vec![0.5; 160];
        let packet = AudioPacket::new(data, 16000, 1);
        assert!(!packet.is_silent(0.01));
    }

    #[test]
    fn test_audio_packet_stereo() {
        let data = vec![0.1, -0.2, 0.3, -0.4];
        let packet = AudioPacket::new(data, 44100, 2);
        assert_eq!(packet.channels, 2);
        assert_eq!(packet.sample_rate, 44100);
        assert!(packet.duration_ms > 0.0);
    }

    #[test]
    fn test_audio_packet_empty_data() {
        let packet = AudioPacket::new(vec![], 16000, 1);
        assert_eq!(packet.peak_level, 0.0);
        assert_eq!(packet.rms_level, 0.0);
        assert_eq!(packet.duration_ms, 0.0);
    }

    #[test]
    fn test_compute_levels_peak() {
        let data = vec![0.0, 0.5, 0.0, -0.8, 0.0];
        let (peak, _rms) = compute_levels(&data);
        assert!((peak - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_compute_levels_rms() {
        let data = vec![0.5, 0.5, 0.5, 0.5];
        let (_peak, rms) = compute_levels(&data);
        assert!((rms - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_compute_levels_empty() {
        let (peak, rms) = compute_levels(&[]);
        assert_eq!(peak, 0.0);
        assert_eq!(rms, 0.0);
    }
}
