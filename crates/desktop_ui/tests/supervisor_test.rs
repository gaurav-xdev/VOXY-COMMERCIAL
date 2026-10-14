use std::time::Duration;
use voxy_desktop_ui::supervisor::{Supervisor, SupervisorConfig, WindowsJobObject};

#[test]
fn test_integration_windows_job_object_limits() {
    let job = WindowsJobObject::new().expect("Failed to create Windows Job Object");
    drop(job);
}

#[tokio::test]
async fn test_integration_supervisor_lifecycle_and_shutdown() {
    let pipe_name = format!(r"\\.\pipe\osmoo-test-sup-{}", uuid::Uuid::new_v4());
    let cfg = SupervisorConfig {
        autostart_daemon: false,
        autostart_overlay: false,
        graceful_shutdown_timeout: Duration::from_millis(500),
        pipe_name,
        ..Default::default()
    };

    let supervisor = Supervisor::new(cfg).expect("Supervisor init failed");
    let status_before = supervisor.status();
    assert!(!status_before.is_shutting_down);

    let shutdown_res = supervisor.shutdown().await;
    assert!(shutdown_res.is_ok(), "Shutdown should execute cleanly");

    let status_after = supervisor.status();
    assert!(status_after.is_shutting_down);
}
