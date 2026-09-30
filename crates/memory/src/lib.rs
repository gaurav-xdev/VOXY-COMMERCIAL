pub mod api;
pub mod compression;
pub mod config;
pub mod consolidation;
pub mod error;
pub mod event;
pub mod forgetting;
pub mod graph;
pub mod hermes;
pub mod in_memory_engines;
pub mod ltm_v2;
pub mod pipeline;
pub mod preferences;
pub mod ranking;
pub mod retrieval;
pub mod sqlite_engine;
pub mod stores;
pub mod synapse;
pub mod types;
pub mod versioning;

pub use api::{MemoryApi, MemoryStats};
pub use compression::CompressionEngine;
pub use config::MemoryConfig;
pub use consolidation::ConsolidationEngine;
pub use error::{MemoryError, Result};
pub use event::MemoryEvent;
pub use forgetting::ForgettingEngine;
pub use graph::{EdgeType, GraphEdge, GraphNode, GraphQuery, KnowledgeGraph, NodeId, NodeType};
pub use hermes::{
    BehaviorAnalysis, ExperienceAnalysis, HabitDetection, HermesClassification, HermesEngine,
    PatternDetection, PreferenceExtraction, ReflectionLearning, RelationshipModel, SkillExtraction,
};
pub use in_memory_engines::{InMemoryHermesEngine, InMemoryKnowledgeGraph};
pub use ltm_v2::{
    LongTermMemoryV2, MemoryCategory, MemoryItemV2, MemoryQueryResult, MemoryQueryV2,
    UserPreference,
};
pub use pipeline::{InMemoryMemoryPipeline, MemoryPhase, MemoryPipeline};
pub use preferences::{Preference, PreferenceStats, PreferenceTracker};
pub use ranking::{ImportanceScorer, MemoryRanker};
pub use retrieval::{RetrievalEngine, SearchResult};
pub use sqlite_engine::SqliteMemoryEngine;
pub use stores::{
    EpisodicMemory, ProceduralMemory, SemanticMemory, ShortTermMemory, VectorMemory, WorkingMemory,
};
pub use synapse::{MemorySynapse, SynapseStats};
pub use types::{
    CompressionPolicy, ConsolidationPolicy, ForgettingPolicy, HermesDecision, ImportanceFactors,
    ImportanceScore, MemoryId, MemoryItem, MemoryQuery, MemoryState, MemoryType, VersionInfo,
};
pub use versioning::MemoryVersioning;

pub mod prelude {
    pub use crate::api::{MemoryApi, MemoryStats};
    pub use crate::compression::CompressionEngine;
    pub use crate::config::MemoryConfig;
    pub use crate::consolidation::ConsolidationEngine;
    pub use crate::error::{MemoryError, Result};
    pub use crate::event::MemoryEvent;
    pub use crate::forgetting::ForgettingEngine;
    pub use crate::graph::{
        EdgeType, GraphEdge, GraphNode, GraphQuery, KnowledgeGraph, NodeId, NodeType,
    };
    pub use crate::hermes::{
        BehaviorAnalysis, ExperienceAnalysis, HabitDetection, HermesClassification, HermesEngine,
        PatternDetection, PreferenceExtraction, ReflectionLearning, RelationshipModel,
        SkillExtraction,
    };
    pub use crate::pipeline::{InMemoryMemoryPipeline, MemoryPhase, MemoryPipeline};
    pub use crate::preferences::{Preference, PreferenceStats, PreferenceTracker};
    pub use crate::ranking::{ImportanceScorer, MemoryRanker};
    pub use crate::retrieval::{RetrievalEngine, SearchResult};
    pub use crate::stores::{
        EpisodicMemory, ProceduralMemory, SemanticMemory, ShortTermMemory, VectorMemory,
        WorkingMemory,
    };
    pub use crate::synapse::{MemorySynapse, SynapseStats};
    pub use crate::types::{
        CompressionPolicy, ConsolidationPolicy, ForgettingPolicy, HermesDecision,
        ImportanceFactors, ImportanceScore, MemoryId, MemoryItem, MemoryQuery, MemoryState,
        MemoryType, VersionInfo,
    };
    pub use crate::versioning::MemoryVersioning;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    #[test]
    fn test_memory_id_creation_and_display() {
        let id = MemoryId("mem-001".to_string());
        assert_eq!(id.0, "mem-001");
        assert_eq!(format!("{}", id), "mem-001");
    }

    #[test]
    fn test_memory_type_variants_and_display() {
        assert_eq!(format!("{}", MemoryType::Working), "Working");
        assert_eq!(format!("{}", MemoryType::ShortTerm), "ShortTerm");
        assert_eq!(format!("{}", MemoryType::Episodic), "Episodic");
        assert_eq!(format!("{}", MemoryType::Semantic), "Semantic");
        assert_eq!(format!("{}", MemoryType::Procedural), "Procedural");
        assert_eq!(format!("{}", MemoryType::Vector), "Vector");
    }

    #[test]
    fn test_memory_state_variants() {
        assert_eq!(format!("{}", MemoryState::Active), "Active");
        assert_eq!(format!("{}", MemoryState::Dormant), "Dormant");
        assert_eq!(format!("{}", MemoryState::Compressed), "Compressed");
        assert_eq!(format!("{}", MemoryState::Archived), "Archived");
    }

    #[test]
    fn test_memory_item_creation() {
        let now = Utc::now();
        let item = MemoryItem {
            id: MemoryId("item-1".to_string()),
            memory_type: MemoryType::Working,
            state: MemoryState::Active,
            content: serde_json::json!({"text": "hello"}),
            importance: 0.8,
            timestamp: now,
            last_accessed: now,
            access_count: 1,
            context_tags: vec!["test".to_string()],
            source: "test".to_string(),
            version: 1,
            ttl: None,
            metadata: HashMap::new(),
            embedding: None,
            parent_id: None,
            related_ids: vec![],
        };
        assert_eq!(item.id.0, "item-1");
        assert_eq!(item.memory_type, MemoryType::Working);
        assert_eq!(item.state, MemoryState::Active);
        assert_eq!(item.importance, 0.8);
    }

    #[test]
    fn test_importance_factors_default() {
        let factors = ImportanceFactors::default();
        assert_eq!(factors.recency, 0.0);
        assert_eq!(factors.frequency, 0.0);
        assert_eq!(factors.relevance, 0.0);
        assert_eq!(factors.emotional_salience, 0.0);
        assert_eq!(factors.goal_alignment, 0.0);
        assert_eq!(factors.user_priority, 0.0);
        assert_eq!(factors.novelty, 0.0);
    }

    #[test]
    fn test_importance_score_creation() {
        let factors = ImportanceFactors {
            recency: 0.9,
            frequency: 0.5,
            relevance: 0.8,
            emotional_salience: 0.3,
            goal_alignment: 0.7,
            user_priority: 0.6,
            novelty: 0.4,
        };
        let score = ImportanceScore {
            overall: 0.75,
            factors: factors.clone(),
            explanations: vec!["high recency".to_string()],
        };
        assert_eq!(score.overall, 0.75);
        assert_eq!(score.factors.recency, 0.9);
        assert_eq!(score.explanations[0], "high recency");
    }

    #[test]
    fn test_memory_query_creation() {
        let query = MemoryQuery {
            query_text: Some("test query".to_string()),
            memory_types: Some(vec![MemoryType::Working, MemoryType::ShortTerm]),
            states: None,
            tags: Some(vec!["important".to_string()]),
            time_range: None,
            min_importance: Some(0.5),
            max_results: 20,
            include_embeddings: false,
            source_filter: None,
        };
        assert_eq!(query.query_text.unwrap(), "test query");
        assert_eq!(query.max_results, 20);
        assert!(!query.include_embeddings);
    }

    #[test]
    fn test_consolidation_policy_defaults() {
        let policy = ConsolidationPolicy {
            min_age_seconds: 3600,
            min_access_count: 3,
            min_importance: 0.6,
            max_items_per_run: 50,
        };
        assert_eq!(policy.min_age_seconds, 3600);
        assert_eq!(policy.min_access_count, 3);
        assert_eq!(policy.min_importance, 0.6);
        assert_eq!(policy.max_items_per_run, 50);
    }

    #[test]
    fn test_compression_policy_defaults() {
        let policy = CompressionPolicy {
            max_age_days: 30,
            min_importance_threshold: 0.3,
            compress_unaccessed_days: 14,
        };
        assert_eq!(policy.max_age_days, 30);
        assert_eq!(policy.min_importance_threshold, 0.3);
        assert_eq!(policy.compress_unaccessed_days, 14);
    }

    #[test]
    fn test_forgetting_policy_defaults() {
        let policy = ForgettingPolicy {
            active_to_dormant_days: 30,
            dormant_to_compressed_days: 60,
            compressed_to_archived_days: 90,
            archived_retention_days: 365,
            min_importance_to_keep: 0.5,
        };
        assert_eq!(policy.active_to_dormant_days, 30);
        assert_eq!(policy.dormant_to_compressed_days, 60);
        assert_eq!(policy.compressed_to_archived_days, 90);
        assert_eq!(policy.archived_retention_days, 365);
        assert_eq!(policy.min_importance_to_keep, 0.5);
    }

    #[test]
    fn test_hermes_decision_variants_and_display() {
        assert_eq!(format!("{}", HermesDecision::Ignore), "Ignore");
        assert_eq!(
            format!("{}", HermesDecision::TemporaryMemory),
            "TemporaryMemory"
        );
        assert_eq!(
            format!("{}", HermesDecision::LongTermMemory),
            "LongTermMemory"
        );
        assert_eq!(format!("{}", HermesDecision::Preference), "Preference");
        assert_eq!(format!("{}", HermesDecision::Habit), "Habit");
        assert_eq!(format!("{}", HermesDecision::Knowledge), "Knowledge");
        assert_eq!(
            format!("{}", HermesDecision::ProceduralSkill),
            "ProceduralSkill"
        );
    }

    #[test]
    fn test_node_id_creation() {
        let id = NodeId("node-001".to_string());
        assert_eq!(id.0, "node-001");
        assert_eq!(format!("{}", id), "node-001");
    }

    #[test]
    fn test_node_type_variants_and_display() {
        assert_eq!(format!("{}", NodeType::User), "User");
        assert_eq!(format!("{}", NodeType::Person), "Person");
        assert_eq!(format!("{}", NodeType::Device), "Device");
        assert_eq!(format!("{}", NodeType::Skill), "Skill");
        assert_eq!(format!("{}", NodeType::Preference), "Preference");
        assert_eq!(format!("{}", NodeType::Project), "Project");
        assert_eq!(format!("{}", NodeType::Task), "Task");
        assert_eq!(format!("{}", NodeType::Location), "Location");
        assert_eq!(format!("{}", NodeType::Application), "Application");
        assert_eq!(format!("{}", NodeType::Relationship), "Relationship");
        assert_eq!(format!("{}", NodeType::Organization), "Organization");
        assert_eq!(
            format!("{}", NodeType::Custom("test".to_string())),
            "Custom(test)"
        );
    }

    #[test]
    fn test_edge_type_variants_and_display() {
        assert_eq!(format!("{}", EdgeType::Uses), "Uses");
        assert_eq!(format!("{}", EdgeType::Owns), "Owns");
        assert_eq!(format!("{}", EdgeType::Likes), "Likes");
        assert_eq!(format!("{}", EdgeType::Dislikes), "Dislikes");
        assert_eq!(format!("{}", EdgeType::Created), "Created");
        assert_eq!(format!("{}", EdgeType::WorksOn), "WorksOn");
        assert_eq!(format!("{}", EdgeType::ConnectedTo), "ConnectedTo");
        assert_eq!(format!("{}", EdgeType::DependsOn), "DependsOn");
        assert_eq!(format!("{}", EdgeType::AssignedTo), "AssignedTo");
        assert_eq!(format!("{}", EdgeType::RelatedTo), "RelatedTo");
        assert_eq!(
            format!("{}", EdgeType::Custom("edge".to_string())),
            "Custom(edge)"
        );
    }

    #[test]
    fn test_graph_node_creation() {
        let now = Utc::now();
        let node = GraphNode {
            id: NodeId("node-1".to_string()),
            node_type: NodeType::User,
            name: "Test User".to_string(),
            properties: HashMap::new(),
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        };
        assert_eq!(node.id.0, "node-1");
        assert_eq!(node.name, "Test User");
        assert_eq!(node.node_type, NodeType::User);
    }

    #[test]
    fn test_graph_edge_creation() {
        let now = Utc::now();
        let edge = GraphEdge {
            id: "edge-1".to_string(),
            source: NodeId("node-a".to_string()),
            target: NodeId("node-b".to_string()),
            edge_type: EdgeType::Uses,
            weight: 1.0,
            properties: HashMap::new(),
            created_at: now,
        };
        assert_eq!(edge.id, "edge-1");
        assert_eq!(edge.source.0, "node-a");
        assert_eq!(edge.target.0, "node-b");
        assert_eq!(edge.weight, 1.0);
    }

    #[test]
    fn test_graph_query_creation() {
        let query = GraphQuery {
            source: Some(NodeId("src".to_string())),
            target: None,
            edge_types: Some(vec![EdgeType::Uses, EdgeType::Owns]),
            node_types: None,
            max_depth: Some(3),
            max_results: 50,
        };
        assert_eq!(query.source.unwrap().0, "src");
        assert_eq!(query.max_depth.unwrap(), 3);
        assert_eq!(query.max_results, 50);
    }

    #[test]
    fn test_search_result_creation() {
        let now = Utc::now();
        let item = MemoryItem {
            id: MemoryId("mem-1".to_string()),
            memory_type: MemoryType::Semantic,
            state: MemoryState::Active,
            content: serde_json::json!({"key": "value"}),
            importance: 0.9,
            timestamp: now,
            last_accessed: now,
            access_count: 5,
            context_tags: vec![],
            source: "test".to_string(),
            version: 1,
            ttl: None,
            metadata: HashMap::new(),
            embedding: None,
            parent_id: None,
            related_ids: vec![],
        };
        let result = SearchResult {
            item,
            score: 0.95,
            match_reasons: vec!["high importance".to_string()],
        };
        assert_eq!(result.score, 0.95);
        assert_eq!(result.match_reasons[0], "high importance");
    }

    #[test]
    fn test_version_info_creation() {
        let now = Utc::now();
        let info = VersionInfo {
            version: 2,
            created_at: now,
            changes: vec!["updated content".to_string()],
            previous_id: Some(MemoryId("v1".to_string())),
        };
        assert_eq!(info.version, 2);
        assert_eq!(info.changes[0], "updated content");
        assert!(info.previous_id.is_some());
    }

    #[test]
    fn test_experience_analysis_creation() {
        let now = Utc::now();
        let analysis = ExperienceAnalysis {
            experience_id: "exp-1".to_string(),
            summary: "User greeted the system".to_string(),
            emotional_tone: "positive".to_string(),
            key_elements: vec!["greeting".to_string()],
            importance: 0.6,
            novelty: 0.3,
            timestamp: now,
        };
        assert_eq!(analysis.experience_id, "exp-1");
        assert_eq!(analysis.emotional_tone, "positive");
        assert_eq!(analysis.importance, 0.6);
    }

    #[test]
    fn test_preference_extraction_creation() {
        let pref = PreferenceExtraction {
            preference_id: "pref-1".to_string(),
            category: "theme".to_string(),
            value: "dark".to_string(),
            confidence: 0.85,
            evidence: vec!["user set dark mode".to_string()],
            is_stable: true,
        };
        assert_eq!(pref.category, "theme");
        assert_eq!(pref.value, "dark");
        assert!(pref.is_stable);
    }

    #[test]
    fn test_pattern_detection_creation() {
        let pattern = PatternDetection {
            pattern_id: "pat-1".to_string(),
            pattern_type: "usage".to_string(),
            description: "User opens browser at 9am".to_string(),
            frequency: 0.9,
            confidence: 0.8,
            supporting_evidence: vec!["observed 10 times".to_string()],
        };
        assert_eq!(pattern.pattern_type, "usage");
        assert_eq!(pattern.frequency, 0.9);
        assert_eq!(pattern.confidence, 0.8);
    }

    #[test]
    fn test_habit_detection_creation() {
        let habit = HabitDetection {
            habit_id: "hab-1".to_string(),
            trigger: "morning".to_string(),
            action: "check email".to_string(),
            frequency: 0.95,
            time_pattern: "daily".to_string(),
            strength: 0.8,
            is_automatic: true,
        };
        assert_eq!(habit.trigger, "morning");
        assert_eq!(habit.action, "check email");
        assert!(habit.is_automatic);
    }

    #[test]
    fn test_relationship_model_creation() {
        let now = Utc::now();
        let rel = RelationshipModel {
            relationship_id: "rel-1".to_string(),
            entity_a: "Alice".to_string(),
            entity_b: "Bob".to_string(),
            relationship_type: "friend".to_string(),
            strength: 0.7,
            interaction_count: 42,
            last_interaction: now,
            sentiment: 0.9,
        };
        assert_eq!(rel.entity_a, "Alice");
        assert_eq!(rel.entity_b, "Bob");
        assert_eq!(rel.interaction_count, 42);
    }

    #[test]
    fn test_behavior_analysis_creation() {
        let behavior = BehaviorAnalysis {
            behavior_id: "beh-1".to_string(),
            behavior_type: "productivity".to_string(),
            description: "Uses VS Code daily".to_string(),
            frequency: 0.8,
            context: vec!["work".to_string()],
            impact: 0.6,
            is_positive: true,
        };
        assert_eq!(behavior.behavior_type, "productivity");
        assert!(behavior.is_positive);
        assert_eq!(behavior.impact, 0.6);
    }

    #[test]
    fn test_skill_extraction_creation() {
        let now = Utc::now();
        let skill = SkillExtraction {
            skill_id: "sk-1".to_string(),
            skill_name: "Python".to_string(),
            proficiency: 0.7,
            practice_count: 150,
            last_practiced: now,
            steps: vec!["install".to_string(), "write code".to_string()],
            confidence: 0.8,
        };
        assert_eq!(skill.skill_name, "Python");
        assert_eq!(skill.practice_count, 150);
        assert_eq!(skill.steps.len(), 2);
    }

    #[test]
    fn test_reflection_learning_creation() {
        let now = Utc::now();
        let reflection = ReflectionLearning {
            reflection_id: "ref-1".to_string(),
            task_id: "task-1".to_string(),
            goal: "complete project".to_string(),
            outcome: "success".to_string(),
            success_factors: vec!["planning".to_string()],
            failure_factors: vec![],
            lessons_learned: vec!["start early".to_string()],
            pattern_updates: vec![],
            timestamp: now,
        };
        assert_eq!(reflection.goal, "complete project");
        assert_eq!(reflection.outcome, "success");
        assert_eq!(reflection.lessons_learned[0], "start early");
    }

    #[test]
    fn test_hermes_classification_creation() {
        let now = Utc::now();
        let classification = HermesClassification {
            item_id: MemoryId("mem-1".to_string()),
            decision: HermesDecision::LongTermMemory,
            confidence: 0.9,
            reasons: vec!["high importance".to_string()],
            timestamp: now,
        };
        assert_eq!(classification.item_id.0, "mem-1");
        assert_eq!(classification.decision, HermesDecision::LongTermMemory);
        assert_eq!(classification.confidence, 0.9);
    }

    #[test]
    fn test_memory_stats_creation() {
        let stats = MemoryStats {
            total_items: 100,
            working_count: 5,
            short_term_count: 20,
            episodic_count: 30,
            semantic_count: 25,
            procedural_count: 10,
            vector_count: 10,
            graph_nodes: 50,
            graph_edges: 80,
            active_count: 60,
            dormant_count: 25,
            compressed_count: 10,
            archived_count: 5,
        };
        assert_eq!(stats.total_items, 100);
        assert_eq!(stats.graph_nodes, 50);
        assert_eq!(stats.active_count, 60);
    }

    #[test]
    fn test_memory_phase_variants() {
        assert_eq!(format!("{:?}", MemoryPhase::Input), "Input");
        assert_eq!(
            format!("{:?}", MemoryPhase::ImportanceAnalysis),
            "ImportanceAnalysis"
        );
        assert_eq!(
            format!("{:?}", MemoryPhase::HermesAnalysis),
            "HermesAnalysis"
        );
        assert_eq!(
            format!("{:?}", MemoryPhase::Classification),
            "Classification"
        );
        assert_eq!(format!("{:?}", MemoryPhase::Storage), "Storage");
        assert_eq!(format!("{:?}", MemoryPhase::Indexing), "Indexing");
        assert_eq!(format!("{:?}", MemoryPhase::Retrieval), "Retrieval");
        assert_eq!(format!("{:?}", MemoryPhase::Reflection), "Reflection");
        assert_eq!(format!("{:?}", MemoryPhase::Learning), "Learning");
    }

    #[test]
    fn test_memory_event_display_item_stored() {
        let event = MemoryEvent::ItemStored {
            memory_id: "mem-1".to_string(),
            memory_type: "Working".to_string(),
            importance: 0.8,
        };
        let display = format!("{}", event);
        assert!(display.contains("ItemStored"));
        assert!(display.contains("mem-1"));
        assert!(display.contains("0.8"));
    }

    #[test]
    fn test_memory_event_display_consolidation_run() {
        let event = MemoryEvent::ConsolidationRun {
            items_processed: 10,
            duration_ms: 500,
        };
        let display = format!("{}", event);
        assert!(display.contains("ConsolidationRun"));
        assert!(display.contains("10"));
        assert!(display.contains("500"));
    }

    #[test]
    fn test_memory_event_display_forgetting_run() {
        let event = MemoryEvent::ForgettingRun {
            items_affected: 5,
            duration_ms: 200,
        };
        let display = format!("{}", event);
        assert!(display.contains("ForgettingRun"));
        assert!(display.contains("5"));
        assert!(display.contains("200"));
    }

    #[test]
    fn test_memory_error_display_invalid_config() {
        let err = MemoryError::InvalidConfig("bad config".to_string());
        assert_eq!(format!("{}", err), "Invalid configuration: bad config");
    }

    #[test]
    fn test_memory_error_display_store_error() {
        let err = MemoryError::StoreError("disk full".to_string());
        assert_eq!(format!("{}", err), "Store error: disk full");
    }

    #[test]
    fn test_memory_error_display_item_not_found() {
        let err = MemoryError::ItemNotFound("mem-42".to_string());
        assert_eq!(format!("{}", err), "Item not found: mem-42");
    }

    #[test]
    fn test_memory_error_display_timeout() {
        let err = MemoryError::Timeout("operation timed out".to_string());
        assert_eq!(format!("{}", err), "Timeout: operation timed out");
    }
}
