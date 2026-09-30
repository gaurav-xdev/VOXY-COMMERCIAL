use crate::error::Result;
use crate::graph::{GraphEdge, GraphNode, GraphQuery, KnowledgeGraph, NodeId};
use crate::hermes::{
    BehaviorAnalysis, ExperienceAnalysis, HabitDetection, HermesClassification, HermesEngine,
    PatternDetection, PreferenceExtraction, ReflectionLearning, RelationshipModel, SkillExtraction,
};
use crate::types::{ConsolidationPolicy, HermesDecision, MemoryItem};
use chrono::Utc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use voxy_world_model::context::WorldContext;

pub struct InMemoryKnowledgeGraph {
    nodes: RwLock<HashMap<String, GraphNode>>,
    edges: RwLock<HashMap<String, GraphEdge>>,
    max_nodes: usize,
    max_edges: usize,
}

const DEFAULT_MAX_NODES: usize = 10000;
const DEFAULT_MAX_EDGES: usize = 50000;

impl InMemoryKnowledgeGraph {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(HashMap::new()),
            max_nodes: DEFAULT_MAX_NODES,
            max_edges: DEFAULT_MAX_EDGES,
        }
    }

    pub fn with_limits(max_nodes: usize, max_edges: usize) -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(HashMap::new()),
            max_nodes,
            max_edges,
        }
    }
}

impl Default for InMemoryKnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl KnowledgeGraph for InMemoryKnowledgeGraph {
    async fn add_node(&self, node: GraphNode) -> Result<NodeId> {
        let id = node.id.0.clone();
        let mut nodes = self.nodes.write().await;
        nodes.insert(id.clone(), node);
        // Evict oldest nodes if at capacity
        if nodes.len() > self.max_nodes {
            let keys_to_remove: Vec<String> = nodes
                .keys()
                .take(nodes.len() - self.max_nodes)
                .cloned()
                .collect();
            for key in keys_to_remove {
                nodes.remove(&key);
            }
        }
        Ok(NodeId(id))
    }

    async fn get_node(&self, node_id: &NodeId) -> Result<GraphNode> {
        self.nodes
            .read()
            .await
            .get(&node_id.0)
            .cloned()
            .ok_or_else(|| {
                crate::error::MemoryError::GraphError(format!("Node not found: {}", node_id.0))
            })
    }

    async fn update_node(&self, node: GraphNode) -> Result<()> {
        self.nodes.write().await.insert(node.id.0.clone(), node);
        Ok(())
    }

    async fn delete_node(&self, node_id: &NodeId) -> Result<()> {
        self.nodes.write().await.remove(&node_id.0);
        let mut edges = self.edges.write().await;
        edges.retain(|_, e| e.source.0 != node_id.0 && e.target.0 != node_id.0);
        Ok(())
    }

    async fn node_exists(&self, node_id: &NodeId) -> bool {
        self.nodes.read().await.contains_key(&node_id.0)
    }

    async fn add_edge(&self, edge: GraphEdge) -> Result<String> {
        let id = edge.id.clone();
        let mut edges = self.edges.write().await;
        edges.insert(id.clone(), edge);
        // Evict oldest edges if at capacity
        if edges.len() > self.max_edges {
            let keys_to_remove: Vec<String> = edges
                .keys()
                .take(edges.len() - self.max_edges)
                .cloned()
                .collect();
            for key in keys_to_remove {
                edges.remove(&key);
            }
        }
        Ok(id)
    }

    async fn get_edge(&self, edge_id: &str) -> Result<GraphEdge> {
        self.edges
            .read()
            .await
            .get(edge_id)
            .cloned()
            .ok_or_else(|| {
                crate::error::MemoryError::GraphError(format!("Edge not found: {edge_id}"))
            })
    }

    async fn delete_edge(&self, edge_id: &str) -> Result<()> {
        self.edges.write().await.remove(edge_id);
        Ok(())
    }

    async fn query_graph(&self, query: &GraphQuery) -> Result<(Vec<GraphNode>, Vec<GraphEdge>)> {
        let nodes = self.nodes.read().await;
        let edges = self.edges.read().await;

        let filtered_nodes: Vec<GraphNode> = nodes
            .values()
            .filter(|n| {
                if let Some(ref types) = query.node_types {
                    types.iter().any(|t| t == &n.node_type)
                } else {
                    true
                }
            })
            .take(query.max_results)
            .cloned()
            .collect();

        let node_ids: Vec<String> = filtered_nodes.iter().map(|n| n.id.0.clone()).collect();
        let filtered_edges: Vec<GraphEdge> = edges
            .values()
            .filter(|e| {
                let src_match = query.source.as_ref().map_or(true, |s| s.0 == e.source.0);
                let tgt_match = query.target.as_ref().map_or(true, |t| t.0 == e.target.0);
                let type_match = query
                    .edge_types
                    .as_ref()
                    .map_or(true, |types| types.iter().any(|t| t == &e.edge_type));
                src_match
                    && tgt_match
                    && type_match
                    && (node_ids.contains(&e.source.0) || node_ids.contains(&e.target.0))
            })
            .take(query.max_results)
            .cloned()
            .collect();

        Ok((filtered_nodes, filtered_edges))
    }

    async fn find_path(
        &self,
        from: &NodeId,
        to: &NodeId,
        max_depth: usize,
    ) -> Result<Vec<Vec<GraphEdge>>> {
        let edges = self.edges.read().await;
        let mut paths = Vec::new();
        let mut stack: Vec<(Vec<GraphEdge>, String, std::collections::HashSet<String>)> =
            vec![(Vec::new(), from.0.clone(), std::collections::HashSet::new())];

        while let Some((path, current, mut visited)) = stack.pop() {
            if current == to.0 {
                paths.push(path);
                continue;
            }
            if path.len() >= max_depth || visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());
            for edge in edges.values() {
                if edge.source.0 == current && !path.iter().any(|e| e.id == edge.id) {
                    let mut new_path = path.clone();
                    new_path.push(edge.clone());
                    stack.push((new_path, edge.target.0.clone(), visited.clone()));
                }
            }
        }

        Ok(paths)
    }

    async fn get_neighbors(&self, node_id: &NodeId) -> Result<Vec<(GraphNode, GraphEdge)>> {
        let nodes = self.nodes.read().await;
        let edges = self.edges.read().await;
        let mut result = Vec::new();

        for edge in edges.values() {
            if edge.source.0 == node_id.0 {
                if let Some(node) = nodes.get(&edge.target.0) {
                    result.push((node.clone(), edge.clone()));
                }
            } else if edge.target.0 == node_id.0 {
                if let Some(node) = nodes.get(&edge.source.0) {
                    result.push((node.clone(), edge.clone()));
                }
            }
        }

        Ok(result)
    }

    async fn node_count(&self) -> usize {
        self.nodes.read().await.len()
    }

    async fn edge_count(&self) -> usize {
        self.edges.read().await.len()
    }

    async fn clear(&self) -> Result<()> {
        self.nodes.write().await.clear();
        self.edges.write().await.clear();
        Ok(())
    }
}

pub struct InMemoryHermesEngine;

impl InMemoryHermesEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InMemoryHermesEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl HermesEngine for InMemoryHermesEngine {
    async fn analyze_experience(
        &self,
        item: &MemoryItem,
        _context: Option<&WorldContext>,
    ) -> Result<ExperienceAnalysis> {
        let content_str = item.content.to_string().to_lowercase();
        let importance = item.importance;
        let novelty = if item.access_count == 0 { 1.0 } else { 0.3 };

        Ok(ExperienceAnalysis {
            experience_id: format!("exp-{}", item.id.0),
            summary: content_str.chars().take(200).collect(),
            emotional_tone: if importance > 0.7 {
                "positive"
            } else if importance < 0.3 {
                "negative"
            } else {
                "neutral"
            }
            .to_string(),
            key_elements: item.context_tags.clone(),
            importance,
            novelty,
            timestamp: item.timestamp,
        })
    }

    async fn extract_preferences(
        &self,
        items: &[MemoryItem],
        limit: usize,
    ) -> Result<Vec<PreferenceExtraction>> {
        Ok(items
            .iter()
            .take(limit)
            .filter(|i| i.importance > 0.5)
            .enumerate()
            .map(|(idx, item)| PreferenceExtraction {
                preference_id: format!("pref-{idx}"),
                category: item.source.clone(),
                value: item.content.to_string().chars().take(100).collect(),
                confidence: item.importance,
                evidence: item.context_tags.clone(),
                is_stable: item.access_count > 3,
            })
            .collect())
    }

    async fn detect_patterns(
        &self,
        items: &[MemoryItem],
        _window_days: u64,
    ) -> Result<Vec<PatternDetection>> {
        let mut source_counts: HashMap<String, usize> = HashMap::new();
        for item in items {
            *source_counts.entry(item.source.clone()).or_insert(0) += 1;
        }
        Ok(source_counts
            .into_iter()
            .filter(|(_, count)| *count > 2)
            .enumerate()
            .map(|(idx, (source, count))| PatternDetection {
                pattern_id: format!("pat-{idx}"),
                pattern_type: "source_frequency".to_string(),
                description: format!("Source '{source}' appeared {count} times"),
                frequency: (count as f64 / items.len().max(1) as f64).min(1.0),
                confidence: 0.7,
                supporting_evidence: vec![format!("{count} occurrences")],
            })
            .collect())
    }

    async fn detect_habits(
        &self,
        items: &[MemoryItem],
        _window_days: u64,
    ) -> Result<Vec<HabitDetection>> {
        Ok(items
            .iter()
            .filter(|i| i.access_count > 5)
            .enumerate()
            .map(|(idx, item)| HabitDetection {
                habit_id: format!("hab-{idx}"),
                trigger: "repeated_access".to_string(),
                action: item.content.to_string().chars().take(100).collect(),
                frequency: (item.access_count as f64 / 100.0).min(1.0),
                time_pattern: "recurring".to_string(),
                strength: (item.access_count as f64 / 20.0).min(1.0),
                is_automatic: item.access_count > 10,
            })
            .collect())
    }

    async fn model_relationship(
        &self,
        entity_a: &str,
        entity_b: &str,
        interactions: &[MemoryItem],
    ) -> Result<RelationshipModel> {
        Ok(RelationshipModel {
            relationship_id: format!("rel-{entity_a}-{entity_b}"),
            entity_a: entity_a.to_string(),
            entity_b: entity_b.to_string(),
            relationship_type: "interaction".to_string(),
            strength: (interactions.len() as f64 / 10.0).min(1.0),
            interaction_count: interactions.len() as u64,
            last_interaction: interactions
                .last()
                .map(|i| i.timestamp)
                .unwrap_or_else(Utc::now),
            sentiment: 0.5,
        })
    }

    async fn analyze_behavior(
        &self,
        items: &[MemoryItem],
        _window_days: u64,
    ) -> Result<Vec<BehaviorAnalysis>> {
        let mut behaviors = Vec::new();
        let mut type_counts: HashMap<String, usize> = HashMap::new();
        for item in items {
            *type_counts.entry(item.memory_type.to_string()).or_insert(0) += 1;
        }
        for (idx, (mem_type, count)) in type_counts.into_iter().enumerate() {
            behaviors.push(BehaviorAnalysis {
                behavior_id: format!("beh-{idx}"),
                behavior_type: mem_type,
                description: format!("Used {count} times"),
                frequency: (count as f64 / items.len().max(1) as f64).min(1.0),
                context: vec!["auto-detected".to_string()],
                impact: 0.5,
                is_positive: true,
            });
        }
        Ok(behaviors)
    }

    async fn extract_skills(&self, items: &[MemoryItem]) -> Result<Vec<SkillExtraction>> {
        Ok(items
            .iter()
            .filter(|i| i.memory_type == crate::types::MemoryType::Procedural)
            .enumerate()
            .map(|(idx, item)| SkillExtraction {
                skill_id: format!("sk-{idx}"),
                skill_name: item.source.clone(),
                proficiency: item.importance,
                practice_count: item.access_count,
                last_practiced: item.last_accessed,
                steps: item.context_tags.clone(),
                confidence: item.importance,
            })
            .collect())
    }

    async fn build_knowledge(
        &self,
        _items: &[MemoryItem],
        _graph: &dyn KnowledgeGraph,
    ) -> Result<usize> {
        Ok(0)
    }

    async fn reflect_on_task(
        &self,
        goal: &str,
        outcome: &str,
        steps: &[String],
        success: bool,
    ) -> Result<ReflectionLearning> {
        Ok(ReflectionLearning {
            reflection_id: format!("ref-{}", uuid::Uuid::new_v4()),
            task_id: format!("task-{}", uuid::Uuid::new_v4()),
            goal: goal.to_string(),
            outcome: outcome.to_string(),
            success_factors: if success {
                steps.to_vec()
            } else {
                vec!["unknown".to_string()]
            },
            failure_factors: if !success { steps.to_vec() } else { vec![] },
            lessons_learned: vec![format!(
                "Task '{}' completed with outcome: {outcome}",
                goal.chars().take(50).collect::<String>()
            )],
            pattern_updates: vec![],
            timestamp: Utc::now(),
        })
    }

    async fn classify(
        &self,
        item: &MemoryItem,
        analysis: &ExperienceAnalysis,
    ) -> Result<HermesClassification> {
        let decision = if analysis.importance > 0.7 {
            HermesDecision::LongTermMemory
        } else if analysis.importance > 0.4 {
            HermesDecision::TemporaryMemory
        } else {
            HermesDecision::Ignore
        };

        Ok(HermesClassification {
            item_id: item.id.clone(),
            decision,
            confidence: analysis.importance,
            reasons: vec![format!(
                "importance={:.2}, novelty={:.2}",
                analysis.importance, analysis.novelty
            )],
            timestamp: Utc::now(),
        })
    }

    async fn consolidate_long_term(
        &self,
        items: &[MemoryItem],
        policy: &ConsolidationPolicy,
    ) -> Result<Vec<HermesClassification>> {
        Ok(items
            .iter()
            .filter(|i| {
                i.importance >= policy.min_importance && i.access_count >= policy.min_access_count
            })
            .map(|item| HermesClassification {
                item_id: item.id.clone(),
                decision: HermesDecision::LongTermMemory,
                confidence: item.importance,
                reasons: vec!["meets consolidation criteria".to_string()],
                timestamp: Utc::now(),
            })
            .collect())
    }

    async fn evolve_memory(&self, item: &MemoryItem, _feedback: &str) -> Result<MemoryItem> {
        let mut evolved = item.clone();
        evolved.version += 1;
        evolved.last_accessed = Utc::now();
        evolved.access_count += 1;
        Ok(evolved)
    }
}
