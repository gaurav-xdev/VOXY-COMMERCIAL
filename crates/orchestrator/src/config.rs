pub struct OrchestratorConfig {
    pub max_concurrent_tasks: usize,
    pub max_priority_levels: usize,
    pub enable_guardian_override: bool,
    pub enable_emergency_override: bool,
    pub enable_background_tasks: bool,
    pub task_timeout_seconds: u64,
    pub scheduler_tick_ms: u64,
    pub max_retries_per_task: u32,
    pub interruption_cooldown_ms: u64,
    pub health_check_interval_seconds: u64,
    pub pipeline_timeout_seconds: u64,
    pub enable_audit_trail: bool,
    pub enable_correlation_tracking: bool,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 50,
            max_priority_levels: 5,
            enable_guardian_override: true,
            enable_emergency_override: true,
            enable_background_tasks: true,
            task_timeout_seconds: 300,
            scheduler_tick_ms: 100,
            max_retries_per_task: 3,
            interruption_cooldown_ms: 500,
            health_check_interval_seconds: 30,
            pipeline_timeout_seconds: 600,
            enable_audit_trail: true,
            enable_correlation_tracking: true,
        }
    }
}
