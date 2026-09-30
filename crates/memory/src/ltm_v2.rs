//! Long Term Memory V2 — project memory, user preferences, relationship graph.
//!
//! Extends the existing memory system with:
//! - Project memory (files, decisions, context)
//! - User preference memory (learned patterns)
//! - Relationship graph (entity connections)
//! - Memory importance scoring
//! - Forgetting algorithm
//! - Memory compression
//! - Cross-memory links

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Core Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryId(pub String);

impl Default for MemoryId {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

/// Memory categories.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryCategory {
    /// Project-specific memory (files, decisions, context).
    Project,
    /// User preferences and learned patterns.
    UserPreference,
    /// Relationship between entities.
    Relationship,
    /// Episodic memory (events, experiences).
    Episodic,
    /// Semantic memory (facts, knowledge).
    Semantic,
    /// Procedural memory (how to do things).
    Procedural,
}

/// Memory importance factors.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportanceFactors {
    pub recency: f64,
    pub frequency: f64,
    pub relevance: f64,
    pub emotional_salience: f64,
    pub goal_alignment: f64,
    pub user_priority: f64,
    pub novelty: f64,
}

impl ImportanceFactors {
    pub fn weighted_sum(&self) -> f64 {
        self.recency * 0.2
            + self.frequency * 0.15
            + self.relevance * 0.25
            + self.emotional_salience * 0.1
            + self.goal_alignment * 0.15
            + self.user_priority * 0.1
            + self.novelty * 0.05
    }
}

/// A memory item with full metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItemV2 {
    pub id: MemoryId,
    pub category: MemoryCategory,
    pub content: String,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub importance: ImportanceFactors,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_accessed: chrono::DateTime<chrono::Utc>,
    pub access_count: u64,
    pub version: u64,
    pub compressed: bool,
    pub archived: bool,
    pub project_id: Option<String>,
    pub related_memory_ids: Vec<MemoryId>,
    pub metadata: HashMap<String, String>,
}

/// A relationship between two entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub id: String,
    pub from_id: MemoryId,
    pub to_id: MemoryId,
    pub relationship_type: String,
    pub strength: f64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, String>,
}

/// A user preference learned from behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreference {
    pub id: MemoryId,
    pub key: String,
    pub value: String,
    pub confidence: f64,
    pub learned_from: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub times_observed: u32,
}

/// Memory query.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryQueryV2 {
    pub text: Option<String>,
    pub categories: Option<Vec<MemoryCategory>>,
    pub tags: Option<Vec<String>>,
    pub min_importance: Option<f64>,
    pub max_results: usize,
    pub include_archived: bool,
    pub project_filter: Option<String>,
}

/// Memory query result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryQueryResult {
    pub items: Vec<MemoryItemV2>,
    pub total_count: usize,
    pub query_time_ms: u64,
}

// ============================================================================
// Long Term Memory V2
// ============================================================================

/// Extended long-term memory with project, preference, and relationship support.
pub struct LongTermMemoryV2 {
    memories: HashMap<MemoryId, MemoryItemV2>,
    relationships: Vec<Relationship>,
    preferences: Vec<UserPreference>,
    max_memories: usize,
    max_relationships: usize,
    max_preferences: usize,
    forgetting_threshold: f64,
}

impl LongTermMemoryV2 {
    pub fn new(
        max_memories: usize,
        max_relationships: usize,
        max_preferences: usize,
        forgetting_threshold: f64,
    ) -> Self {
        Self {
            memories: HashMap::new(),
            relationships: Vec::new(),
            preferences: Vec::new(),
            max_memories,
            max_relationships,
            max_preferences,
            forgetting_threshold,
        }
    }

    pub fn default_memory() -> Self {
        Self::new(10000, 5000, 500, 0.1)
    }

    /// Store a memory item.
    pub fn store(&mut self, mut item: MemoryItemV2) -> MemoryId {
        if self.memories.len() >= self.max_memories {
            self.forget_least_important();
        }

        let id = item.id.clone();
        // Don't override last_accessed if it was explicitly set
        if item.last_accessed == item.created_at {
            item.last_accessed = chrono::Utc::now();
        }
        self.memories.insert(id.clone(), item);
        id
    }

    /// Retrieve a memory by ID.
    pub fn get(&self, id: &MemoryId) -> Option<&MemoryItemV2> {
        self.memories.get(id)
    }

    /// Query memories.
    pub fn query(&self, query: &MemoryQueryV2) -> MemoryQueryResult {
        let start = std::time::Instant::now();

        let mut results: Vec<MemoryItemV2> = self
            .memories
            .values()
            .filter(|m| {
                if !query.include_archived && m.archived {
                    return false;
                }
                if let Some(ref categories) = query.categories {
                    if !categories.contains(&m.category) {
                        return false;
                    }
                }
                if let Some(ref tags) = query.tags {
                    if !tags.iter().any(|t| m.tags.contains(t)) {
                        return false;
                    }
                }
                if let Some(min_imp) = query.min_importance {
                    if m.importance.weighted_sum() < min_imp {
                        return false;
                    }
                }
                if let Some(ref project) = query.project_filter {
                    if m.project_id.as_ref() != Some(project) {
                        return false;
                    }
                }
                if let Some(ref text) = query.text {
                    if !m.content.to_lowercase().contains(&text.to_lowercase()) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        // Sort by importance
        results.sort_by(|a, b| {
            b.importance
                .weighted_sum()
                .partial_cmp(&a.importance.weighted_sum())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total = results.len();
        if query.max_results > 0 {
            results.truncate(query.max_results);
        }

        let elapsed = start.elapsed().as_millis() as u64;

        MemoryQueryResult {
            items: results,
            total_count: total,
            query_time_ms: elapsed,
        }
    }

    /// Add a relationship between two memories.
    pub fn add_relationship(&mut self, relationship: Relationship) -> Result<(), MemoryError> {
        if self.relationships.len() >= self.max_relationships {
            return Err(MemoryError::CapacityReached(self.max_relationships));
        }
        self.relationships.push(relationship);
        Ok(())
    }

    /// Get relationships for a memory.
    pub fn relationships_for(&self, memory_id: &MemoryId) -> Vec<&Relationship> {
        self.relationships
            .iter()
            .filter(|r| r.from_id == *memory_id || r.to_id == *memory_id)
            .collect()
    }

    /// Store a user preference.
    pub fn store_preference(&mut self, preference: UserPreference) -> Result<(), MemoryError> {
        // Update if exists, otherwise add
        if let Some(existing) = self
            .preferences
            .iter_mut()
            .find(|p| p.key == preference.key)
        {
            existing.value = preference.value;
            existing.confidence = preference.confidence;
            existing.updated_at = chrono::Utc::now();
            existing.times_observed += 1;
        } else {
            if self.preferences.len() >= self.max_preferences {
                return Err(MemoryError::CapacityReached(self.max_preferences));
            }
            self.preferences.push(preference);
        }
        Ok(())
    }

    /// Get a preference by key.
    pub fn get_preference(&self, key: &str) -> Option<&UserPreference> {
        self.preferences.iter().find(|p| p.key == key)
    }

    /// Get all preferences.
    pub fn all_preferences(&self) -> &[UserPreference] {
        &self.preferences
    }

    /// Run forgetting algorithm.
    pub fn forget(&mut self) {
        let now = chrono::Utc::now();
        for item in self.memories.values_mut() {
            let days_since_access = (now - item.last_accessed).num_days() as f64;
            let importance = item.importance.weighted_sum();

            // If below threshold and old enough, archive
            if importance < self.forgetting_threshold && days_since_access > 30.0 {
                item.archived = true;
            }

            // Compress old memories
            if days_since_access > 60.0 && !item.compressed {
                item.compressed = true;
                if item.summary.is_none() {
                    item.summary = Some(item.content.chars().take(100).collect::<String>() + "...");
                }
            }
        }
    }

    /// Get all memories.
    pub fn all(&self) -> Vec<&MemoryItemV2> {
        self.memories.values().collect()
    }

    /// Get memory count.
    pub fn count(&self) -> usize {
        self.memories.len()
    }

    /// Get relationship count.
    pub fn relationship_count(&self) -> usize {
        self.relationships.len()
    }

    fn forget_least_important(&mut self) {
        if let Some(min_id) = self
            .memories
            .iter()
            .min_by_key(|(_, m)| (m.importance.weighted_sum() * 1000.0) as u64)
            .map(|(id, _)| id.clone())
        {
            self.memories.remove(&min_id);
        }
    }
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, Clone, thiserror::Error)]
pub enum MemoryError {
    #[error("Capacity reached: {0}")]
    CapacityReached(usize),

    #[error("Memory not found: {0}")]
    NotFound(String),
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item(content: &str, category: MemoryCategory) -> MemoryItemV2 {
        MemoryItemV2 {
            id: MemoryId::new(),
            category,
            content: content.to_string(),
            summary: None,
            tags: Vec::new(),
            importance: ImportanceFactors::default(),
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            access_count: 0,
            version: 1,
            compressed: false,
            archived: false,
            project_id: None,
            related_memory_ids: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn memory_creation() {
        let _mem = LongTermMemoryV2::default_memory();
    }

    #[test]
    fn store_and_retrieve() {
        let mut mem = LongTermMemoryV2::default_memory();
        let item = sample_item("Test memory", MemoryCategory::Semantic);
        let id = mem.store(item);
        assert!(mem.get(&id).is_some());
    }

    #[test]
    fn query_by_category() {
        let mut mem = LongTermMemoryV2::default_memory();
        mem.store(sample_item("Project item", MemoryCategory::Project));
        mem.store(sample_item("Preference", MemoryCategory::UserPreference));

        let query = MemoryQueryV2 {
            categories: Some(vec![MemoryCategory::Project]),
            max_results: 10,
            ..Default::default()
        };
        let result = mem.query(&query);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].category, MemoryCategory::Project);
    }

    #[test]
    fn query_by_text() {
        let mut mem = LongTermMemoryV2::default_memory();
        mem.store(sample_item("Rust is great", MemoryCategory::Semantic));
        mem.store(sample_item("Python is okay", MemoryCategory::Semantic));

        let query = MemoryQueryV2 {
            text: Some("Rust".to_string()),
            max_results: 10,
            ..Default::default()
        };
        let result = mem.query(&query);
        assert_eq!(result.items.len(), 1);
    }

    #[test]
    fn add_relationship() {
        let mut mem = LongTermMemoryV2::default_memory();
        let id1 = mem.store(sample_item("A", MemoryCategory::Semantic));
        let id2 = mem.store(sample_item("B", MemoryCategory::Semantic));

        let rel = Relationship {
            id: "rel1".to_string(),
            from_id: id1.clone(),
            to_id: id2.clone(),
            relationship_type: "related_to".to_string(),
            strength: 0.8,
            created_at: chrono::Utc::now(),
            metadata: HashMap::new(),
        };
        mem.add_relationship(rel).unwrap();

        let rels = mem.relationships_for(&id1);
        assert_eq!(rels.len(), 1);
    }

    #[test]
    fn store_preference() {
        let mut mem = LongTermMemoryV2::default_memory();
        let pref = UserPreference {
            id: MemoryId::new(),
            key: "response_length".to_string(),
            value: "concise".to_string(),
            confidence: 0.8,
            learned_from: vec!["user corrections".to_string()],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            times_observed: 1,
        };
        mem.store_preference(pref).unwrap();
        assert_eq!(mem.all_preferences().len(), 1);
    }

    #[test]
    fn update_preference() {
        let mut mem = LongTermMemoryV2::default_memory();
        let pref = UserPreference {
            id: MemoryId::new(),
            key: "style".to_string(),
            value: "formal".to_string(),
            confidence: 0.5,
            learned_from: Vec::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            times_observed: 1,
        };
        mem.store_preference(pref).unwrap();

        let pref2 = UserPreference {
            id: MemoryId::new(),
            key: "style".to_string(),
            value: "casual".to_string(),
            confidence: 0.7,
            learned_from: Vec::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            times_observed: 1,
        };
        mem.store_preference(pref2).unwrap();
        assert_eq!(mem.all_preferences().len(), 1);
        assert_eq!(mem.get_preference("style").unwrap().value, "casual");
    }

    #[test]
    fn forgetting_algorithm() {
        let mut mem = LongTermMemoryV2::default_memory();
        let mut item = sample_item("Old memory", MemoryCategory::Semantic);
        item.last_accessed = chrono::Utc::now() - chrono::Duration::days(90);
        item.importance = ImportanceFactors {
            recency: 0.0,
            frequency: 0.0,
            relevance: 0.0,
            ..Default::default()
        };
        mem.store(item);

        mem.forget();
        let items = mem.all();
        assert!(items[0].archived || items[0].compressed);
    }

    #[test]
    fn importance_scoring() {
        let factors = ImportanceFactors {
            recency: 0.8,
            frequency: 0.5,
            relevance: 0.9,
            emotional_salience: 0.3,
            goal_alignment: 0.7,
            user_priority: 0.6,
            novelty: 0.4,
        };
        let score = factors.weighted_sum();
        assert!(score > 0.0 && score < 1.0);
    }

    #[test]
    fn memory_count() {
        let mut mem = LongTermMemoryV2::default_memory();
        assert_eq!(mem.count(), 0);
        mem.store(sample_item("A", MemoryCategory::Semantic));
        mem.store(sample_item("B", MemoryCategory::Semantic));
        assert_eq!(mem.count(), 2);
    }

    #[test]
    fn archived_not_in_query() {
        let mut mem = LongTermMemoryV2::default_memory();
        let mut item = sample_item("Archived", MemoryCategory::Semantic);
        item.archived = true;
        mem.store(item);
        mem.store(sample_item("Active", MemoryCategory::Semantic));

        let query = MemoryQueryV2 {
            include_archived: false,
            max_results: 10,
            ..Default::default()
        };
        let result = mem.query(&query);
        assert_eq!(result.items.len(), 1);
    }

    #[test]
    fn project_filter() {
        let mut mem = LongTermMemoryV2::default_memory();
        let mut item1 = sample_item("A", MemoryCategory::Project);
        item1.project_id = Some("voxy".to_string());
        mem.store(item1);
        let mut item2 = sample_item("B", MemoryCategory::Project);
        item2.project_id = Some("other".to_string());
        mem.store(item2);

        let query = MemoryQueryV2 {
            project_filter: Some("voxy".to_string()),
            max_results: 10,
            ..Default::default()
        };
        let result = mem.query(&query);
        assert_eq!(result.items.len(), 1);
    }
}
