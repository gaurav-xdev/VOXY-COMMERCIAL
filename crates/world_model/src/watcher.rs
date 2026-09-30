use crate::activity::ActivityClassifier;
use crate::config::WorldModelConfig;
use crate::desktop::{ApplicationInfo, DesktopState, WindowInfo};
use crate::error::Result;
use crate::event::WorldModelEvent;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, info, warn};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND};
#[cfg(target_os = "windows")]
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
};

/// Watches desktop state changes by polling the foreground window.
pub struct DesktopWatcher {
    config: WorldModelConfig,
    state: Arc<RwLock<DesktopWatcherState>>,
    event_tx: mpsc::Sender<WorldModelEvent>,
    event_rx: Arc<RwLock<mpsc::Receiver<WorldModelEvent>>>,
    classifier: Arc<ActivityClassifier>,
    idle_threshold_secs: u64,
    last_active_time: Arc<RwLock<Instant>>,
    shutdown_tx: broadcast::Sender<()>,
}

struct DesktopWatcherState {
    last_active_window: Option<String>,
    last_focused_app: Option<String>,
    last_window_title: Option<String>,
    known_windows: HashMap<String, WindowInfo>,
    known_apps: HashMap<String, ApplicationInfo>,
    is_idle: bool,
    activity_type: Option<String>,
}

use std::time::Instant;

const WATCHER_CHANNEL_CAPACITY: usize = 256;

impl DesktopWatcher {
    pub fn new(config: WorldModelConfig) -> Self {
        let (event_tx, event_rx) = mpsc::channel(WATCHER_CHANNEL_CAPACITY);
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            config,
            state: Arc::new(RwLock::new(DesktopWatcherState {
                last_active_window: None,
                last_focused_app: None,
                last_window_title: None,
                known_windows: HashMap::new(),
                known_apps: HashMap::new(),
                is_idle: false,
                activity_type: None,
            })),
            event_tx,
            event_rx: Arc::new(RwLock::new(event_rx)),
            classifier: Arc::new(ActivityClassifier::new()),
            idle_threshold_secs: 300,
            last_active_time: Arc::new(RwLock::new(Instant::now())),
            shutdown_tx,
        }
    }

    pub fn with_idle_threshold(mut self, secs: u64) -> Self {
        self.idle_threshold_secs = secs;
        self
    }

    pub async fn start(&self) -> Result<()> {
        let state = self.state.clone();
        let event_tx = self.event_tx.clone();
        let interval_ms = self.config.desktop_poll_interval_ms;
        let classifier = self.classifier.clone();
        let _idle_threshold = self.idle_threshold_secs;
        let last_active_time = self.last_active_time.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        info!(interval_ms = interval_ms, "Starting desktop watcher");

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_millis(interval_ms));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) =
                            Self::poll_once(&state, &event_tx, &classifier, &last_active_time).await
                        {
                            warn!(error = %e, "Desktop poll failed");
                        }
                        if event_tx.is_closed() {
                            info!("Desktop watcher shutting down (channel closed)");
                            break;
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Desktop watcher shutting down (signal received)");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(());
    }

    pub async fn take_events(&self) -> Vec<WorldModelEvent> {
        let mut rx = self.event_rx.write().await;
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }

    pub async fn current_snapshot(&self) -> DesktopState {
        let state = self.state.read().await;
        let windows: Vec<WindowInfo> = state.known_windows.values().cloned().collect();
        let focused_app = state.last_focused_app.clone();

        DesktopState {
            windows,
            active_window_id: state.last_active_window.clone(),
            workspaces: vec![],
            focused_app,
        }
    }

    async fn poll_once(
        state: &Arc<RwLock<DesktopWatcherState>>,
        event_tx: &mpsc::Sender<WorldModelEvent>,
        classifier: &ActivityClassifier,
        last_active_time: &Arc<RwLock<Instant>>,
    ) -> Result<()> {
        let snapshot = Self::capture_desktop_state().await?;

        let mut state_guard = state.write().await;
        let mut events = Vec::new();
        let now = Utc::now();

        // Detect focused app change
        let focused_changed = state_guard.last_focused_app != snapshot.focused_app;
        if focused_changed {
            if let Some(ref new_app) = snapshot.focused_app {
                if let Some(ref old_app) = state_guard.last_focused_app {
                    if old_app != new_app {
                        events.push(WorldModelEvent::ApplicationFocused {
                            app_id: new_app.clone(),
                            app_name: new_app.clone(),
                            timestamp: now,
                        });
                        events.push(WorldModelEvent::IdleEnded {
                            new_app: new_app.clone(),
                            timestamp: now,
                        });
                        state_guard.is_idle = false;
                    }
                } else {
                    events.push(WorldModelEvent::ApplicationFocused {
                        app_id: new_app.clone(),
                        app_name: new_app.clone(),
                        timestamp: now,
                    });
                }

                // Classify activity
                let desktop_state = DesktopState {
                    windows: snapshot.windows.clone(),
                    active_window_id: snapshot.active_window_id.clone(),
                    workspaces: vec![],
                    focused_app: snapshot.focused_app.clone(),
                };
                let classification = classifier.classify(&desktop_state);
                let new_activity_type = format!("{:?}", classification.activity_type);
                if state_guard.activity_type.as_ref() != Some(&new_activity_type) {
                    events.push(WorldModelEvent::ActivityChanged {
                        app_id: new_app.clone(),
                        activity_type: new_activity_type.clone(),
                        confidence: classification.confidence,
                        timestamp: now,
                    });
                    state_guard.activity_type = Some(new_activity_type);
                }
            } else {
                // No focused app - idle
                if !state_guard.is_idle {
                    events.push(WorldModelEvent::IdleStarted {
                        last_active_app: state_guard.last_focused_app.clone(),
                        timestamp: now,
                    });
                    state_guard.is_idle = true;
                }
            }
            state_guard.last_focused_app = snapshot.focused_app.clone();

            // Update last active time
            {
                let mut time = last_active_time.write().await;
                *time = Instant::now();
            }
        }

        // Detect window title change
        let current_title = snapshot
            .windows
            .iter()
            .find(|w| w.is_focused)
            .map(|w| w.title.clone());
        if current_title != state_guard.last_window_title {
            if let Some(ref app_id) = snapshot.focused_app {
                if let Some(ref title) = current_title {
                    events.push(WorldModelEvent::WindowChanged {
                        app_id: app_id.clone(),
                        window_title: title.clone(),
                        timestamp: now,
                    });

                    // Detect project from window title
                    if let Some(project) = Self::detect_project(title) {
                        events.push(WorldModelEvent::ProjectDetected {
                            project_name: project.name,
                            project_path: project.path,
                            language: project.language,
                            timestamp: now,
                        });
                    }
                }
            }
            state_guard.last_window_title = current_title;
        }

        // Emit context update if anything changed
        if !events.is_empty() {
            events.push(WorldModelEvent::ContextUpdated {
                focused_app: snapshot.focused_app.clone(),
                activity_type: state_guard.activity_type.clone(),
                window_title: snapshot
                    .windows
                    .iter()
                    .find(|w| w.is_focused)
                    .map(|w| w.title.clone()),
                timestamp: now,
            });
        }

        // Update known windows and apps
        state_guard.known_windows.clear();
        for w in &snapshot.windows {
            state_guard.known_windows.insert(w.id.clone(), w.clone());
        }
        state_guard.known_apps.clear();
        for a in &snapshot.apps {
            state_guard.known_apps.insert(a.id.clone(), a.clone());
        }

        for event in events {
            debug!(event = %event, "Emitting desktop event");
            let _ = event_tx.send(event).await;
        }

        Ok(())
    }

    fn detect_project(title: &str) -> Option<ProjectInfo> {
        let lower = title.to_lowercase();
        let mut name = String::new();
        let mut language = None;

        if lower.contains(".rs") {
            name = "Rust project".to_string();
            language = Some("Rust".to_string());
        } else if lower.contains(".py") {
            name = "Python project".to_string();
            language = Some("Python".to_string());
        } else if lower.contains(".js") || lower.contains(".ts") {
            name = "JavaScript project".to_string();
            language = Some("JavaScript".to_string());
        } else if lower.contains(".go") {
            name = "Go project".to_string();
            language = Some("Go".to_string());
        } else if lower.contains("github.com") || lower.contains("gitlab.com") {
            name = "Git project".to_string();
        } else if lower.contains("main.rs") || lower.contains("lib.rs") {
            name = "Rust project".to_string();
            language = Some("Rust".to_string());
        } else if lower.contains("index.html") || lower.contains("style.css") {
            name = "Web project".to_string();
            language = Some("HTML/CSS".to_string());
        }

        if !name.is_empty() {
            Some(ProjectInfo {
                name,
                path: None,
                language,
            })
        } else {
            None
        }
    }

    async fn capture_desktop_state() -> Result<DesktopSnapshot> {
        #[cfg(target_os = "windows")]
        {
            Self::capture_windows_state().await
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(crate::error::WorldModelError::DesktopError(
                "Desktop watching not supported on this platform".into(),
            ))
        }
    }

    #[cfg(target_os = "windows")]
    async fn capture_windows_state() -> Result<DesktopSnapshot> {
        let mut windows = Vec::new();
        let mut apps: HashMap<String, ApplicationInfo> = HashMap::new();

        let foreground_hwnd = unsafe { GetForegroundWindow() };
        let foreground_hwnd_val = foreground_hwnd.0 as isize;
        if foreground_hwnd_val == 0 {
            return Ok(DesktopSnapshot {
                windows,
                apps: apps.into_values().collect(),
                active_window_id: None,
                focused_app: None,
            });
        }

        let title = Self::get_window_title(foreground_hwnd);
        let (pid, exe_name) = Self::get_process_info(foreground_hwnd);
        let bounds = Self::get_window_bounds(foreground_hwnd);

        let win_id = format!("hwnd_{}", foreground_hwnd_val);
        let app_id = exe_name.as_deref().unwrap_or("unknown").to_string();

        let window = WindowInfo {
            id: win_id.clone(),
            title,
            application_id: app_id.clone(),
            application_name: app_id.clone(),
            bounds,
            is_focused: true,
            is_minimized: false,
            process_id: pid,
        };

        apps.entry(app_id.clone())
            .or_insert_with(|| ApplicationInfo {
                id: app_id.clone(),
                name: app_id.clone(),
                process_id: pid,
                bundle_id: None,
                is_running: true,
                window_count: 1,
            });

        windows.push(window);

        Ok(DesktopSnapshot {
            windows,
            apps: apps.into_values().collect(),
            active_window_id: Some(win_id),
            focused_app: Some(app_id),
        })
    }

    #[cfg(target_os = "windows")]
    fn get_window_title(hwnd: HWND) -> String {
        let mut buf = [0u16; 512];
        let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
        if len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            String::new()
        }
    }

    #[cfg(target_os = "windows")]
    fn get_process_info(hwnd: HWND) -> (Option<u32>, Option<String>) {
        let mut pid: u32 = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }

        if pid == 0 {
            return (None, None);
        }

        let handle =
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, BOOL::from(false), pid) };

        let exe_name = match handle {
            Ok(h) => {
                let result = Self::get_exe_name(h);
                unsafe {
                    let _ = CloseHandle(h);
                };
                result
            }
            Err(_) => None,
        };

        (Some(pid), exe_name)
    }

    #[cfg(target_os = "windows")]
    fn get_exe_name(handle: windows::Win32::Foundation::HANDLE) -> Option<String> {
        let mut buf = [0u16; 260];

        let success = unsafe { GetModuleFileNameExW(handle, None, &mut buf) };

        if success != 0 {
            let path = String::from_utf16_lossy(&buf[..success as usize]);
            std::path::Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        } else {
            None
        }
    }

    #[cfg(target_os = "windows")]
    fn get_window_bounds(hwnd: HWND) -> Option<voxy_shared::Rect> {
        let mut rect = windows::Win32::Foundation::RECT::default();
        unsafe {
            if GetWindowRect(hwnd, &mut rect).is_ok() {
                Some(voxy_shared::Rect::new(
                    rect.left,
                    rect.top,
                    (rect.right - rect.left) as u32,
                    (rect.bottom - rect.top) as u32,
                ))
            } else {
                None
            }
        }
    }
}

struct DesktopSnapshot {
    windows: Vec<WindowInfo>,
    apps: Vec<ApplicationInfo>,
    active_window_id: Option<String>,
    focused_app: Option<String>,
}

struct ProjectInfo {
    name: String,
    path: Option<String>,
    language: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watcher_creation() {
        let config = WorldModelConfig::default();
        let _watcher = DesktopWatcher::new(config);
    }

    #[test]
    fn test_watcher_initial_state() {
        let config = WorldModelConfig::default();
        let watcher = DesktopWatcher::new(config);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let state = rt.block_on(watcher.current_snapshot());
        assert!(state.windows.is_empty());
        assert!(state.active_window_id.is_none());
        assert!(state.focused_app.is_none());
    }

    #[test]
    fn test_detect_project() {
        assert!(DesktopWatcher::detect_project("main.rs - VS Code").is_some());
        assert!(DesktopWatcher::detect_project("index.html - Chrome").is_some());
        assert!(DesktopWatcher::detect_project("app.py - PyCharm").is_some());
        assert!(DesktopWatcher::detect_project("Random window title").is_none());
    }
}
