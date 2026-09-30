use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryType {
    Episodic,
    Semantic,
    Working,
    Procedural,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ImportanceLevel {
    Critical,
    High,
    Medium,
    Low,
    Negligible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: String,
    pub memory_type: MemoryType,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub access_count: usize,
    pub decay_rate: f64,
    pub semantic_value: f64,
    pub project_value: f64,
    pub emotional_weight: f64,
    pub tags: Vec<String>,
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryScore {
    pub memory_id: String,
    pub recall_probability: f64,
    pub importance: ImportanceLevel,
    pub reasons: Vec<String>,
}

pub struct MemoryImportanceEngine {
    memories: Vec<MemoryItem>,
    config: ImportanceConfig,
    decay_timer: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct ImportanceConfig {
    pub base_decay_rate: f64,
    pub max_memories: usize,
    pub promotion_threshold: f64,
    pub forgetting_threshold: f64,
    pub recency_weight: f64,
    pub frequency_weight: f64,
    pub semantic_weight: f64,
    pub project_weight: f64,
    pub emotional_weight: f64,
}

impl Default for ImportanceConfig {
    fn default() -> Self {
        Self {
            base_decay_rate: 0.01,
            max_memories: 1000,
            promotion_threshold: 0.7,
            forgetting_threshold: 0.1,
            recency_weight: 0.25,
            frequency_weight: 0.2,
            semantic_weight: 0.2,
            project_weight: 0.2,
            emotional_weight: 0.15,
        }
    }
}

impl MemoryImportanceEngine {
    pub fn new(config: ImportanceConfig) -> Self {
        Self {
            memories: Vec::with_capacity(config.max_memories),
            config,
            decay_timer: None,
        }
    }

    pub fn add_memory(&mut self, memory: MemoryItem) {
        if self.memories.len() >= self.config.max_memories {
            self.forget_lowest_importance();
        }
        self.memories.push(memory);
    }

    pub fn score_memory(&self, memory: &MemoryItem) -> MemoryScore {
        let recency = self.calculate_recency(memory);
        let frequency = self.calculate_frequency(memory);
        let semantic = memory.semantic_value;
        let project = memory.project_value;
        let emotional = memory.emotional_weight;

        let recall_probability = recency * self.config.recency_weight
            + frequency * self.config.frequency_weight
            + semantic * self.config.semantic_weight
            + project * self.config.project_weight
            + emotional * self.config.emotional_weight;

        let recall_probability = recall_probability.clamp(0.0, 1.0);

        let importance = self.classify_importance(recall_probability);
        let reasons = self.generate_reasons(memory, recency, frequency);

        MemoryScore {
            memory_id: memory.id.clone(),
            recall_probability,
            importance,
            reasons,
        }
    }

    pub fn score_all(&self) -> Vec<MemoryScore> {
        self.memories.iter().map(|m| self.score_memory(m)).collect()
    }

    pub fn get_memories_for_promotion(&self) -> Vec<&MemoryItem> {
        self.memories
            .iter()
            .filter(|m| {
                let score = self.score_memory(m);
                score.recall_probability >= self.config.promotion_threshold
            })
            .collect()
    }

    pub fn get_memories_for_forgetting(&self) -> Vec<&MemoryItem> {
        self.memories
            .iter()
            .filter(|m| {
                let score = self.score_memory(m);
                score.recall_probability <= self.config.forgetting_threshold
            })
            .collect()
    }

    pub fn apply_decay(&mut self) {
        let now = Utc::now();
        if let Some(timer) = self.decay_timer {
            let elapsed = (now - timer).num_seconds() as f64;
            if elapsed < 60.0 {
                return;
            }
        }

        for memory in &mut self.memories {
            let hours_since_access = (now - memory.last_accessed).num_hours() as f64;
            let decay = self.config.base_decay_rate * hours_since_access / 24.0;
            memory.decay_rate = (memory.decay_rate + decay).min(1.0);
        }

        self.decay_timer = Some(now);
    }

    pub fn access_memory(&mut self, memory_id: &str) -> Option<&MemoryItem> {
        if let Some(memory) = self.memories.iter_mut().find(|m| m.id == memory_id) {
            memory.last_accessed = Utc::now();
            memory.access_count += 1;
            memory.decay_rate = (memory.decay_rate - 0.1).max(0.0);
            Some(memory)
        } else {
            None
        }
    }

    pub fn forget_lowest_importance(&mut self) -> Option<MemoryItem> {
        if self.memories.is_empty() {
            return None;
        }

        let worst_idx = self
            .memories
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let score_a = self.score_memory(a).recall_probability;
                let score_b = self.score_memory(b).recall_probability;
                score_a
                    .partial_cmp(&score_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| idx);

        if let Some(idx) = worst_idx {
            Some(self.memories.remove(idx))
        } else {
            None
        }
    }

    pub fn memories(&self) -> &[MemoryItem] {
        &self.memories
    }

    pub fn memory_count(&self) -> usize {
        self.memories.len()
    }

    fn calculate_recency(&self, memory: &MemoryItem) -> f64 {
        let hours = (Utc::now() - memory.last_accessed).num_hours() as f64;
        1.0 / (1.0 + hours / 24.0)
    }

    fn calculate_frequency(&self, memory: &MemoryItem) -> f64 {
        (memory.access_count as f64).log10() / 3.0
    }

    fn classify_importance(&self, recall_probability: f64) -> ImportanceLevel {
        if recall_probability >= 0.9 {
            ImportanceLevel::Critical
        } else if recall_probability >= 0.7 {
            ImportanceLevel::High
        } else if recall_probability >= 0.5 {
            ImportanceLevel::Medium
        } else if recall_probability >= 0.3 {
            ImportanceLevel::Low
        } else {
            ImportanceLevel::Negligible
        }
    }

    fn generate_reasons(&self, memory: &MemoryItem, recency: f64, frequency: f64) -> Vec<String> {
        let mut reasons = Vec::new();

        if recency > 0.8 {
            reasons.push("Recently accessed".to_string());
        }
        if frequency > 0.7 {
            reasons.push("Frequently recalled".to_string());
        }
        if memory.semantic_value > 0.8 {
            reasons.push("High semantic value".to_string());
        }
        if memory.project_value > 0.8 {
            reasons.push("Relevant to current project".to_string());
        }
        if memory.emotional_weight > 0.7 {
            reasons.push("Emotionally significant".to_string());
        }
        if memory.access_count == 0 {
            reasons.push("Never accessed".to_string());
        }

        reasons
    }
}

impl Default for MemoryImportanceEngine {
    fn default() -> Self {
        Self::new(ImportanceConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn create_test_memory(content: &str, semantic: f64, project: f64) -> MemoryItem {
        MemoryItem {
            id: Uuid::new_v4().to_string(),
            memory_type: MemoryType::Episodic,
            content: content.to_string(),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            access_count: 5,
            decay_rate: 0.0,
            semantic_value: semantic,
            project_value: project,
            emotional_weight: 0.5,
            tags: vec![],
            context: HashMap::new(),
        }
    }

    #[test]
    fn test_memory_importance_engine_creation() {
        let engine = MemoryImportanceEngine::new(ImportanceConfig::default());
        assert_eq!(engine.memory_count(), 0);
    }

    #[test]
    fn test_add_memory() {
        let mut engine = MemoryImportanceEngine::default();
        let memory = create_test_memory("test memory", 0.8, 0.7);
        engine.add_memory(memory);
        assert_eq!(engine.memory_count(), 1);
    }

    #[test]
    fn test_score_memory() {
        let engine = MemoryImportanceEngine::default();
        let memory = create_test_memory("important memory", 0.9, 0.8);
        let score = engine.score_memory(&memory);
        assert!(score.recall_probability > 0.5);
        assert!(!score.reasons.is_empty());
    }

    #[test]
    fn test_promotion_candidates() {
        let mut engine = MemoryImportanceEngine::default();
        engine.add_memory(create_test_memory("high value", 0.95, 0.9));
        engine.add_memory(create_test_memory("low value", 0.1, 0.1));

        let promoted = engine.get_memories_for_promotion();
        assert_eq!(promoted.len(), 1);
    }

    #[test]
    fn test_forgetting_candidates() {
        let mut engine = MemoryImportanceEngine::default();
        let mut low_memory = create_test_memory("forgotten", 0.05, 0.05);
        low_memory.access_count = 0;
        low_memory.last_accessed = Utc::now() - chrono::Duration::hours(100);
        engine.add_memory(low_memory);

        let forgotten = engine.get_memories_for_forgetting();
        assert_eq!(forgotten.len(), 1);
    }
}
