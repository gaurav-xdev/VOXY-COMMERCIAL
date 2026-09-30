use crate::api::{MemoryApi, MemoryStats};
use crate::config::MemoryConfig;
use crate::error::{MemoryError, Result};
use crate::graph::{GraphEdge, GraphNode, GraphQuery, KnowledgeGraph, NodeId};
use crate::hermes::{
    BehaviorAnalysis, ExperienceAnalysis, HabitDetection, HermesClassification, HermesEngine,
    PatternDetection, PreferenceExtraction, ReflectionLearning, RelationshipModel, SkillExtraction,
};
use crate::retrieval::SearchResult;
use crate::types::{
    ConsolidationPolicy, HermesDecision, MemoryId, MemoryItem, MemoryQuery, MemoryState, MemoryType,
};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::sync::OnceLock;
use tokio::sync::Mutex;
use tracing::{debug, info};
use voxy_world_model::context::WorldContext;

// ---------------------------------------------------------------------------
// Noop KnowledgeGraph — used by SqliteMemoryEngine which doesn't support graph
// ---------------------------------------------------------------------------

struct NoopKnowledgeGraph;

#[async_trait::async_trait]
impl KnowledgeGraph for NoopKnowledgeGraph {
    async fn add_node(&self, _node: GraphNode) -> Result<NodeId> {
        Err(MemoryError::GraphError(
            "SqliteMemoryEngine does not support KnowledgeGraph — use InMemoryKnowledgeGraph"
                .into(),
        ))
    }
    async fn get_node(&self, _node_id: &NodeId) -> Result<GraphNode> {
        Err(MemoryError::GraphError(
            "SqliteMemoryEngine does not support KnowledgeGraph".into(),
        ))
    }
    async fn update_node(&self, _node: GraphNode) -> Result<()> {
        Err(MemoryError::GraphError(
            "SqliteMemoryEngine does not support KnowledgeGraph".into(),
        ))
    }
    async fn delete_node(&self, _node_id: &NodeId) -> Result<()> {
        Err(MemoryError::GraphError(
            "SqliteMemoryEngine does not support KnowledgeGraph".into(),
        ))
    }
    async fn node_exists(&self, _node_id: &NodeId) -> bool {
        false
    }
    async fn add_edge(&self, _edge: GraphEdge) -> Result<String> {
        Err(MemoryError::GraphError(
            "SqliteMemoryEngine does not support KnowledgeGraph".into(),
        ))
    }
    async fn get_edge(&self, _edge_id: &str) -> Result<GraphEdge> {
        Err(MemoryError::GraphError(
            "SqliteMemoryEngine does not support KnowledgeGraph".into(),
        ))
    }
    async fn delete_edge(&self, _edge_id: &str) -> Result<()> {
        Err(MemoryError::GraphError(
            "SqliteMemoryEngine does not support KnowledgeGraph".into(),
        ))
    }
    async fn query_graph(&self, _query: &GraphQuery) -> Result<(Vec<GraphNode>, Vec<GraphEdge>)> {
        Err(MemoryError::GraphError(
            "SqliteMemoryEngine does not support KnowledgeGraph".into(),
        ))
    }
    async fn find_path(
        &self,
        _from: &NodeId,
        _to: &NodeId,
        _max_depth: usize,
    ) -> Result<Vec<Vec<GraphEdge>>> {
        Err(MemoryError::GraphError(
            "SqliteMemoryEngine does not support KnowledgeGraph".into(),
        ))
    }
    async fn get_neighbors(&self, _node_id: &NodeId) -> Result<Vec<(GraphNode, GraphEdge)>> {
        Err(MemoryError::GraphError(
            "SqliteMemoryEngine does not support KnowledgeGraph".into(),
        ))
    }
    async fn node_count(&self) -> usize {
        0
    }
    async fn edge_count(&self) -> usize {
        0
    }
    async fn clear(&self) -> Result<()> {
        Err(MemoryError::GraphError(
            "SqliteMemoryEngine does not support KnowledgeGraph".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Noop HermesEngine — used by SqliteMemoryEngine which doesn't support Hermes
// ---------------------------------------------------------------------------

struct NoopHermesEngine;

#[async_trait::async_trait]
impl HermesEngine for NoopHermesEngine {
    async fn analyze_experience(
        &self,
        _item: &MemoryItem,
        _context: Option<&WorldContext>,
    ) -> Result<ExperienceAnalysis> {
        Err(MemoryError::HermesError(
            "SqliteMemoryEngine does not support HermesEngine — use InMemoryHermesEngine".into(),
        ))
    }
    async fn extract_preferences(
        &self,
        _items: &[MemoryItem],
        _limit: usize,
    ) -> Result<Vec<PreferenceExtraction>> {
        Err(MemoryError::HermesError(
            "SqliteMemoryEngine does not support HermesEngine".into(),
        ))
    }
    async fn detect_patterns(
        &self,
        _items: &[MemoryItem],
        _window_days: u64,
    ) -> Result<Vec<PatternDetection>> {
        Err(MemoryError::HermesError(
            "SqliteMemoryEngine does not support HermesEngine".into(),
        ))
    }
    async fn detect_habits(
        &self,
        _items: &[MemoryItem],
        _window_days: u64,
    ) -> Result<Vec<HabitDetection>> {
        Err(MemoryError::HermesError(
            "SqliteMemoryEngine does not support HermesEngine".into(),
        ))
    }
    async fn model_relationship(
        &self,
        _a: &str,
        _b: &str,
        _interactions: &[MemoryItem],
    ) -> Result<RelationshipModel> {
        Err(MemoryError::HermesError(
            "SqliteMemoryEngine does not support HermesEngine".into(),
        ))
    }
    async fn analyze_behavior(
        &self,
        _items: &[MemoryItem],
        _window_days: u64,
    ) -> Result<Vec<BehaviorAnalysis>> {
        Err(MemoryError::HermesError(
            "SqliteMemoryEngine does not support HermesEngine".into(),
        ))
    }
    async fn extract_skills(&self, _items: &[MemoryItem]) -> Result<Vec<SkillExtraction>> {
        Err(MemoryError::HermesError(
            "SqliteMemoryEngine does not support HermesEngine".into(),
        ))
    }
    async fn build_knowledge(
        &self,
        _items: &[MemoryItem],
        _graph: &dyn KnowledgeGraph,
    ) -> Result<usize> {
        Err(MemoryError::HermesError(
            "SqliteMemoryEngine does not support HermesEngine".into(),
        ))
    }
    async fn reflect_on_task(
        &self,
        _goal: &str,
        _outcome: &str,
        _steps: &[String],
        _success: bool,
    ) -> Result<ReflectionLearning> {
        Err(MemoryError::HermesError(
            "SqliteMemoryEngine does not support HermesEngine".into(),
        ))
    }
    async fn classify(
        &self,
        _item: &MemoryItem,
        _analysis: &ExperienceAnalysis,
    ) -> Result<HermesClassification> {
        Err(MemoryError::HermesError(
            "SqliteMemoryEngine does not support HermesEngine".into(),
        ))
    }
    async fn consolidate_long_term(
        &self,
        _items: &[MemoryItem],
        _policy: &ConsolidationPolicy,
    ) -> Result<Vec<HermesClassification>> {
        Err(MemoryError::HermesError(
            "SqliteMemoryEngine does not support HermesEngine".into(),
        ))
    }
    async fn evolve_memory(&self, _item: &MemoryItem, _feedback: &str) -> Result<MemoryItem> {
        Err(MemoryError::HermesError(
            "SqliteMemoryEngine does not support HermesEngine".into(),
        ))
    }
}

fn noop_graph() -> &'static NoopKnowledgeGraph {
    static GRAPH: OnceLock<NoopKnowledgeGraph> = OnceLock::new();
    GRAPH.get_or_init(|| NoopKnowledgeGraph)
}

fn noop_hermes() -> &'static NoopHermesEngine {
    static HERMES: OnceLock<NoopHermesEngine> = OnceLock::new();
    HERMES.get_or_init(|| NoopHermesEngine)
}

pub struct SqliteMemoryEngine {
    conn: Mutex<Option<Connection>>,
    #[allow(dead_code)]
    config: MemoryConfig,
}

impl SqliteMemoryEngine {
    pub fn new() -> Self {
        Self {
            conn: Mutex::new(None),
            config: MemoryConfig::default(),
        }
    }

    pub fn with_config(config: MemoryConfig) -> Self {
        Self {
            conn: Mutex::new(None),
            config,
        }
    }

    async fn with_conn<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Connection) -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let conn_guard = self.conn.lock().await;
        let conn = conn_guard
            .as_ref()
            .ok_or_else(|| MemoryError::StoreError("Database not initialized".into()))?;
        f(conn)
    }

    fn create_tables(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                memory_type TEXT NOT NULL,
                state TEXT NOT NULL DEFAULT 'Active',
                content TEXT NOT NULL,
                importance REAL NOT NULL DEFAULT 0.5,
                timestamp TEXT NOT NULL,
                last_accessed TEXT NOT NULL,
                access_count INTEGER NOT NULL DEFAULT 0,
                context_tags TEXT NOT NULL DEFAULT '[]',
                source TEXT NOT NULL DEFAULT '',
                version INTEGER NOT NULL DEFAULT 1,
                ttl_seconds INTEGER,
                metadata TEXT NOT NULL DEFAULT '{}',
                embedding BLOB,
                parent_id TEXT,
                related_ids TEXT NOT NULL DEFAULT '[]'
            );
            CREATE INDEX IF NOT EXISTS idx_memories_type ON memories(memory_type);
            CREATE INDEX IF NOT EXISTS idx_memories_state ON memories(state);
            CREATE INDEX IF NOT EXISTS idx_memories_importance ON memories(importance);
            CREATE INDEX IF NOT EXISTS idx_memories_timestamp ON memories(timestamp);
            CREATE INDEX IF NOT EXISTS idx_memories_source ON memories(source);

            CREATE TABLE IF NOT EXISTS graph_nodes (
                id TEXT PRIMARY KEY,
                node_type TEXT NOT NULL,
                name TEXT NOT NULL,
                properties TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                metadata TEXT NOT NULL DEFAULT '{}'
            );

            CREATE TABLE IF NOT EXISTS graph_edges (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                target TEXT NOT NULL,
                edge_type TEXT NOT NULL,
                weight REAL NOT NULL DEFAULT 1.0,
                properties TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                FOREIGN KEY (source) REFERENCES graph_nodes(id),
                FOREIGN KEY (target) REFERENCES graph_nodes(id)
            );
            CREATE INDEX IF NOT EXISTS idx_edges_source ON graph_edges(source);
            CREATE INDEX IF NOT EXISTS idx_edges_target ON graph_edges(target);

            CREATE TABLE IF NOT EXISTS memory_versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                memory_id TEXT NOT NULL,
                version INTEGER NOT NULL,
                content TEXT NOT NULL,
                changes TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                FOREIGN KEY (memory_id) REFERENCES memories(id)
            );",
        )?;
        Ok(())
    }

    fn row_to_memory(row: &rusqlite::Row) -> rusqlite::Result<MemoryItem> {
        let id: String = row.get(0)?;
        let memory_type: String = row.get(1)?;
        let state: String = row.get(2)?;
        let content: String = row.get(3)?;
        let importance: f64 = row.get(4)?;
        let timestamp: String = row.get(5)?;
        let last_accessed: String = row.get(6)?;
        let access_count: u64 = row.get(7)?;
        let context_tags: String = row.get(8)?;
        let source: String = row.get(9)?;
        let version: u64 = row.get(10)?;
        let ttl_seconds: Option<i64> = row.get(11)?;
        let metadata: String = row.get(12)?;
        let embedding_bytes: Option<Vec<u8>> = row.get(13)?;
        let parent_id: Option<String> = row.get(14)?;
        let related_ids: String = row.get(15)?;

        let mem_type = match memory_type.as_str() {
            "Working" => MemoryType::Working,
            "ShortTerm" => MemoryType::ShortTerm,
            "Episodic" => MemoryType::Episodic,
            "Semantic" => MemoryType::Semantic,
            "Procedural" => MemoryType::Procedural,
            "Vector" => MemoryType::Vector,
            _ => MemoryType::Working,
        };

        let mem_state = match state.as_str() {
            "Active" => MemoryState::Active,
            "Dormant" => MemoryState::Dormant,
            "Compressed" => MemoryState::Compressed,
            "Archived" => MemoryState::Archived,
            _ => MemoryState::Active,
        };

        let ts: DateTime<Utc> = DateTime::parse_from_rfc3339(&timestamp)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let la: DateTime<Utc> = DateTime::parse_from_rfc3339(&last_accessed)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let tags: Vec<String> = serde_json::from_str(&context_tags).unwrap_or_default();
        let meta: HashMap<String, String> = serde_json::from_str(&metadata).unwrap_or_default();
        let embedding: Option<Vec<f32>> = embedding_bytes.map(|bytes| {
            bytes
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect()
        });
        let related: Vec<String> = serde_json::from_str(&related_ids).unwrap_or_default();
        let related: Vec<MemoryId> = related.into_iter().map(MemoryId).collect();

        Ok(MemoryItem {
            id: MemoryId(id),
            memory_type: mem_type,
            state: mem_state,
            content: serde_json::from_str(&content).unwrap_or(serde_json::Value::Null),
            importance,
            timestamp: ts,
            last_accessed: la,
            access_count,
            context_tags: tags,
            source,
            version,
            ttl: ttl_seconds.map(|s| Duration::seconds(s)),
            metadata: meta,
            embedding,
            parent_id: parent_id.map(MemoryId),
            related_ids: related,
        })
    }

    #[allow(dead_code)]
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        (dot / (norm_a * norm_b)) as f64
    }

    fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
        embedding
            .iter()
            .flat_map(|f| f.to_le_bytes().to_vec())
            .collect()
    }
}

impl Default for SqliteMemoryEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl MemoryApi for SqliteMemoryEngine {
    async fn init(&self, _config: &MemoryConfig) -> Result<()> {
        let conn = Connection::open(":memory:")
            .map_err(|e| MemoryError::StoreError(format!("Failed to open database: {e}")))?;
        Self::create_tables(&conn)?;
        let mut conn_guard = self.conn.lock().await;
        *conn_guard = Some(conn);
        info!("SqliteMemoryEngine initialized with in-memory database");
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        let mut conn_guard = self.conn.lock().await;
        *conn_guard = None;
        info!("SqliteMemoryEngine shut down");
        Ok(())
    }

    async fn store(&self, item: MemoryItem) -> Result<MemoryId> {
        let id = item.id.0.clone();
        self.with_conn(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO memories 
                 (id, memory_type, state, content, importance, timestamp, last_accessed,
                  access_count, context_tags, source, version, ttl_seconds, metadata,
                  embedding, parent_id, related_ids)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    id,
                    item.memory_type.to_string(),
                    item.state.to_string(),
                    item.content.to_string(),
                    item.importance,
                    item.timestamp.to_rfc3339(),
                    item.last_accessed.to_rfc3339(),
                    item.access_count,
                    serde_json::to_string(&item.context_tags).unwrap_or_default(),
                    item.source,
                    item.version,
                    item.ttl.map(|d| d.num_seconds()),
                    serde_json::to_string(&item.metadata).unwrap_or_default(),
                    item.embedding.as_ref().map(|e| Self::embedding_to_bytes(e)),
                    item.parent_id.as_ref().map(|p| &p.0),
                    serde_json::to_string(
                        &item.related_ids.iter().map(|r| &r.0).collect::<Vec<_>>()
                    )
                    .unwrap_or_default(),
                ],
            )?;
            debug!(memory_id = %id, "Stored memory item");
            Ok(MemoryId(id))
        })
        .await
    }

    async fn store_with_analysis(
        &self,
        item: MemoryItem,
        _context: Option<&voxy_world_model::context::WorldContext>,
    ) -> Result<(MemoryId, HermesClassification)> {
        let id = self.store(item.clone()).await?;
        let classification = HermesClassification {
            item_id: id.clone(),
            decision: HermesDecision::LongTermMemory,
            confidence: item.importance,
            reasons: vec!["stored via store_with_analysis".to_string()],
            timestamp: Utc::now(),
        };
        Ok((id, classification))
    }

    async fn retrieve(&self, id: &MemoryId) -> Result<MemoryItem> {
        let id_str = id.0.clone();
        self.with_conn(move |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT id, memory_type, state, content, importance, timestamp, last_accessed,
                     access_count, context_tags, source, version, ttl_seconds, metadata,
                     embedding, parent_id, related_ids
                     FROM memories WHERE id = ?1",
                )
                .map_err(|e| MemoryError::StoreError(e.to_string()))?;
            let item = stmt
                .query_row(params![id_str], Self::row_to_memory)
                .map_err(|_| MemoryError::ItemNotFound(id_str.clone()))?;

            conn.execute(
                "UPDATE memories SET access_count = access_count + 1, last_accessed = ?1 WHERE id = ?2",
                params![Utc::now().to_rfc3339(), id_str],
            )?;

            Ok(item)
        }).await
    }

    async fn search(&self, query: &MemoryQuery) -> Result<Vec<SearchResult>> {
        let query_text = query.query_text.clone();
        let query_types = query.memory_types.clone();
        let query_states = query.states.clone();
        let query_min_importance = query.min_importance;
        let query_source = query.source_filter.clone();
        let query_time_range = query.time_range;
        let query_tags = query.tags.clone();
        let query_include_embeddings = query.include_embeddings;
        let max_results = query.max_results;

        self.with_conn(move |conn| {
            let mut sql =
                "SELECT id, memory_type, state, content, importance, timestamp, last_accessed,
                     access_count, context_tags, source, version, ttl_seconds, metadata,
                     embedding, parent_id, related_ids FROM memories WHERE 1=1"
                    .to_string();
            let mut param_values: Vec<String> = Vec::new();

            if let Some(ref types) = query_types {
                for t in types {
                    param_values.push(t.to_string());
                    sql.push_str(&format!(" AND memory_type = ?{}", param_values.len()));
                }
            }
            if let Some(ref states) = query_states {
                for s in states {
                    param_values.push(s.to_string());
                    sql.push_str(&format!(" AND state = ?{}", param_values.len()));
                }
            }
            if let Some(min_imp) = query_min_importance {
                param_values.push(min_imp.to_string());
                sql.push_str(&format!(" AND importance >= ?{}", param_values.len()));
            }
            if let Some(ref source) = query_source {
                param_values.push(source.clone());
                sql.push_str(&format!(" AND source = ?{}", param_values.len()));
            }
            if let Some((ref start, ref end)) = query_time_range {
                param_values.push(start.to_rfc3339());
                let start_idx = param_values.len();
                param_values.push(end.to_rfc3339());
                let end_idx = param_values.len();
                sql.push_str(&format!(
                    " AND timestamp >= ?{start_idx} AND timestamp <= ?{end_idx}"
                ));
            }

            sql.push_str(&format!(
                " ORDER BY importance DESC, timestamp DESC LIMIT {max_results}"
            ));

            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| MemoryError::RetrievalError(e.to_string()))?;

            let param_refs: Vec<&dyn rusqlite::types::ToSql> = param_values
                .iter()
                .map(|v| v as &dyn rusqlite::types::ToSql)
                .collect();

            let items: Vec<MemoryItem> = stmt
                .query_map(param_refs.as_slice(), Self::row_to_memory)
                .map_err(|e| MemoryError::RetrievalError(e.to_string()))?
                .filter_map(|r| r.ok())
                .collect();

            let text_lower = query_text.as_deref().unwrap_or("").to_lowercase();

            let results: Vec<SearchResult> = items
                .into_iter()
                .map(|item| {
                    let mut score = item.importance;
                    let mut reasons = Vec::new();

                    if !text_lower.is_empty() {
                        let content_str = item.content.to_string().to_lowercase();
                        if content_str.contains(&text_lower) {
                            score += 0.2;
                            reasons.push("text match".to_string());
                        }
                    }

                    if let Some(ref tags) = query_tags {
                        if tags.iter().any(|t| item.context_tags.contains(t)) {
                            score += 0.1;
                            reasons.push("tag match".to_string());
                        }
                    }

                    if let Some(ref emb) = item.embedding {
                        if query_include_embeddings && !emb.is_empty() {
                            reasons.push("has embedding".to_string());
                        }
                    }

                    if reasons.is_empty() {
                        reasons.push("importance ranked".to_string());
                    }

                    SearchResult {
                        item,
                        score: score.min(1.0),
                        match_reasons: reasons,
                    }
                })
                .collect();

            Ok(results)
        })
        .await
    }

    async fn update(&self, item: MemoryItem) -> Result<()> {
        let id = item.id.0.clone();
        self.with_conn(move |conn| {
            let changed = conn.execute(
                "UPDATE memories SET content = ?1, importance = ?2, state = ?3, 
                 context_tags = ?4, metadata = ?5, version = version + 1
                 WHERE id = ?6",
                params![
                    item.content.to_string(),
                    item.importance,
                    item.state.to_string(),
                    serde_json::to_string(&item.context_tags).unwrap_or_default(),
                    serde_json::to_string(&item.metadata).unwrap_or_default(),
                    id,
                ],
            )?;
            if changed == 0 {
                return Err(MemoryError::ItemNotFound(id));
            }
            Ok(())
        })
        .await
    }

    async fn delete(&self, id: &MemoryId) -> Result<()> {
        let id_str = id.0.clone();
        self.with_conn(move |conn| {
            let changed = conn.execute("DELETE FROM memories WHERE id = ?1", params![id_str])?;
            if changed == 0 {
                return Err(MemoryError::ItemNotFound(id_str));
            }
            Ok(())
        })
        .await
    }

    async fn forget(&self, id: &MemoryId) -> Result<()> {
        let id_str = id.0.clone();
        self.with_conn(move |conn| {
            conn.execute(
                "UPDATE memories SET state = 'Archived' WHERE id = ?1",
                params![id_str],
            )?;
            debug!(memory_id = %id_str, "Archived memory item (forgotten)");
            Ok(())
        })
        .await
    }

    async fn recall(&self, id: &MemoryId) -> Result<MemoryItem> {
        self.retrieve(id).await
    }

    async fn consolidate(&self) -> Result<usize> {
        self.with_conn(move |conn| {
            let cutoff = (Utc::now() - Duration::hours(1)).to_rfc3339();
            let threshold = 0.6;
            let changed = conn.execute(
                "UPDATE memories SET state = 'Dormant' 
                 WHERE state = 'Active' AND importance < ?1 AND last_accessed < ?2",
                params![threshold, cutoff],
            )?;
            info!(count = changed, "Consolidation: demoted active to dormant");
            Ok(changed)
        })
        .await
    }

    async fn run_forgetting(&self) -> Result<usize> {
        self.with_conn(move |conn| {
            let dormant_cutoff = (Utc::now() - Duration::days(30)).to_rfc3339();
            let compressed_cutoff = (Utc::now() - Duration::days(60)).to_rfc3339();
            let archived_cutoff = (Utc::now() - Duration::days(365)).to_rfc3339();

            let compressed = conn.execute(
                "UPDATE memories SET state = 'Compressed' 
                 WHERE state = 'Dormant' AND last_accessed < ?1",
                params![dormant_cutoff],
            )?;

            let archived = conn.execute(
                "UPDATE memories SET state = 'Archived' 
                 WHERE state = 'Compressed' AND last_accessed < ?1",
                params![compressed_cutoff],
            )?;

            let deleted = conn.execute(
                "DELETE FROM memories 
                 WHERE state = 'Archived' AND last_accessed < ?1 AND importance < 0.2",
                params![archived_cutoff],
            )?;

            let total = compressed + archived + deleted;
            info!(compressed, archived, deleted, "Forgetting cycle complete");
            Ok(total)
        })
        .await
    }

    async fn graph(&self) -> &dyn KnowledgeGraph {
        noop_graph()
    }

    async fn hermes(&self) -> &dyn HermesEngine {
        noop_hermes()
    }

    async fn stats(&self) -> Result<MemoryStats> {
        self.with_conn(move |conn| {
            let total: usize = conn
                .query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))
                .unwrap_or(0);
            let working: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM memories WHERE memory_type = 'Working'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let short_term: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM memories WHERE memory_type = 'ShortTerm'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let episodic: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM memories WHERE memory_type = 'Episodic'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let semantic: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM memories WHERE memory_type = 'Semantic'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let procedural: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM memories WHERE memory_type = 'Procedural'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let vector: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM memories WHERE memory_type = 'Vector'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let graph_nodes: usize = conn
                .query_row("SELECT COUNT(*) FROM graph_nodes", [], |row| row.get(0))
                .unwrap_or(0);
            let graph_edges: usize = conn
                .query_row("SELECT COUNT(*) FROM graph_edges", [], |row| row.get(0))
                .unwrap_or(0);
            let active: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM memories WHERE state = 'Active'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let dormant: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM memories WHERE state = 'Dormant'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let compressed: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM memories WHERE state = 'Compressed'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let archived: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM memories WHERE state = 'Archived'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            Ok(MemoryStats {
                total_items: total,
                working_count: working,
                short_term_count: short_term,
                episodic_count: episodic,
                semantic_count: semantic,
                procedural_count: procedural,
                vector_count: vector,
                graph_nodes,
                graph_edges,
                active_count: active,
                dormant_count: dormant,
                compressed_count: compressed,
                archived_count: archived,
            })
        })
        .await
    }

    async fn clear(&self) -> Result<()> {
        self.with_conn(move |conn| {
            conn.execute("DELETE FROM memories", [])?;
            conn.execute("DELETE FROM graph_nodes", [])?;
            conn.execute("DELETE FROM graph_edges", [])?;
            conn.execute("DELETE FROM memory_versions", [])?;
            info!("All memory data cleared");
            Ok(())
        })
        .await
    }
}
