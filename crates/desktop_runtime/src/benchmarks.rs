//! Performance benchmarks for desktop runtime.
//!
//! Run with: cargo test -p voxy-desktop-runtime --lib -- --nocapture benchmarks

#[cfg(test)]
mod benchmarks {
    use crate::settings::SettingsManager;
    use crate::{DesktopEventBridge, DesktopRuntime, PushToTalk, RuntimeConfig};
    use std::sync::Arc;
    use std::time::Instant;
    use voxy_event_bus::EventBus;

    #[test]
    fn bench_runtime_init() {
        let iterations = 1000;
        let start = Instant::now();
        for _ in 0..iterations {
            let config = RuntimeConfig::new("VOXY");
            let _rt = DesktopRuntime::new(config).unwrap();
        }
        let elapsed = start.elapsed();
        let per_init = elapsed / iterations;
        println!(
            "[BENCH] Runtime init: {} iterations in {:?} ({:?}/init)",
            iterations, elapsed, per_init
        );
    }

    #[test]
    fn bench_tray_creation() {
        use crate::tray::TrayIcon;
        let iterations = 1000;
        let start = Instant::now();
        for _ in 0..iterations {
            let _tray = TrayIcon::new("VOXY", "VOXY AI Assistant").unwrap();
        }
        let elapsed = start.elapsed();
        let per_iter = elapsed / iterations;
        println!(
            "[BENCH] Tray creation: {} iterations in {:?} ({:?}/iter)",
            iterations, elapsed, per_iter
        );
    }

    #[test]
    fn bench_notification_send() {
        use crate::notifications::NotificationManager;
        let mgr = NotificationManager::new().unwrap();
        let iterations = 1000;
        let start = Instant::now();
        for i in 0..iterations {
            let notif = crate::notifications::Notification::info(
                &format!("Title {}", i),
                &format!("Message {}", i),
            );
            let _ = mgr.send(notif);
        }
        let elapsed = start.elapsed();
        let per_iter = elapsed / iterations;
        println!(
            "[BENCH] Notification send: {} iterations in {:?} ({:?}/iter)",
            iterations, elapsed, per_iter
        );
    }

    #[test]
    fn bench_shortcut_register() {
        use crate::shortcuts::{Modifiers, ShortcutManager};
        let iterations = 1000;
        let start = Instant::now();
        for i in 0..iterations {
            let mut mgr = ShortcutManager::new().unwrap();
            let _ = mgr.register(&format!("test_{}", i), "Test", Modifiers::ctrl(), 0x41);
        }
        let elapsed = start.elapsed();
        let per_iter = elapsed / iterations;
        println!(
            "[BENCH] Shortcut register: {} iterations in {:?} ({:?}/iter)",
            iterations, elapsed, per_iter
        );
    }

    #[test]
    fn bench_settings_reload() {
        let mgr = SettingsManager::new().unwrap();
        let iterations = 1000;
        let start = Instant::now();
        for _ in 0..iterations {
            let _snapshot = mgr.get();
        }
        let elapsed = start.elapsed();
        let per_iter = elapsed / iterations;
        println!(
            "[BENCH] Settings read: {} iterations in {:?} ({:?}/iter)",
            iterations, elapsed, per_iter
        );
    }

    #[tokio::test]
    async fn bench_eventbus_publish() {
        let bus = Arc::new(EventBus::new(1000));
        let bridge = DesktopEventBridge::new(bus);
        let iterations = 1000;
        let start = Instant::now();
        for _ in 0..iterations {
            bridge.publish_tray_action("test", None).await.unwrap();
        }
        let elapsed = start.elapsed();
        let per_iter = elapsed / iterations;
        println!(
            "[BENCH] EventBus publish: {} iterations in {:?} ({:?}/iter)",
            iterations, elapsed, per_iter
        );
    }

    #[tokio::test]
    async fn bench_push_to_talk() {
        let bus = Arc::new(EventBus::new(100));
        let bridge = Arc::new(DesktopEventBridge::new(bus));
        let ptt = PushToTalk::new(bridge, "push_to_talk");
        let iterations = 1000;
        let start = Instant::now();
        for _ in 0..iterations {
            ptt.on_pressed().await.unwrap();
            ptt.on_pressed().await.unwrap();
        }
        let elapsed = start.elapsed();
        let per_iter = elapsed / iterations;
        println!(
            "[BENCH] Push-to-talk toggle: {} iterations in {:?} ({:?}/iter)",
            iterations, elapsed, per_iter
        );
    }

    #[test]
    fn bench_download_manager_creation() {
        use crate::download::DownloadManager;
        let iterations = 1000;
        let start = Instant::now();
        for _ in 0..iterations {
            let _mgr = DownloadManager::new(3, None).unwrap();
        }
        let elapsed = start.elapsed();
        let per_iter = elapsed / iterations;
        println!(
            "[BENCH] DownloadManager creation: {} iterations in {:?} ({:?}/iter)",
            iterations, elapsed, per_iter
        );
    }

    #[test]
    fn bench_settings_validate() {
        let mgr = SettingsManager::new().unwrap();
        let iterations = 1000;
        let start = Instant::now();
        for _ in 0..iterations {
            let snapshot = mgr.get();
            let _ = mgr.validate(&snapshot);
        }
        let elapsed = start.elapsed();
        let per_iter = elapsed / iterations;
        println!(
            "[BENCH] Settings validate: {} iterations in {:?} ({:?}/iter)",
            iterations, elapsed, per_iter
        );
    }

    #[test]
    #[ignore = "manual benchmark requiring isolated desktop clipboard access"]
    fn bench_clipboard_read() {
        use crate::clipboard::ClipboardManager;
        let cb = ClipboardManager::new();
        let iterations = 100;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = cb.read_text();
        }
        let elapsed = start.elapsed();
        let per_iter = elapsed / iterations;
        println!(
            "[BENCH] Clipboard read: {} iterations in {:?} ({:?}/iter)",
            iterations, elapsed, per_iter
        );
    }
}
