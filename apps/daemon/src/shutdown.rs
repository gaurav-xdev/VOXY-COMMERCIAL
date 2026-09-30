use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// Priority of a shutdown step. Lower number = shutdown earlier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShutdownPriority {
    /// High priority: flush critical data (memory, state)
    #[allow(dead_code)]
    Flush = 0,
    /// Medium priority: stop background tasks
    Background = 1,
    /// Low priority: stop user-facing services
    Services = 2,
    /// Final: stop audio/capture
    #[allow(dead_code)]
    Final = 3,
}

type AsyncFn = Box<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

struct ShutdownStep {
    name: String,
    priority: ShutdownPriority,
    timeout: Duration,
    flush_fn: Option<AsyncFn>,
    shutdown_fn: AsyncFn,
}

pub struct GracefulShutdown {
    steps: Vec<ShutdownStep>,
    running: Arc<AtomicBool>,
    shutdown_timeout: Duration,
}

impl GracefulShutdown {
    pub fn new(running: Arc<AtomicBool>) -> Self {
        Self {
            steps: Vec::new(),
            running,
            shutdown_timeout: Duration::from_secs(30),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.shutdown_timeout = timeout;
        self
    }

    /// Register a subsystem with flush + shutdown.
    pub fn register<F, Fut, S, SFut>(
        &mut self,
        name: &str,
        priority: ShutdownPriority,
        timeout: Duration,
        flush_fn: Option<F>,
        shutdown_fn: S,
    ) where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
        S: Fn() -> SFut + Send + Sync + 'static,
        SFut: Future<Output = ()> + Send + 'static,
    {
        self.steps.push(ShutdownStep {
            name: name.to_string(),
            priority,
            timeout,
            flush_fn: flush_fn.map(|f| {
                Box::new(move || -> Pin<Box<dyn Future<Output = ()> + Send>> { Box::pin(f()) })
                    as Box<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>
            }),
            shutdown_fn: Box::new(move || -> Pin<Box<dyn Future<Output = ()> + Send>> {
                Box::pin(shutdown_fn())
            }),
        });
    }

    /// Register a simple shutdown (no flush) subsystem.
    pub fn register_simple<F, Fut>(
        &mut self,
        name: &str,
        priority: ShutdownPriority,
        timeout: Duration,
        shutdown_fn: F,
    ) where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.register(
            name,
            priority,
            timeout,
            None::<fn() -> std::future::Ready<()>>,
            shutdown_fn,
        )
    }

    /// Execute graceful shutdown: flush first, then shutdown by priority order.
    pub async fn execute(&self) {
        info!(
            "Starting graceful shutdown ({} subsystems)",
            self.steps.len()
        );
        self.running.store(false, Ordering::Relaxed);

        // Sort by priority (lower = earlier)
        let mut sorted: Vec<&ShutdownStep> = self.steps.iter().collect();
        sorted.sort_by_key(|s| s.priority);

        // Phase 1: Flush all subsystems
        info!("Phase 1: Flushing critical data...");
        for step in &sorted {
            if let Some(flush) = &step.flush_fn {
                info!("  Flushing {}...", step.name);
                let name = step.name.clone();
                let fut = flush();
                let flush_timeout = step.timeout.max(Duration::from_secs(5));
                match tokio::time::timeout(flush_timeout, fut).await {
                    Ok(()) => info!("  Flush {} complete", name),
                    Err(_) => warn!("  Flush {} timed out", name),
                }
            }
        }

        // Phase 2: Shutdown in priority order
        info!("Phase 2: Shutting down subsystems...");
        for step in &sorted {
            info!(
                "  Shutting down {} (timeout: {:?})...",
                step.name, step.timeout
            );
            let name = step.name.clone();
            let fut = (step.shutdown_fn)();
            match tokio::time::timeout(step.timeout, fut).await {
                Ok(()) => info!("  {} stopped", name),
                Err(_) => error!("  {} timed out after {:?}", name, step.timeout),
            }
        }

        info!("Graceful shutdown complete");
    }

    #[allow(dead_code)]
    pub fn is_shutting_down(&self) -> bool {
        !self.running.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shutdown_creation() {
        let running = Arc::new(AtomicBool::new(true));
        let shutdown = GracefulShutdown::new(running.clone());
        assert!(!shutdown.is_shutting_down());
        assert!(shutdown.steps.is_empty());
    }

    #[tokio::test]
    async fn test_register_simple() {
        let running = Arc::new(AtomicBool::new(true));
        let mut shutdown = GracefulShutdown::new(running.clone());
        shutdown.register_simple(
            "test",
            ShutdownPriority::Services,
            Duration::from_secs(5),
            || async {},
        );
        assert_eq!(shutdown.steps.len(), 1);
        assert_eq!(shutdown.steps[0].name, "test");
    }

    #[tokio::test]
    async fn test_register_with_flush() {
        let running = Arc::new(AtomicBool::new(true));
        let mut shutdown = GracefulShutdown::new(running.clone());
        let flushed = Arc::new(AtomicBool::new(false));
        let flushed_clone = flushed.clone();
        shutdown.register(
            "db",
            ShutdownPriority::Flush,
            Duration::from_secs(5),
            Some(move || {
                let flushed = flushed_clone.clone();
                async move {
                    flushed.store(true, Ordering::Relaxed);
                }
            }),
            || async {},
        );
        assert_eq!(shutdown.steps.len(), 1);
        assert!(shutdown.steps[0].flush_fn.is_some());
    }

    #[tokio::test]
    async fn test_execute_shutdown() {
        let running = Arc::new(AtomicBool::new(true));
        let mut shutdown = GracefulShutdown::new(running.clone());
        let order = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        let o = order.clone();
        shutdown.register_simple(
            "final",
            ShutdownPriority::Final,
            Duration::from_secs(5),
            move || {
                let o = o.clone();
                async move {
                    o.lock().await.push("final");
                }
            },
        );

        let o = order.clone();
        shutdown.register_simple(
            "flush",
            ShutdownPriority::Flush,
            Duration::from_secs(5),
            move || {
                let o = o.clone();
                async move {
                    o.lock().await.push("flush");
                }
            },
        );

        shutdown.execute().await;
        assert!(!running.load(Ordering::Relaxed));

        let result = order.lock().await;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "flush");
        assert_eq!(result[1], "final");
    }
}
