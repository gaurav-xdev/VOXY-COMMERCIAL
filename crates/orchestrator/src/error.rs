#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("Task error: {0}")]
    TaskError(String),
    #[error("Scheduling error: {0}")]
    SchedulingError(String),
    #[error("Interruption error: {0}")]
    InterruptionError(String),
    #[error("Cancellation error: {0}")]
    CancellationError(String),
    #[error("Job error: {0}")]
    JobError(String),
    #[error("Priority error: {0}")]
    PriorityError(String),
    #[error("Guardian override: {0}")]
    GuardianOverride(String),
    #[error("Automation error: {0}")]
    AutomationError(String),
    #[error("Home error: {0}")]
    HomeError(String),
    #[error("Plugin error: {0}")]
    PluginError(String),
    #[error("Integration error: {0}")]
    IntegrationError(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Shutdown error: {0}")]
    ShutdownError(String),
    #[error("Pipeline error: {0}")]
    PipelineError(String),
    #[error("Stage error: {0}")]
    StageError(String),
}

pub type Result<T> = std::result::Result<T, OrchestratorError>;
