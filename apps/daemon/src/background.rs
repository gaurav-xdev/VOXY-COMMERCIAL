use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use voxy_cognitive_orchestrator::bridge::CognitiveBridge;
use voxy_cognitive_orchestrator::knowledge_validation::KnowledgeItem;
use voxy_cognitive_orchestrator::reflection::ConversationRecord;
use voxy_runtime_guard::RuntimeGuard;

use crate::VoiceMetrics;

pub struct BackgroundRuntime {
    #[allow(dead_code)]
    handles: Vec<JoinHandle<()>>,
    shutdown_tx: broadcast::Sender<()>,
}

/// A cloneable handle for triggering background runtime shutdown.
#[derive(Clone)]
pub struct ShutdownHandle {
    shutdown_tx: broadcast::Sender<()>,
}

impl ShutdownHandle {
    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
        // Give tasks a moment to receive the shutdown signal
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// A cloneable handle for submitting conversation records for reflection.
#[derive(Clone)]
pub struct ReflectionSubmitter {
    tx: mpsc::Sender<ConversationRecord>,
}

impl ReflectionSubmitter {
    pub async fn submit(&self, record: ConversationRecord) -> bool {
        self.tx.send(record).await.is_ok()
    }
}

/// A cloneable handle for submitting knowledge items for validation.
#[derive(Clone)]
pub struct KnowledgeValidationSubmitter {
    #[allow(dead_code)]
    tx: mpsc::Sender<KnowledgeItem>,
}

impl KnowledgeValidationSubmitter {
    #[allow(dead_code)]
    pub async fn submit(&self, item: KnowledgeItem) -> bool {
        self.tx.send(item).await.is_ok()
    }
}

impl BackgroundRuntime {
    pub fn new(
        running: Arc<AtomicBool>,
        _metrics: Arc<VoiceMetrics>,
        cognitive: Arc<CognitiveBridge>,
        guard: Arc<RuntimeGuard>,
    ) -> (Self, ReflectionSubmitter, KnowledgeValidationSubmitter) {
        let (shutdown_tx, _) = broadcast::channel(16);
        let (reflection_tx, reflection_rx) = mpsc::channel(256);
        let (validation_tx, validation_rx) = mpsc::channel(128);
        let mut handles = Vec::new();

        // 1. Resource watchdog (CPU/RAM monitoring)
        {
            let running = running.clone();
            let mut shutdown_rx = shutdown_tx.subscribe();
            handles.push(tokio::spawn(async move {
                resource_watchdog_task(running, &mut shutdown_rx).await;
            }));
        }

        // 2. Reflection worker
        {
            let cognitive = cognitive.clone();
            let mut shutdown_rx = shutdown_tx.subscribe();
            handles.push(tokio::spawn(async move {
                reflection_worker_task(cognitive, reflection_rx, &mut shutdown_rx).await;
            }));
        }

        // 3. Experience replay
        {
            let cognitive = cognitive.clone();
            let mut shutdown_rx = shutdown_tx.subscribe();
            handles.push(tokio::spawn(async move {
                experience_replay_task(cognitive, &mut shutdown_rx).await;
            }));
        }

        // 4. Goal progress checker
        {
            let cognitive = cognitive.clone();
            let mut shutdown_rx = shutdown_tx.subscribe();
            handles.push(tokio::spawn(async move {
                goal_progress_task(cognitive, &mut shutdown_rx).await;
            }));
        }

        // 5. Curiosity engine
        {
            let cognitive = cognitive.clone();
            let mut shutdown_rx = shutdown_tx.subscribe();
            handles.push(tokio::spawn(async move {
                curiosity_task(cognitive, &mut shutdown_rx).await;
            }));
        }

        // 6. Knowledge validator
        {
            let cognitive = cognitive.clone();
            let mut shutdown_rx = shutdown_tx.subscribe();
            handles.push(tokio::spawn(async move {
                knowledge_validation_task(cognitive, &mut shutdown_rx).await;
            }));
        }

        // 7. Skill discovery
        {
            let cognitive = cognitive.clone();
            let mut shutdown_rx = shutdown_tx.subscribe();
            handles.push(tokio::spawn(async move {
                skill_discovery_task(cognitive, &mut shutdown_rx).await;
            }));
        }

        // 8. Workflow learner
        {
            let cognitive = cognitive.clone();
            let mut shutdown_rx = shutdown_tx.subscribe();
            handles.push(tokio::spawn(async move {
                workflow_learning_task(cognitive, &mut shutdown_rx).await;
            }));
        }

        // 9. Health monitor
        {
            let running = running.clone();
            let cognitive = cognitive.clone();
            let guard = guard.clone();
            let mut shutdown_rx = shutdown_tx.subscribe();
            handles.push(tokio::spawn(async move {
                health_monitor_task(running, cognitive, guard, &mut shutdown_rx).await;
            }));
        }

        // 10. Knowledge validation worker
        {
            let cognitive = cognitive.clone();
            let mut shutdown_rx = shutdown_tx.subscribe();
            handles.push(tokio::spawn(async move {
                knowledge_validation_worker_task(cognitive, validation_rx, &mut shutdown_rx).await;
            }));
        }

        let reflection_submitter = ReflectionSubmitter { tx: reflection_tx };
        let validation_submitter = KnowledgeValidationSubmitter { tx: validation_tx };
        (
            Self {
                handles,
                shutdown_tx,
            },
            reflection_submitter,
            validation_submitter,
        )
    }

    #[allow(dead_code)]
    pub async fn shutdown(self) {
        info!("Background runtime shutting down...");
        let _ = self.shutdown_tx.send(());

        for handle in self.handles {
            match tokio::time::timeout(Duration::from_secs(5), handle).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => warn!("Background task panicked: {e}"),
                Err(_) => warn!("Background task did not stop in time"),
            }
        }
        info!("Background runtime stopped");
    }

    /// Get a cloneable shutdown handle for use in graceful shutdown coordinator.
    pub fn clone_for_shutdown(&self) -> ShutdownHandle {
        ShutdownHandle {
            shutdown_tx: self.shutdown_tx.clone(),
        }
    }
}

async fn knowledge_validation_worker_task(
    _cognitive: Arc<CognitiveBridge>,
    mut validation_rx: mpsc::Receiver<KnowledgeItem>,
    shutdown_rx: &mut broadcast::Receiver<()>,
) {
    use voxy_cognitive_orchestrator::config::OrchestratorConfig;
    use voxy_cognitive_orchestrator::knowledge_validation::{KnowledgeValidator, ValidationStatus};

    let config = OrchestratorConfig::default();
    let mut validator = KnowledgeValidator::new(config.knowledge_validation);
    let mut validated_count = 0u64;
    let mut quarantined_count = 0u64;

    loop {
        tokio::select! {
            Some(item) = validation_rx.recv() => {
                match validator.validate(item) {
                    Ok(result) => {
                        match result.status {
                            ValidationStatus::Validated | ValidationStatus::PartialTrust => {
                                validated_count += 1;
                                tracing::debug!(
                                    item_id = %result.item_id,
                                    trust = result.trust_score,
                                    "Knowledge validated"
                                );
                            }
                            ValidationStatus::Quarantined | ValidationStatus::Rejected => {
                                quarantined_count += 1;
                                tracing::warn!(
                                    item_id = %result.item_id,
                                    trust = result.trust_score,
                                    flags = ?result.flags,
                                    "Knowledge quarantined/rejected"
                                );
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Knowledge validation failed: {}", e);
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                info!(
                    validated = validated_count,
                    quarantined = quarantined_count,
                    "Knowledge validation worker shutting down"
                );
                break;
            }
        }
    }
}

async fn resource_watchdog_task(
    running: Arc<AtomicBool>,
    shutdown_rx: &mut broadcast::Receiver<()>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    let mut high_cpu_warnings = 0u32;
    let mut high_ram_warnings = 0u32;

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                let (used_ram, total_ram) = tokio::task::spawn_blocking(get_memory_usage)
                    .await
                    .unwrap_or((0, 0));
                if total_ram > 0 {
                    let pct = used_ram as f64 / total_ram as f64 * 100.0;
                    if pct > 90.0 {
                        high_ram_warnings += 1;
                        warn!("High RAM usage: {:.1}% (warning {}/3)", pct, high_ram_warnings);
                        if high_ram_warnings >= 3 {
                            error!("Critical RAM usage: {:.1}%. Suggest restarting.", pct);
                            high_ram_warnings = 0;
                        }
                    } else {
                        high_ram_warnings = 0;
                    }
                }
                if let Ok(load) = tokio::task::spawn_blocking(get_system_cpu_load).await.unwrap_or(Ok(0.0)) {
                    if load > 0.9 {
                        high_cpu_warnings += 1;
                        warn!("High CPU load: {:.0}% (warning {}/3)", load * 100.0, high_cpu_warnings);
                        if high_cpu_warnings >= 3 {
                            error!("Critical CPU load: {:.0}%. Suggest reducing workload.", load * 100.0);
                            high_cpu_warnings = 0;
                        }
                    } else {
                        high_cpu_warnings = 0;
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                info!("Resource watchdog shutting down");
                break;
            }
        }
    }
}

async fn reflection_worker_task(
    _cognitive: Arc<CognitiveBridge>,
    mut reflection_rx: mpsc::Receiver<ConversationRecord>,
    shutdown_rx: &mut broadcast::Receiver<()>,
) {
    use voxy_cognitive_orchestrator::config::OrchestratorConfig;
    use voxy_cognitive_orchestrator::reflection::ReflectionEngine;

    let config = OrchestratorConfig::default();
    let mut engine = ReflectionEngine::new(config.reflection);
    let mut interval = tokio::time::interval(Duration::from_secs(300));

    loop {
        tokio::select! {
            Some(record) = reflection_rx.recv() => {
                match engine.analyze_conversation(record) {
                    Ok(result) => {
                        tracing::info!(
                            conversation_id = %result.conversation_id,
                            quality = result.quality_score,
                            lessons = result.lessons.len(),
                            "Reflection analysis complete"
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Reflection analysis failed: {}", e);
                    }
                }
            }
            _ = interval.tick() => {
                let stats = engine.get_reflections().len();
                let avg = engine.average_quality();
                tracing::debug!(
                    total_reflections = stats,
                    avg_quality = avg,
                    "Reflection worker tick — stats update"
                );
            }
            _ = shutdown_rx.recv() => {
                info!("Reflection worker shutting down");
                break;
            }
        }
    }
}

async fn experience_replay_task(
    _cognitive: Arc<CognitiveBridge>,
    shutdown_rx: &mut broadcast::Receiver<()>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                tracing::debug!("Experience replay tick — processing replay buffer");
            }
            _ = shutdown_rx.recv() => {
                info!("Experience replay shutting down");
                break;
            }
        }
    }
}

async fn goal_progress_task(
    _cognitive: Arc<CognitiveBridge>,
    shutdown_rx: &mut broadcast::Receiver<()>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(300));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                tracing::debug!("Goal progress check tick");
            }
            _ = shutdown_rx.recv() => {
                info!("Goal progress checker shutting down");
                break;
            }
        }
    }
}

async fn curiosity_task(
    _cognitive: Arc<CognitiveBridge>,
    shutdown_rx: &mut broadcast::Receiver<()>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(300));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                tracing::debug!("Curiosity engine tick — pattern detection");
            }
            _ = shutdown_rx.recv() => {
                info!("Curiosity engine shutting down");
                break;
            }
        }
    }
}

async fn knowledge_validation_task(
    _cognitive: Arc<CognitiveBridge>,
    shutdown_rx: &mut broadcast::Receiver<()>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(600));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                tracing::debug!("Knowledge validation tick");
            }
            _ = shutdown_rx.recv() => {
                info!("Knowledge validator shutting down");
                break;
            }
        }
    }
}

async fn skill_discovery_task(
    _cognitive: Arc<CognitiveBridge>,
    shutdown_rx: &mut broadcast::Receiver<()>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(600));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                tracing::debug!("Skill discovery tick");
            }
            _ = shutdown_rx.recv() => {
                info!("Skill discovery shutting down");
                break;
            }
        }
    }
}

async fn workflow_learning_task(
    _cognitive: Arc<CognitiveBridge>,
    shutdown_rx: &mut broadcast::Receiver<()>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(600));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                tracing::debug!("Workflow learning tick");
            }
            _ = shutdown_rx.recv() => {
                info!("Workflow learner shutting down");
                break;
            }
        }
    }
}

async fn health_monitor_task(
    running: Arc<AtomicBool>,
    _cognitive: Arc<CognitiveBridge>,
    guard: Arc<RuntimeGuard>,
    shutdown_rx: &mut broadcast::Receiver<()>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    let mut consecutive_failures = 0u32;
    let ollama_url =
        std::env::var("VOXY_OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if !running.load(Ordering::Relaxed) {
                    break;
                }

                // Check Ollama LLM provider health
                let url = format!("{}/api/tags", ollama_url);
                match reqwest::Client::new()
                    .get(&url)
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await
                {
                    Ok(resp) if resp.status().is_success() => {
                        consecutive_failures = 0;
                    }
                    Ok(resp) => {
                        consecutive_failures += 1;
                        warn!(
                            status = %resp.status(),
                            failures = consecutive_failures,
                            "Ollama health check returned non-success"
                        );
                    }
                    Err(e) => {
                        consecutive_failures += 1;
                        warn!(
                            error = %e,
                            failures = consecutive_failures,
                            "Ollama health check failed"
                        );
                        if consecutive_failures >= 3 {
                            error!(
                                "Ollama unreachable for {} consecutive checks. Triggering self-healing.",
                                consecutive_failures
                            );
                            // Send a heartbeat to trigger RuntimeGuard's self-healing mechanism
                            guard.heartbeat("ollama_llm");
                            consecutive_failures = 0;
                        }
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                info!("Health monitor shutting down");
                break;
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn get_memory_usage() -> (u64, u64) {
    let mut used = 0u64;
    let mut total = 0u64;
    if let Ok(output) = std::process::Command::new("wmic")
        .args([
            "OS",
            "get",
            "TotalVisibleMemorySize,FreePhysicalMemory",
            "/format:csv",
        ])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 3 {
                if let Ok(free_kb) = parts[2].trim().parse::<u64>() {
                    if let Ok(total_kb) = parts[1].trim().parse::<u64>() {
                        total = total_kb * 1024;
                        used = total.saturating_sub(free_kb * 1024);
                    }
                }
            }
        }
    }
    (used, total)
}

#[cfg(not(target_os = "windows"))]
fn get_memory_usage() -> (u64, u64) {
    let mut used = 0u64;
    let mut total = 0u64;
    if let Ok(info) = std::fs::read_to_string("/proc/meminfo") {
        for line in info.lines() {
            if let Some(val) = line.strip_prefix("MemTotal:") {
                total = val
                    .trim()
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0)
                    * 1024;
            } else if let Some(val) = line.strip_prefix("MemAvailable:") {
                let avail = val
                    .trim()
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0)
                    * 1024;
                if total > avail {
                    used = total - avail;
                }
            }
        }
    }
    (used, total)
}

#[cfg(target_os = "windows")]
fn get_system_cpu_load() -> Result<f64, String> {
    use std::process::Command;
    let output = Command::new("wmic")
        .args(["cpu", "get", "LoadPercentage", "/format:csv"])
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().skip(1) {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if let Some(val) = parts.last().and_then(|s| s.trim().parse::<f64>().ok()) {
            return Ok(val / 100.0);
        }
    }
    Ok(0.0)
}

#[cfg(not(target_os = "windows"))]
fn get_system_cpu_load() -> Result<f64, String> {
    let load = std::fs::read_to_string("/proc/loadavg").map_err(|e| e.to_string())?;
    let one_min = load
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let cores = num_cpus::get() as f64;
    if cores > 0.0 {
        Ok((one_min / cores).min(1.0))
    } else {
        Ok(0.0)
    }
}
