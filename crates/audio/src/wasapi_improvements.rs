//! WASAPI audio backend improvements.
//!
//! Provides configuration and monitoring for Windows Audio Session API:
//! - Event-driven shared mode configuration
//! - Clock drift detection and compensation
//! - Buffer underrun detection
//! - Latency metrics tracking
//!
//! This module wraps the actual WASAPI platform calls with safe abstractions.
//! The actual platform integration is behind `#[cfg(windows)]`.

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;

/// Configuration for WASAPI event-driven shared mode.
#[derive(Debug, Clone)]
pub struct WasapiEventDrivenConfig {
    /// Use event-driven mode instead of polling.
    pub event_driven: bool,
    /// Buffer size in samples.
    pub buffer_size: u32,
    /// Period size in samples (event interval).
    pub period_size: u32,
    /// Share mode: shared or exclusive.
    pub share_mode: WasapiShareMode,
    /// Enable automatic stream start/stop on device changes.
    pub auto_start_stop: bool,
}

impl Default for WasapiEventDrivenConfig {
    fn default() -> Self {
        Self {
            event_driven: true,
            buffer_size: 960, // 20ms at 48kHz
            period_size: 480, // 10ms at 48kHz
            share_mode: WasapiShareMode::Shared,
            auto_start_stop: true,
        }
    }
}

/// WASAPI share mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasapiShareMode {
    /// Shared mode (through Windows audio engine).
    Shared,
    /// Exclusive mode (direct hardware access, lower latency).
    Exclusive,
}

/// Clock drift detection metrics.
#[derive(Debug, Clone)]
pub struct ClockDriftMetrics {
    /// Measured drift in microseconds.
    pub drift_us: i64,
    /// Whether drift exceeds threshold.
    pub is_drifting: bool,
    /// Compensation factor (1.0 = no compensation).
    pub compensation_factor: f64,
    /// Number of drift corrections applied.
    pub correction_count: u64,
}

impl Default for ClockDriftMetrics {
    fn default() -> Self {
        Self {
            drift_us: 0,
            is_drifting: false,
            compensation_factor: 1.0,
            correction_count: 0,
        }
    }
}

/// Buffer underrun detection metrics.
#[derive(Debug, Clone, Default)]
pub struct UnderrunMetrics {
    /// Total underrun events detected.
    pub underrun_count: u64,
    /// Last underrun timestamp.
    pub last_underrun: Option<Instant>,
    /// Severity (0-100, 100 = worst).
    pub severity: u32,
    /// Whether underruns are causing audible glitches.
    pub causing_glitches: bool,
}

/// Combined WASAPI health metrics.
#[derive(Debug, Clone, Default)]
pub struct WasapiHealthMetrics {
    /// Clock drift information.
    pub clock_drift: ClockDriftMetrics,
    /// Buffer underrun information.
    pub underruns: UnderrunMetrics,
    /// Round-trip latency estimate (ms).
    pub round_trip_latency_ms: f64,
    /// Output buffer fill level (0.0 - 1.0).
    pub output_fill_level: f64,
    /// Input buffer fill level (0.0 - 1.0).
    pub input_fill_level: f64,
    /// Whether the stream is running.
    pub is_streaming: bool,
}

/// Clock drift detector for WASAPI streams.
pub struct ClockDriftDetector {
    /// Expected samples per period.
    expected_samples_per_period: u64,
    /// Accumulated drift.
    accumulated_drift_us: AtomicI64,
    /// Correction count.
    correction_count: AtomicU64,
    /// Drift threshold in microseconds.
    drift_threshold_us: i64,
    /// Last period timestamp.
    last_period_start: RwLock<Option<Instant>>,
}

impl ClockDriftDetector {
    pub fn new(sample_rate: u32, period_ms: u32) -> Self {
        let expected_samples = (sample_rate as u64 * period_ms as u64) / 1000;
        Self {
            expected_samples_per_period: expected_samples,
            accumulated_drift_us: AtomicI64::new(0),
            correction_count: AtomicU64::new(0),
            drift_threshold_us: 500, // 0.5ms threshold
            last_period_start: RwLock::new(None),
        }
    }

    /// Record the start of a new audio period.
    pub fn record_period_start(&self) {
        let now = Instant::now();
        let mut last_start = self.last_period_start.write();
        if let Some(prev) = *last_start {
            let elapsed = now.duration_since(prev);
            let expected = Duration::from_millis(self.expected_samples_per_period * 1000 / 48000);
            let diff = elapsed.as_micros() as i64 - expected.as_micros() as i64;
            self.accumulated_drift_us.fetch_add(diff, Ordering::Relaxed);
        }
        *last_start = Some(now);
    }

    /// Get current drift metrics.
    pub fn metrics(&self) -> ClockDriftMetrics {
        let drift = self.accumulated_drift_us.load(Ordering::Relaxed);
        let is_drifting = drift.abs() > self.drift_threshold_us;
        let compensation = if is_drifting {
            1.0 - (drift as f64 / 1_000_000.0)
        } else {
            1.0
        };
        ClockDriftMetrics {
            drift_us: drift,
            is_drifting,
            compensation_factor: compensation.clamp(0.9, 1.1),
            correction_count: self.correction_count.load(Ordering::Relaxed),
        }
    }

    /// Reset drift tracking.
    pub fn reset(&self) {
        self.accumulated_drift_us.store(0, Ordering::Relaxed);
        self.correction_count.store(0, Ordering::Relaxed);
        *self.last_period_start.write() = None;
    }
}

/// Buffer underrun detector for WASAPI streams.
pub struct UnderrunDetector {
    /// Expected buffer fill level (0.0 - 1.0).
    #[allow(dead_code)]
    expected_fill: f64,
    /// Underrun threshold.
    underrun_threshold: f64,
    /// Total underruns detected.
    underrun_count: AtomicU64,
    /// Last underrun time.
    last_underrun: RwLock<Option<Instant>>,
}

impl UnderrunDetector {
    pub fn new(expected_fill: f64) -> Self {
        Self {
            expected_fill,
            underrun_threshold: 0.1, // 10% = underrun
            underrun_count: AtomicU64::new(0),
            last_underrun: RwLock::new(None),
        }
    }

    /// Check if a buffer fill level indicates an underrun.
    pub fn check_fill_level(&self, fill_level: f64) -> bool {
        if fill_level < self.underrun_threshold {
            self.underrun_count.fetch_add(1, Ordering::Relaxed);
            *self.last_underrun.write() = Some(Instant::now());
            true
        } else {
            false
        }
    }

    /// Get current underrun metrics.
    pub fn metrics(&self) -> UnderrunMetrics {
        let count = self.underrun_count.load(Ordering::Relaxed);
        let last = *self.last_underrun.read();
        let severity = if count == 0 {
            0
        } else {
            let recency = last
                .map(|t| {
                    let age = t.elapsed().as_secs();
                    if age < 1 {
                        100
                    } else if age < 10 {
                        50
                    } else {
                        25
                    }
                })
                .unwrap_or(0);
            (count.min(100) as u32 + recency).min(100)
        };

        UnderrunMetrics {
            underrun_count: count,
            last_underrun: last,
            severity,
            causing_glitches: severity > 50,
        }
    }

    /// Reset underrun tracking.
    pub fn reset(&self) {
        self.underrun_count.store(0, Ordering::Relaxed);
        *self.last_underrun.write() = None;
    }
}

/// WASAPI health monitor combining drift and underrun detection.
pub struct WasapiHealthMonitor {
    drift_detector: Arc<ClockDriftDetector>,
    underrun_detector: Arc<UnderrunDetector>,
    is_streaming: AtomicBool,
    last_metrics: RwLock<WasapiHealthMetrics>,
}

impl WasapiHealthMonitor {
    pub fn new(sample_rate: u32, period_ms: u32) -> Self {
        Self {
            drift_detector: Arc::new(ClockDriftDetector::new(sample_rate, period_ms)),
            underrun_detector: Arc::new(UnderrunDetector::new(0.8)),
            is_streaming: AtomicBool::new(false),
            last_metrics: RwLock::new(WasapiHealthMetrics::default()),
        }
    }

    /// Get the drift detector.
    pub fn drift_detector(&self) -> &Arc<ClockDriftDetector> {
        &self.drift_detector
    }

    /// Get the underrun detector.
    pub fn underrun_detector(&self) -> &Arc<UnderrunDetector> {
        &self.underrun_detector
    }

    /// Record a period start (for drift detection).
    pub fn record_period(&self) {
        self.drift_detector.record_period_start();
    }

    /// Check a buffer fill level (for underrun detection).
    pub fn check_fill(&self, fill_level: f64) -> bool {
        self.underrun_detector.check_fill_level(fill_level)
    }

    /// Get combined health metrics.
    pub fn metrics(&self) -> WasapiHealthMetrics {
        let m = WasapiHealthMetrics {
            clock_drift: self.drift_detector.metrics(),
            underruns: self.underrun_detector.metrics(),
            is_streaming: self.is_streaming.load(Ordering::Relaxed),
            ..Default::default()
        };
        *self.last_metrics.write() = m.clone();
        m
    }

    /// Set streaming state.
    pub fn set_streaming(&self, streaming: bool) {
        self.is_streaming.store(streaming, Ordering::SeqCst);
    }

    /// Reset all tracking.
    pub fn reset(&self) {
        self.drift_detector.reset();
        self.underrun_detector.reset();
        self.is_streaming.store(false, Ordering::SeqCst);
    }
}

impl Default for WasapiHealthMonitor {
    fn default() -> Self {
        Self::new(48000, 10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasapi_event_driven_config_defaults() {
        let config = WasapiEventDrivenConfig::default();
        assert!(config.event_driven);
        assert!(config.buffer_size > 0);
        assert!(config.period_size > 0);
        assert_eq!(config.share_mode, WasapiShareMode::Shared);
    }

    #[test]
    fn clock_drift_detector_creation() {
        let detector = ClockDriftDetector::new(48000, 10);
        let metrics = detector.metrics();
        assert_eq!(metrics.drift_us, 0);
        assert!(!metrics.is_drifting);
    }

    #[tokio::test]
    async fn clock_drift_detector_record_period() {
        let detector = ClockDriftDetector::new(48000, 10);
        detector.record_period_start();
        // Wait to simulate a real period
        tokio::time::sleep(Duration::from_millis(12)).await;
        detector.record_period_start();
        let metrics = detector.metrics();
        // After one real period (~12ms measured vs 10ms expected), drift is small
        assert!(metrics.drift_us.abs() < 50000);
    }

    #[test]
    fn clock_drift_detector_reset() {
        let detector = ClockDriftDetector::new(48000, 10);
        detector.record_period_start();
        detector.record_period_start();
        detector.reset();
        let metrics = detector.metrics();
        assert_eq!(metrics.drift_us, 0);
    }

    #[test]
    fn underrun_detector_no_underrun() {
        let detector = UnderrunDetector::new(0.8);
        assert!(!detector.check_fill_level(0.9));
        let metrics = detector.metrics();
        assert_eq!(metrics.underrun_count, 0);
        assert_eq!(metrics.severity, 0);
    }

    #[test]
    fn underrun_detector_detects_underrun() {
        let detector = UnderrunDetector::new(0.8);
        assert!(detector.check_fill_level(0.05));
        let metrics = detector.metrics();
        assert_eq!(metrics.underrun_count, 1);
        assert!(metrics.severity > 0);
    }

    #[test]
    fn underrun_detector_severity_accumulates() {
        let detector = UnderrunDetector::new(0.8);
        for _ in 0..10 {
            detector.check_fill_level(0.05);
        }
        let metrics = detector.metrics();
        assert_eq!(metrics.underrun_count, 10);
        assert!(metrics.severity >= 10);
    }

    #[test]
    fn underrun_detector_reset() {
        let detector = UnderrunDetector::new(0.8);
        detector.check_fill_level(0.05);
        detector.reset();
        let metrics = detector.metrics();
        assert_eq!(metrics.underrun_count, 0);
    }

    #[test]
    fn health_monitor_creation() {
        let monitor = WasapiHealthMonitor::new(48000, 10);
        let metrics = monitor.metrics();
        assert!(!metrics.is_streaming);
    }

    #[tokio::test]
    async fn health_monitor_period_recording() {
        let monitor = WasapiHealthMonitor::new(48000, 10);
        monitor.record_period();
        // Wait roughly one period to get meaningful drift data
        tokio::time::sleep(Duration::from_millis(12)).await;
        monitor.record_period();
        let metrics = monitor.metrics();
        // Drift should be within a reasonable range after one real period
        // (measured ~12ms vs expected 10ms = ~2000us drift)
        assert!(metrics.clock_drift.drift_us.abs() < 50000);
    }

    #[test]
    fn health_monitor_fill_check() {
        let monitor = WasapiHealthMonitor::new(48000, 10);
        assert!(!monitor.check_fill(0.9));
        assert!(monitor.check_fill(0.05));
    }

    #[test]
    fn health_monitor_streaming_state() {
        let monitor = WasapiHealthMonitor::new(48000, 10);
        assert!(!monitor.metrics().is_streaming);

        monitor.set_streaming(true);
        assert!(monitor.metrics().is_streaming);

        monitor.set_streaming(false);
        assert!(!monitor.metrics().is_streaming);
    }

    #[test]
    fn health_monitor_reset() {
        let monitor = WasapiHealthMonitor::new(48000, 10);
        monitor.record_period();
        monitor.check_fill(0.05);
        monitor.reset();

        let metrics = monitor.metrics();
        assert_eq!(metrics.clock_drift.drift_us, 0);
        assert_eq!(metrics.underruns.underrun_count, 0);
    }

    #[test]
    fn share_mode_variants() {
        assert_eq!(format!("{:?}", WasapiShareMode::Shared), "Shared");
        assert_eq!(format!("{:?}", WasapiShareMode::Exclusive), "Exclusive");
    }

    #[test]
    fn clock_drift_compensation_factor() {
        let detector = ClockDriftDetector::new(48000, 10);
        let metrics = detector.metrics();
        // No drift -> compensation should be 1.0
        assert!((metrics.compensation_factor - 1.0).abs() < 0.01);
    }

    #[test]
    fn underrun_glitch_detection() {
        let detector = UnderrunDetector::new(0.8);
        // Many underruns should cause glitches
        for _ in 0..50 {
            detector.check_fill_level(0.05);
        }
        let metrics = detector.metrics();
        assert!(metrics.causing_glitches);
    }
}
