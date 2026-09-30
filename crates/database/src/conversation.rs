use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub user_id: String,
    pub title: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_active: bool,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub token_count: Option<i64>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
            MessageRole::System => write!(f, "system"),
        }
    }
}

impl std::str::FromStr for MessageRole {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user" => Ok(MessageRole::User),
            "assistant" => Ok(MessageRole::Assistant),
            "system" => Ok(MessageRole::System),
            _ => Err(format!("Unknown message role: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationStats {
    pub total_conversations: usize,
    pub total_messages: usize,
    pub active_conversations: usize,
}

#[async_trait]
pub trait ConversationStore: Send + Sync {
    async fn create_conversation(
        &self,
        user_id: &str,
        title: Option<&str>,
    ) -> Result<Conversation, String>;
    async fn get_conversation(&self, conversation_id: &str)
        -> Result<Option<Conversation>, String>;
    async fn list_conversations(
        &self,
        user_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Conversation>, String>;
    async fn delete_conversation(&self, conversation_id: &str) -> Result<(), String>;
    async fn update_conversation_title(
        &self,
        conversation_id: &str,
        title: &str,
    ) -> Result<(), String>;

    async fn add_message(
        &self,
        conversation_id: &str,
        role: MessageRole,
        content: &str,
        token_count: Option<i64>,
        metadata: Option<&str>,
    ) -> Result<Message, String>;
    async fn get_messages(
        &self,
        conversation_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Message>, String>;
    async fn get_message_count(&self, conversation_id: &str) -> Result<usize, String>;
    async fn delete_message(&self, message_id: &str) -> Result<(), String>;

    async fn stats(&self, user_id: &str) -> Result<ConversationStats, String>;
}

pub struct InMemoryConversationStore {
    conversations: parking_lot::RwLock<HashMap<String, Conversation>>,
    messages: parking_lot::RwLock<HashMap<String, Vec<Message>>>,
}

impl InMemoryConversationStore {
    pub fn new() -> Self {
        Self {
            conversations: parking_lot::RwLock::new(HashMap::new()),
            messages: parking_lot::RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryConversationStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConversationStore for InMemoryConversationStore {
    async fn create_conversation(
        &self,
        user_id: &str,
        title: Option<&str>,
    ) -> Result<Conversation, String> {
        let conv = Conversation {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            title: title.map(|t| t.to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            is_active: true,
            metadata: None,
        };
        self.conversations
            .write()
            .insert(conv.id.clone(), conv.clone());
        Ok(conv)
    }

    async fn get_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Option<Conversation>, String> {
        Ok(self.conversations.read().get(conversation_id).cloned())
    }

    async fn list_conversations(
        &self,
        user_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Conversation>, String> {
        let mut convs: Vec<_> = self
            .conversations
            .read()
            .values()
            .filter(|c| c.user_id == user_id)
            .cloned()
            .collect();
        convs.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
        Ok(convs.into_iter().skip(offset).take(limit).collect())
    }

    async fn delete_conversation(&self, conversation_id: &str) -> Result<(), String> {
        self.conversations.write().remove(conversation_id);
        self.messages.write().remove(conversation_id);
        Ok(())
    }

    async fn update_conversation_title(
        &self,
        conversation_id: &str,
        title: &str,
    ) -> Result<(), String> {
        if let Some(conv) = self.conversations.write().get_mut(conversation_id) {
            conv.title = Some(title.to_string());
            conv.updated_at = Utc::now();
            Ok(())
        } else {
            Err("Conversation not found".to_string())
        }
    }

    async fn add_message(
        &self,
        conversation_id: &str,
        role: MessageRole,
        content: &str,
        token_count: Option<i64>,
        metadata: Option<&str>,
    ) -> Result<Message, String> {
        let msg = Message {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_string(),
            role,
            content: content.to_string(),
            timestamp: Utc::now(),
            token_count,
            metadata: metadata.map(|m| m.to_string()),
        };
        self.messages
            .write()
            .entry(conversation_id.to_string())
            .or_default()
            .push(msg.clone());
        if let Some(conv) = self.conversations.write().get_mut(conversation_id) {
            conv.updated_at = Utc::now();
        }
        Ok(msg)
    }

    async fn get_messages(
        &self,
        conversation_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Message>, String> {
        let msgs = self.messages.read();
        let empty = Vec::new();
        let conv_msgs = msgs.get(conversation_id).unwrap_or(&empty);
        Ok(conv_msgs.iter().skip(offset).take(limit).cloned().collect())
    }

    async fn get_message_count(&self, conversation_id: &str) -> Result<usize, String> {
        let msgs = self.messages.read();
        Ok(msgs.get(conversation_id).map_or(0, |m| m.len()))
    }

    async fn delete_message(&self, message_id: &str) -> Result<(), String> {
        let mut msgs = self.messages.write();
        for vec in msgs.values_mut() {
            vec.retain(|m| m.id != message_id);
        }
        Ok(())
    }

    async fn stats(&self, user_id: &str) -> Result<ConversationStats, String> {
        let convs = self.conversations.read();
        let msgs = self.messages.read();
        let user_convs: Vec<_> = convs.values().filter(|c| c.user_id == user_id).collect();
        let total_messages: usize = user_convs
            .iter()
            .filter_map(|c| msgs.get(&c.id).map(|m| m.len()))
            .sum();
        Ok(ConversationStats {
            total_conversations: user_convs.len(),
            total_messages,
            active_conversations: user_convs.iter().filter(|c| c.is_active).count(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_conversation_create() {
        let store = InMemoryConversationStore::new();
        let conv = store
            .create_conversation("user-1", Some("Test Chat"))
            .await
            .unwrap();
        assert!(!conv.id.is_empty());
        assert_eq!(conv.user_id, "user-1");
        assert_eq!(conv.title.as_deref(), Some("Test Chat"));
    }

    #[tokio::test]
    async fn test_messages_roundtrip() {
        let store = InMemoryConversationStore::new();
        let conv = store.create_conversation("user-1", None).await.unwrap();
        store
            .add_message(&conv.id, MessageRole::User, "Hello", None, None)
            .await
            .unwrap();
        store
            .add_message(&conv.id, MessageRole::Assistant, "Hi!", Some(12), None)
            .await
            .unwrap();
        let msgs = store.get_messages(&conv.id, 10, 0).await.unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].content, "Hello");
        assert_eq!(msgs[1].content, "Hi!");
    }

    #[tokio::test]
    async fn test_delete_cascades() {
        let store = InMemoryConversationStore::new();
        let conv = store.create_conversation("user-1", None).await.unwrap();
        store
            .add_message(&conv.id, MessageRole::User, "test", None, None)
            .await
            .unwrap();
        store.delete_conversation(&conv.id).await.unwrap();
        assert!(store.get_conversation(&conv.id).await.unwrap().is_none());
        assert!(store
            .get_messages(&conv.id, 10, 0)
            .await
            .unwrap()
            .is_empty());
    }
}
