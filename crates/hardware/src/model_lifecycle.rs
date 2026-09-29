use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelLifecycleState {
    Unloaded,
    Loading,
    Loaded,
    Active,
    Evicted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelLifecycleInfo {
    pub model_id: String,
    pub state: ModelLifecycleState,
    pub load_duration_ms: Option<u64>,
    pub total_invocations: u64,
}

struct ManagedModel {
    model_id: String,
    state: ModelLifecycleState,
    load_start: Option<Instant>,
    loaded_at: Option<Instant>,
    last_invoked: Option<Instant>,
    load_duration_ms: Option<u64>,
    total_invocations: u64,
}

pub struct ModelLifecycleManager {
    idle_eviction_timeout: Duration,
    models: RwLock<HashMap<String, ManagedModel>>,
}

impl ModelLifecycleManager {
    /// Creates a new model lifecycle manager with a default 10-minute idle eviction timeout.
    pub fn new() -> Self {
        Self {
            idle_eviction_timeout: Duration::from_secs(600),
            models: RwLock::new(HashMap::new()),
        }
    }

    pub fn with_idle_timeout(timeout: Duration) -> Self {
        Self {
            idle_eviction_timeout: timeout,
            models: RwLock::new(HashMap::new()),
        }
    }

    pub fn mark_loading(&self, model_id: &str) {
        let mut models = self.models.write().unwrap();
        let entry = models.entry(model_id.to_string()).or_insert_with(|| ManagedModel {
            model_id: model_id.to_string(),
            state: ModelLifecycleState::Unloaded,
            load_start: None,
            loaded_at: None,
            last_invoked: None,
            load_duration_ms: None,
            total_invocations: 0,
        });
        entry.state = ModelLifecycleState::Loading;
        entry.load_start = Some(Instant::now());
    }

    pub fn mark_loaded(&self, model_id: &str) {
        let mut models = self.models.write().unwrap();
        if let Some(entry) = models.get_mut(model_id) {
            entry.state = ModelLifecycleState::Loaded;
            let duration = entry.load_start.map(|s| s.elapsed().as_millis() as u64);
            entry.load_duration_ms = duration;
            entry.loaded_at = Some(Instant::now());
            entry.last_invoked = Some(Instant::now());
        }
    }

    pub fn mark_active(&self, model_id: &str) {
        let mut models = self.models.write().unwrap();
        if let Some(entry) = models.get_mut(model_id) {
            entry.state = ModelLifecycleState::Active;
            entry.last_invoked = Some(Instant::now());
            entry.total_invocations += 1;
        }
    }

    pub fn mark_idle(&self, model_id: &str) {
        let mut models = self.models.write().unwrap();
        if let Some(entry) = models.get_mut(model_id) {
            if entry.state == ModelLifecycleState::Active {
                entry.state = ModelLifecycleState::Loaded;
            }
        }
    }

    pub fn mark_evicted(&self, model_id: &str) {
        let mut models = self.models.write().unwrap();
        if let Some(entry) = models.get_mut(model_id) {
            entry.state = ModelLifecycleState::Evicted;
        }
    }

    /// Finds models that have been idle longer than the eviction timeout.
    pub fn find_eviction_candidates(&self) -> Vec<String> {
        let models = self.models.read().unwrap();
        let now = Instant::now();
        models
            .values()
            .filter(|m| {
                m.state == ModelLifecycleState::Loaded
                    && m.last_invoked
                        .map(|t| now.duration_since(t) > self.idle_eviction_timeout)
                        .unwrap_or(false)
            })
            .map(|m| m.model_id.clone())
            .collect()
    }

    pub fn get_info(&self, model_id: &str) -> Option<ModelLifecycleInfo> {
        let models = self.models.read().unwrap();
        models.get(model_id).map(|m| ModelLifecycleInfo {
            model_id: m.model_id.clone(),
            state: m.state,
            load_duration_ms: m.load_duration_ms,
            total_invocations: m.total_invocations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifecycle_transitions() {
        let manager = ModelLifecycleManager::new();
        manager.mark_loading("llama3");
        assert_eq!(
            manager.get_info("llama3").unwrap().state,
            ModelLifecycleState::Loading
        );

        manager.mark_loaded("llama3");
        assert_eq!(
            manager.get_info("llama3").unwrap().state,
            ModelLifecycleState::Loaded
        );

        manager.mark_active("llama3");
        assert_eq!(
            manager.get_info("llama3").unwrap().state,
            ModelLifecycleState::Active
        );
        assert_eq!(manager.get_info("llama3").unwrap().total_invocations, 1);

        manager.mark_idle("llama3");
        assert_eq!(
            manager.get_info("llama3").unwrap().state,
            ModelLifecycleState::Loaded
        );

        manager.mark_evicted("llama3");
        assert_eq!(
            manager.get_info("llama3").unwrap().state,
            ModelLifecycleState::Evicted
        );
    }
}
