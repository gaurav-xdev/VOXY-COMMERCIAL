//! Stress tests for Windows production integration modules.
//!
//! Tests rapid state transitions, concurrent access, and extreme conditions.

use std::sync::Arc;
use std::time::Duration;

use crate::bluetooth::{
    BluetoothDeviceInfo, BluetoothManager, BluetoothProfile, BluetoothStrategyConfig,
    InMemoryBluetoothManager,
};
use crate::hot_swap::{HotSwapConfig, HotSwapManager, NoopHotSwapHandler, PipelineState};
use crate::streaming_stt::{MockStreamingSttProvider, StreamingSttClient, StreamingSttConfig};
use crate::wasapi_improvements::WasapiHealthMonitor;
use crate::wasapi_session::{
    AudioSessionInfo, AudioSessionManager, AudioSessionState, InMemorySessionManager,
    WasapiSessionConfig,
};

fn bt_device(
    name: &str,
    profile: BluetoothProfile,
    quality: u32,
    latency: u32,
) -> BluetoothDeviceInfo {
    BluetoothDeviceInfo {
        name: name.to_string(),
        address: format!("AA:BB:{}", name.len()),
        profile,
        codec: None,
        battery_level: Some(80),
        signal_strength: Some(80),
        quality_score: quality,
        latency_ms: latency,
        is_active: false,
        last_seen: std::time::Instant::now(),
    }
}

fn audio_session(id: &str, name: &str, volume: f32, excluded: bool) -> AudioSessionInfo {
    AudioSessionInfo {
        id: id.to_string(),
        display_name: name.to_string(),
        volume,
        is_muted: false,
        process_id: 1000,
        is_voxy: false,
        is_excluded: excluded,
        state: AudioSessionState::Active,
    }
}

/// Drain an mpsc receiver on a background task until the channel closes.
///
/// `HotSwapManager`/`InMemorySessionManager` emit an event on a bounded
/// (capacity 64) channel for every transition and block the caller when the
/// buffer fills. Stress tests that pump hundreds of events must consume them
/// concurrently or they will deadlock on `send().await`.
///
/// The caller drops the producer (the manager) after the burst so the channel
/// closes, which lets this task finish; await the returned handle for clean
/// shutdown.
fn spawn_event_drainer<T: Send + 'static>(
    mut rx: tokio::sync::mpsc::Receiver<T>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move { while rx.recv().await.is_some() {} })
}

// ============================================================================
// Streaming STT Stress Tests
// ============================================================================

#[tokio::test]
async fn stress_streaming_stt_rapid_connect_disconnect() {
    let config = StreamingSttConfig {
        max_reconnect_attempts: 5,
        reconnect_delay: Duration::from_millis(10),
        ..Default::default()
    };

    for i in 0..50 {
        let mock = MockStreamingSttProvider::new();
        let client = StreamingSttClient::new(mock, config.clone());

        let result = client.connect(16000).await;
        assert!(result.is_ok(), "Connect failed at iteration {}", i);

        client.disconnect().await.ok();
    }
}

#[tokio::test]
async fn stress_streaming_stt_rapid_events() {
    let mock = MockStreamingSttProvider::new();
    let client = StreamingSttClient::new(mock, StreamingSttConfig::default());
    client.connect(16000).await.unwrap();

    for _ in 0..100 {
        let _ = client.send_audio(&vec![0.0f32; 4800], 16000).await;
        tokio::time::sleep(Duration::from_micros(100)).await;
    }
}

// ============================================================================
// WASAPI Session Stress Tests
// ============================================================================

#[tokio::test]
async fn stress_wasapi_ducking_rapid_toggle() {
    let mgr = InMemorySessionManager::new();
    let config = WasapiSessionConfig::default();
    mgr.initialize(&config).await.unwrap();

    // Consume ducking events concurrently; otherwise the bounded channel
    // blocks the sender after 64 toggles.
    let drainer = mgr.take_events().await.map(spawn_event_drainer);

    mgr.add_session(audio_session("1", "Spotify", 1.0, false));
    mgr.add_session(audio_session("2", "Discord", 0.8, true));

    for _ in 0..500 {
        mgr.start_ducking().await.unwrap();
        mgr.stop_ducking().await.unwrap();
    }

    let vol = mgr.get_volume("1").await.unwrap();
    assert!((vol - 1.0).abs() < 0.01);

    // Drop the manager to close the channel, then wait for the drainer.
    drop(mgr);
    if let Some(drainer) = drainer {
        drainer.await.unwrap();
    }
}

#[tokio::test]
async fn stress_wasapi_volume_flood() {
    let mgr = InMemorySessionManager::new();
    let config = WasapiSessionConfig::default();
    mgr.initialize(&config).await.unwrap();

    mgr.add_session(audio_session("1", "Music", 0.5, false));

    for i in 0..50 {
        let vol = (i as f32) / 100.0;
        mgr.set_volume("1", vol).await.unwrap();
    }

    let vol = mgr.get_volume("1").await.unwrap();
    assert!((0.0..=1.0).contains(&vol));
}

// ============================================================================
// Hot-Swap Stress Tests
// ============================================================================

#[tokio::test]
async fn stress_hot_swap_rapid_state_transitions() {
    let manager = HotSwapManager::new();
    manager.initialize(HotSwapConfig::default());

    for _ in 0..1000 {
        manager.set_state(PipelineState::Paused);
        manager.set_state(PipelineState::Recovering);
        manager.set_state(PipelineState::Running);
        manager.set_state(PipelineState::Sleeping);
        manager.set_state(PipelineState::Running);
    }

    assert_eq!(manager.state(), PipelineState::Running);
}

#[tokio::test]
async fn stress_hot_swap_concurrent_process() {
    let manager = HotSwapManager::new();
    manager.initialize(HotSwapConfig::default());

    // Consume hot-swap events concurrently; otherwise the bounded channel
    // blocks the sender after 64 events.
    let drainer = manager.take_events().map(spawn_event_drainer);

    for i in 0..10 {
        for j in 0..50 {
            let event = crate::device_watcher::DeviceChangeEvent::DeviceConnected {
                id: format!("device-{}-{}", i, j),
                name: format!("Device {}-{}", i, j),
            };
            let handler: Arc<dyn crate::hot_swap::HotSwapHandler> = Arc::new(NoopHotSwapHandler);
            manager.process_event(&handler, event).await;
        }
    }

    drop(manager);
    if let Some(drainer) = drainer {
        drainer.await.unwrap();
    }
}

#[tokio::test]
async fn stress_hot_swap_device_churn() {
    let manager = HotSwapManager::new();
    manager.initialize(HotSwapConfig::default());
    let handler: Arc<dyn crate::hot_swap::HotSwapHandler> = Arc::new(NoopHotSwapHandler);

    // Consume hot-swap events concurrently; otherwise the bounded channel
    // blocks the sender after 64 events.
    let drainer = manager.take_events().map(spawn_event_drainer);

    for i in 0..100 {
        let event = crate::device_watcher::DeviceChangeEvent::DeviceConnected {
            id: format!("device-{}", i),
            name: format!("Device {}", i),
        };
        manager.process_event(&handler, event).await;
        let event2 = crate::device_watcher::DeviceChangeEvent::DeviceDisconnected {
            id: format!("device-{}", i),
            name: format!("Device {}", i),
        };
        manager.process_event(&handler, event2).await;
    }

    drop(manager);
    if let Some(drainer) = drainer {
        drainer.await.unwrap();
    }
}

// ============================================================================
// Bluetooth Stress Tests
// ============================================================================

#[tokio::test]
async fn stress_bluetooth_rapid_scan() {
    let mgr = InMemoryBluetoothManager::new();
    let config = BluetoothStrategyConfig::default();
    mgr.initialize(&config).await.unwrap();

    mgr.add_device(bt_device(
        "Headset1",
        BluetoothProfile::HfpHandsFree,
        80,
        100,
    ));
    mgr.add_device(bt_device("Headset2", BluetoothProfile::LeAudio, 90, 30));

    for _ in 0..500 {
        let devices = mgr.scan_devices().await.unwrap();
        assert_eq!(devices.len(), 2);
    }
}

#[tokio::test]
async fn stress_bluetooth_recommendation_flood() {
    let mgr = InMemoryBluetoothManager::new();
    let config = BluetoothStrategyConfig::default();
    mgr.initialize(&config).await.unwrap();

    mgr.add_device(bt_device("Voice", BluetoothProfile::HfpHandsFree, 85, 100));
    mgr.add_device(bt_device("Music", BluetoothProfile::A2dp, 90, 150));

    for _ in 0..500 {
        let voice = mgr.recommend_voice_device().await;
        assert!(voice.is_some());
        let music = mgr.recommend_music_device().await;
        assert!(music.is_some());
    }
}

#[tokio::test]
async fn stress_bluetooth_device_churn() {
    let mgr = InMemoryBluetoothManager::new();
    let config = BluetoothStrategyConfig::default();
    mgr.initialize(&config).await.unwrap();

    for i in 0..100 {
        // Add device, scan, then clear all by creating new manager state
        mgr.add_device(bt_device(
            &format!("Device{}", i),
            BluetoothProfile::HfpHandsFree,
            80,
            100,
        ));
        let devices = mgr.scan_devices().await.unwrap();
        assert!(!devices.is_empty());
    }
}

// ============================================================================
// WASAPI Health Monitor Stress Tests
// ============================================================================

#[tokio::test]
async fn stress_wasapi_health_rapid_periods() {
    let monitor = WasapiHealthMonitor::new(48000, 10);

    for _ in 0..1000 {
        monitor.record_period();
    }

    let metrics = monitor.metrics();
    // Drift value should not be NaN (i64 is always finite, just verify no panic)
    let _ = metrics.clock_drift.drift_us;
}

#[tokio::test]
async fn stress_wasapi_health_fill_flood() {
    let monitor = WasapiHealthMonitor::new(48000, 10);

    let mut underruns = 0;
    for i in 0..1000 {
        let fill = if i % 3 == 0 { 0.05 } else { 0.9 };
        if monitor.check_fill(fill) {
            underruns += 1;
        }
    }

    assert!(underruns > 0);
    let metrics = monitor.metrics();
    assert_eq!(metrics.underruns.underrun_count, underruns as u64);
}

#[tokio::test]
async fn stress_wasapi_health_reset_flood() {
    let monitor = WasapiHealthMonitor::new(48000, 10);

    for _ in 0..100 {
        monitor.record_period();
        monitor.check_fill(0.05);
        monitor.reset();
    }

    let metrics = monitor.metrics();
    assert_eq!(metrics.clock_drift.drift_us, 0);
    assert_eq!(metrics.underruns.underrun_count, 0);
}

// ============================================================================
// Combined Stress Test
// ============================================================================

#[tokio::test]
async fn stress_all_modules_concurrent() {
    // Hot-swap and health monitor are Send + Sync
    let hot_swap = Arc::new(HotSwapManager::new());
    hot_swap.initialize(HotSwapConfig::default());

    let health = Arc::new(WasapiHealthMonitor::new(48000, 10));

    let mut handles = Vec::new();

    // Hot-swap state stress
    {
        let hs = hot_swap.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                hs.set_state(PipelineState::Paused);
                hs.set_state(PipelineState::Recovering);
                hs.set_state(PipelineState::Running);
            }
        }));
    }

    // Health monitor stress
    {
        let hm = health.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                hm.record_period();
                hm.check_fill(0.5);
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(hot_swap.state(), PipelineState::Running);
}
