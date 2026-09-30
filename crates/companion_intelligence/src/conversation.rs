use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

/// SECURITY: Sanitize text before storing in conversation memory.
/// Strips system prompt delimiters and control sequences that could be used
/// for indirect prompt injection when conversation history is fed back to LLMs.
fn sanitize_conversation_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            // Block control characters except newline/tab
            '\x00'..='\x08' | '\x0b' | '\x0c' | '\x0e'..='\x1f' | '\x7f' => {
                result.push(' ');
            }
            // Escape system prompt delimiters
            '<' => {
                // Check if this starts a system tag
                let peek: String = chars.clone().take(20).collect();
                if peek.starts_with("system>")
                    || peek.starts_with("/system>")
                    || peek.starts_with("user_message>")
                    || peek.starts_with("/user_message>")
                    || peek.starts_with("voxy_identity>")
                    || peek.starts_with("security_rules>")
                {
                    result.push_str("&lt;");
                } else {
                    result.push('<');
                }
            }
            _ => result.push(c),
        }
    }

    // Truncate to prevent memory exhaustion from adversarial input
    const MAX_TURN_TEXT: usize = 10_000;
    if result.len() > MAX_TURN_TEXT {
        result.truncate(MAX_TURN_TEXT);
        result.push_str("...[truncated]");
    }

    result
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub id: String,
    pub speaker: Speaker,
    pub text: String,
    pub timestamp: DateTime<Utc>,
    pub topics: Vec<String>,
    pub entities: Vec<Entity>,
    pub sentiment: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Speaker {
    User,
    Voxy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub name: String,
    pub entity_type: EntityType,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityType {
    Application,
    File,
    Project,
    Person,
    Command,
    Concept,
    Date,
    Number,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub id: String,
    pub name: String,
    pub first_mentioned: DateTime<Utc>,
    pub last_mentioned: DateTime<Utc>,
    pub mention_count: usize,
    pub relevance: f64,
    pub related_topics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceCandidate {
    pub turn_id: String,
    pub text: String,
    pub relevance: f64,
    pub recency: f64,
    pub entity_match: bool,
    pub topic_match: bool,
}

pub struct ConversationMemory {
    turns: VecDeque<ConversationTurn>,
    max_turns: usize,
}

pub struct TopicTracker {
    topics: HashMap<String, Topic>,
    active_topic: Option<String>,
    topic_history: VecDeque<String>,
    max_topics: usize,
}

pub struct ReferenceResolver {
    #[allow(dead_code)]
    pronouns: HashMap<String, Vec<String>>,
    #[allow(dead_code)]
    contextual_references: HashMap<String, Vec<String>>,
}

pub struct ConversationIntelligence {
    memory: ConversationMemory,
    topic_tracker: TopicTracker,
    reference_resolver: ReferenceResolver,
    context: ConversationContext,
}

#[derive(Debug, Clone, Default)]
pub struct ConversationContext {
    pub current_topic: Option<String>,
    pub recent_entities: Vec<Entity>,
    pub mentioned_projects: Vec<String>,
    pub mentioned_files: Vec<String>,
    pub mentioned_apps: Vec<String>,
    pub conversation_depth: usize,
    pub last_user_intent: Option<String>,
}

impl ConversationMemory {
    pub fn new(max_turns: usize) -> Self {
        Self {
            turns: VecDeque::with_capacity(max_turns),
            max_turns,
        }
    }

    pub fn add_turn(&mut self, turn: ConversationTurn) {
        if self.turns.len() >= self.max_turns {
            self.turns.pop_front();
        }
        self.turns.push_back(turn);
    }

    pub fn recent_turns(&self, count: usize) -> Vec<&ConversationTurn> {
        self.turns.iter().rev().take(count).collect()
    }

    pub fn turns_by_speaker(&self, speaker: Speaker) -> Vec<&ConversationTurn> {
        self.turns.iter().filter(|t| t.speaker == speaker).collect()
    }

    pub fn find_turns_with_entity(&self, entity_name: &str) -> Vec<&ConversationTurn> {
        self.turns
            .iter()
            .filter(|t| t.entities.iter().any(|e| e.name == entity_name))
            .collect()
    }

    pub fn find_turns_with_topic(&self, topic: &str) -> Vec<&ConversationTurn> {
        self.turns
            .iter()
            .filter(|t| t.topics.iter().any(|t| t == topic))
            .collect()
    }

    pub fn last_user_turn(&self) -> Option<&ConversationTurn> {
        self.turns.iter().rev().find(|t| t.speaker == Speaker::User)
    }

    pub fn turn_count(&self) -> usize {
        self.turns.len()
    }

    pub fn clear(&mut self) {
        self.turns.clear();
    }
}

impl TopicTracker {
    pub fn new(max_topics: usize) -> Self {
        Self {
            topics: HashMap::new(),
            active_topic: None,
            topic_history: VecDeque::with_capacity(max_topics),
            max_topics,
        }
    }

    pub fn track_topic(&mut self, topic_name: &str) {
        let now = Utc::now();
        let topic = self
            .topics
            .entry(topic_name.to_string())
            .or_insert_with(|| Topic {
                id: Uuid::new_v4().to_string(),
                name: topic_name.to_string(),
                first_mentioned: now,
                last_mentioned: now,
                mention_count: 0,
                relevance: 0.5,
                related_topics: vec![],
            });

        topic.last_mentioned = now;
        topic.mention_count += 1;
        topic.relevance = (topic.relevance + 0.1).min(1.0);

        self.active_topic = Some(topic_name.to_string());
        self.topic_history.push_back(topic_name.to_string());
        if self.topic_history.len() > self.max_topics {
            self.topic_history.pop_front();
        }

        // Evict least-relevant topics if exceeding capacity
        if self.topics.len() > self.max_topics * 2 {
            let mut topics: Vec<_> = self
                .topics
                .iter()
                .map(|(k, v)| (k.clone(), v.relevance))
                .collect();
            topics.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            let to_remove = topics.len() - self.max_topics;
            for (name, _) in topics.into_iter().take(to_remove) {
                self.topics.remove(&name);
            }
        }
    }

    pub fn active_topic(&self) -> Option<&str> {
        self.active_topic.as_deref()
    }

    pub fn get_topic(&self, name: &str) -> Option<&Topic> {
        self.topics.get(name)
    }

    pub fn recent_topics(&self, count: usize) -> Vec<&Topic> {
        self.topic_history
            .iter()
            .rev()
            .take(count)
            .filter_map(|name| self.topics.get(name))
            .collect()
    }

    pub fn all_topics(&self) -> &HashMap<String, Topic> {
        &self.topics
    }

    pub fn decay_topics(&mut self, decay_rate: f64) {
        for topic in self.topics.values_mut() {
            topic.relevance = (topic.relevance - decay_rate).max(0.0);
        }
    }

    pub fn topic_shift_score(&self, new_topic: &str) -> f64 {
        if let Some(active) = &self.active_topic {
            if active == new_topic {
                0.0
            } else {
                let active_topic = self.topics.get(active);
                let new_topic_data = self.topics.get(new_topic);

                match (active_topic, new_topic_data) {
                    (Some(a), Some(n)) => {
                        let related = a.related_topics.contains(&n.name);
                        if related {
                            0.3
                        } else {
                            0.8
                        }
                    }
                    _ => 0.5,
                }
            }
        } else {
            0.5
        }
    }
}

impl Default for ReferenceResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ReferenceResolver {
    pub fn new() -> Self {
        let mut pronouns = HashMap::new();
        pronouns.insert("it".to_string(), vec![]);
        pronouns.insert("that".to_string(), vec![]);
        pronouns.insert("this".to_string(), vec![]);
        pronouns.insert("they".to_string(), vec![]);
        pronouns.insert("them".to_string(), vec![]);
        pronouns.insert("he".to_string(), vec![]);
        pronouns.insert("she".to_string(), vec![]);

        Self {
            pronouns,
            contextual_references: HashMap::new(),
        }
    }

    pub fn resolve_reference(
        &self,
        text: &str,
        context: &ConversationContext,
        memory: &ConversationMemory,
    ) -> Option<String> {
        let lower = text.to_lowercase();

        if lower == "that" || lower == "it" || lower == "this" {
            return self.resolve_pronoun(&lower, context, memory);
        }

        if lower == "continue" || lower == "same project" || lower == "same thing" {
            return self.resolve_continuation(context, memory);
        }

        if lower.starts_with("the ") || lower.starts_with("that ") {
            return self.resolve_definite_reference(&lower, context, memory);
        }

        None
    }

    fn resolve_pronoun(
        &self,
        _pronoun: &str,
        context: &ConversationContext,
        memory: &ConversationMemory,
    ) -> Option<String> {
        if !context.recent_entities.is_empty() {
            let entity = &context.recent_entities[0];
            return Some(entity.name.clone());
        }

        let last_user_turn = memory.last_user_turn()?;
        if !last_user_turn.entities.is_empty() {
            return Some(last_user_turn.entities[0].name.clone());
        }

        None
    }

    fn resolve_continuation(
        &self,
        context: &ConversationContext,
        memory: &ConversationMemory,
    ) -> Option<String> {
        if let Some(topic) = &context.current_topic {
            return Some(topic.clone());
        }

        if let Some(project) = context.mentioned_projects.last() {
            return Some(project.clone());
        }

        let last_user_turn = memory.last_user_turn()?;
        if let Some(topic) = last_user_turn.topics.first() {
            return Some(topic.clone());
        }

        None
    }

    fn resolve_definite_reference(
        &self,
        text: &str,
        context: &ConversationContext,
        _memory: &ConversationMemory,
    ) -> Option<String> {
        let search_term = text
            .strip_prefix("the ")
            .or_else(|| text.strip_prefix("that "))
            .unwrap_or(text);

        for entity in &context.recent_entities {
            if entity.name.to_lowercase().contains(search_term) {
                return Some(entity.name.clone());
            }
        }

        None
    }

    pub fn track_entity(&mut self, entity: Entity, context: &mut ConversationContext) {
        context.recent_entities.insert(0, entity.clone());
        if context.recent_entities.len() > 10 {
            context.recent_entities.pop();
        }

        match entity.entity_type {
            EntityType::Project if !context.mentioned_projects.contains(&entity.name) => {
                context.mentioned_projects.push(entity.name.clone());
                if context.mentioned_projects.len() > 50 {
                    context.mentioned_projects.remove(0);
                }
            }
            EntityType::File if !context.mentioned_files.contains(&entity.name) => {
                context.mentioned_files.push(entity.name.clone());
                if context.mentioned_files.len() > 50 {
                    context.mentioned_files.remove(0);
                }
            }
            EntityType::Application if !context.mentioned_apps.contains(&entity.name) => {
                context.mentioned_apps.push(entity.name.clone());
                if context.mentioned_apps.len() > 50 {
                    context.mentioned_apps.remove(0);
                }
            }
            _ => {}
        }
    }
}

impl ConversationIntelligence {
    pub fn new(max_turns: usize, max_topics: usize) -> Self {
        Self {
            memory: ConversationMemory::new(max_turns),
            topic_tracker: TopicTracker::new(max_topics),
            reference_resolver: ReferenceResolver::new(),
            context: ConversationContext::default(),
        }
    }

    pub fn process_turn(&mut self, turn: ConversationTurn) {
        // SECURITY: Sanitize text before storing to prevent indirect prompt injection
        let sanitized_turn = ConversationTurn {
            id: turn.id,
            speaker: turn.speaker,
            text: sanitize_conversation_text(&turn.text),
            timestamp: turn.timestamp,
            topics: turn.topics,
            entities: turn.entities,
            sentiment: turn.sentiment,
        };

        for topic in &sanitized_turn.topics {
            self.topic_tracker.track_topic(topic);
        }

        for entity in &sanitized_turn.entities {
            self.reference_resolver
                .track_entity(entity.clone(), &mut self.context);
        }

        self.context.current_topic = self.topic_tracker.active_topic().map(|s| s.to_string());
        self.context.conversation_depth += 1;

        self.memory.add_turn(sanitized_turn);
    }

    pub fn resolve_reference(&self, text: &str) -> Option<String> {
        self.reference_resolver
            .resolve_reference(text, &self.context, &self.memory)
    }

    pub fn memory(&self) -> &ConversationMemory {
        &self.memory
    }

    pub fn topic_tracker(&self) -> &TopicTracker {
        &self.topic_tracker
    }

    pub fn context(&self) -> &ConversationContext {
        &self.context
    }

    pub fn build_context_summary(&self) -> String {
        let mut parts = Vec::new();

        if let Some(topic) = &self.context.current_topic {
            parts.push(format!("Current topic: {}", topic));
        }

        if !self.context.mentioned_projects.is_empty() {
            parts.push(format!(
                "Projects: {}",
                self.context.mentioned_projects.join(", ")
            ));
        }

        if !self.context.mentioned_apps.is_empty() {
            parts.push(format!("Apps: {}", self.context.mentioned_apps.join(", ")));
        }

        if parts.is_empty() {
            "No active conversation context".to_string()
        } else {
            parts.join("; ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_turn(text: &str, speaker: Speaker) -> ConversationTurn {
        ConversationTurn {
            id: Uuid::new_v4().to_string(),
            speaker,
            text: text.to_string(),
            timestamp: Utc::now(),
            topics: vec![],
            entities: vec![],
            sentiment: 0.5,
        }
    }

    #[test]
    fn test_conversation_memory() {
        let mut memory = ConversationMemory::new(10);
        memory.add_turn(create_test_turn("Hello", Speaker::User));
        memory.add_turn(create_test_turn("Hi there", Speaker::Voxy));
        assert_eq!(memory.turn_count(), 2);
        assert!(memory.last_user_turn().is_some());
    }

    #[test]
    fn test_topic_tracker() {
        let mut tracker = TopicTracker::new(10);
        tracker.track_topic("rust");
        tracker.track_topic("coding");
        assert_eq!(tracker.active_topic(), Some("coding"));
        assert_eq!(tracker.all_topics().len(), 2);
    }

    #[test]
    fn test_reference_resolver() {
        let resolver = ReferenceResolver::new();
        let mut context = ConversationContext::default();
        context.recent_entities.push(Entity {
            name: "main.rs".to_string(),
            entity_type: EntityType::File,
            start: 0,
            end: 0,
        });
        let memory = ConversationMemory::new(10);

        let resolved = resolver.resolve_reference("it", &context, &memory);
        assert_eq!(resolved, Some("main.rs".to_string()));
    }

    #[test]
    fn test_conversation_intelligence() {
        let mut intel = ConversationIntelligence::new(20, 10);
        let turn = ConversationTurn {
            id: Uuid::new_v4().to_string(),
            speaker: Speaker::User,
            text: "Let's work on the Rust project".to_string(),
            timestamp: Utc::now(),
            topics: vec!["rust".to_string()],
            entities: vec![Entity {
                name: "Rust project".to_string(),
                entity_type: EntityType::Project,
                start: 20,
                end: 32,
            }],
            sentiment: 0.7,
        };

        intel.process_turn(turn);
        assert_eq!(intel.topic_tracker.active_topic(), Some("rust"));
        assert_eq!(intel.context.mentioned_projects.len(), 1);
    }

    #[test]
    fn test_context_summary() {
        let mut intel = ConversationIntelligence::new(20, 10);
        let turn = ConversationTurn {
            id: Uuid::new_v4().to_string(),
            speaker: Speaker::User,
            text: "Working on voxy".to_string(),
            timestamp: Utc::now(),
            topics: vec!["voxy".to_string()],
            entities: vec![],
            sentiment: 0.5,
        };
        intel.process_turn(turn);

        let summary = intel.build_context_summary();
        assert!(summary.contains("voxy"));
    }
}
