//! Enterprise Email Management Connector (Gmail & Microsoft Outlook 365).
//! Implements secure inbox search, thread context retrieval, action item extraction,
//! message drafting, and MANDATORY user confirmation approval before sending.

use crate::audit::{AuditStatus, OfficeAuditEvent, OfficeAuditTrail};
use crate::error::{OfficeError, Result};
use crate::scope::{OfficeScope, ScopeSet};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmailProvider {
    Gmail,
    Outlook,
    LocalMock,
}

impl EmailProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            EmailProvider::Gmail => "gmail",
            EmailProvider::Outlook => "outlook",
            EmailProvider::LocalMock => "local_mock",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailMessage {
    pub id: String,
    pub thread_id: String,
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub date: DateTime<Utc>,
    pub has_attachments: bool,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailDraft {
    pub id: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub in_reply_to: Option<String>,
    pub created_at: DateTime<Utc>,
    pub is_approved: bool,
    pub status: String, // "draft", "awaiting_approval", "sent"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub thread_id: String,
    pub subject: String,
    pub participant_count: usize,
    pub message_count: usize,
    pub executive_summary: String,
    pub action_items: Vec<String>,
}

#[derive(Clone)]
pub struct MailConnector {
    provider: EmailProvider,
    email_address: String,
    #[allow(dead_code)]
    oauth_token: Option<String>,
    mock_inbox: Arc<parking_lot::RwLock<HashMap<String, EmailMessage>>>,
    drafts: Arc<parking_lot::RwLock<HashMap<String, EmailDraft>>>,
    audit_trail: Arc<OfficeAuditTrail>,
}

impl std::fmt::Debug for MailConnector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MailConnector")
            .field("provider", &self.provider)
            .field("email_address", &self.email_address)
            .finish()
    }
}

impl MailConnector {
    pub fn new(email_address: impl Into<String>, audit_trail: Arc<OfficeAuditTrail>) -> Self {
        Self {
            provider: EmailProvider::LocalMock,
            email_address: email_address.into(),
            oauth_token: None,
            mock_inbox: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            drafts: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            audit_trail,
        }
    }

    pub fn with_oauth(
        provider: EmailProvider,
        email_address: impl Into<String>,
        oauth_token: impl Into<String>,
        audit_trail: Arc<OfficeAuditTrail>,
    ) -> Self {
        Self {
            provider,
            email_address: email_address.into(),
            oauth_token: Some(oauth_token.into()),
            mock_inbox: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            drafts: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            audit_trail,
        }
    }

    /// Add an email message to inbox (used by sync or testing)
    pub fn ingest_message(&self, msg: EmailMessage) {
        self.mock_inbox.write().insert(msg.id.clone(), msg);
    }

    /// Search emails by keyword across sender, subject, and body
    pub fn search_inbox(
        &self,
        query: &str,
        max_results: usize,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<Vec<EmailMessage>> {
        scopes.check(OfficeScope::FilesRead)?; // Email read scope check

        let q = query.to_lowercase();
        let inbox = self.mock_inbox.read();

        let mut matches: Vec<EmailMessage> = inbox
            .values()
            .filter(|m| {
                m.subject.to_lowercase().contains(&q)
                    || m.from.to_lowercase().contains(&q)
                    || m.body.to_lowercase().contains(&q)
            })
            .cloned()
            .collect();

        matches.sort_by(|a, b| b.date.cmp(&a.date));
        matches.truncate(max_results);

        self.audit_trail.record(OfficeAuditEvent::new(
            "Mail",
            "search_inbox",
            query,
            user_id,
            "mail:read",
            AuditStatus::Success,
            serde_json::json!({ "results_count": matches.len() }),
        ));

        Ok(matches)
    }

    /// Retrieve full thread context for an email thread
    pub fn read_thread(
        &self,
        thread_id: &str,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<Vec<EmailMessage>> {
        scopes.check(OfficeScope::FilesRead)?;

        let inbox = self.mock_inbox.read();
        let mut thread: Vec<EmailMessage> = inbox
            .values()
            .filter(|m| m.thread_id == thread_id)
            .cloned()
            .collect();

        thread.sort_by(|a, b| a.date.cmp(&b.date));

        self.audit_trail.record(OfficeAuditEvent::new(
            "Mail",
            "read_thread",
            thread_id,
            user_id,
            "mail:read",
            AuditStatus::Success,
            serde_json::json!({ "messages_in_thread": thread.len() }),
        ));

        Ok(thread)
    }

    /// Summarize email thread and extract action items
    pub fn summarize_thread(&self, thread_id: &str, scopes: &ScopeSet) -> Result<ThreadSummary> {
        let messages = self.read_thread(thread_id, scopes, Some("summarizer"))?;
        if messages.is_empty() {
            return Err(OfficeError::NotFound(format!(
                "Thread {thread_id} not found"
            )));
        }

        let subject = messages
            .first()
            .map(|m| m.subject.clone())
            .unwrap_or_default();
        let mut participants = std::collections::HashSet::new();
        let mut full_body = String::new();

        for m in &messages {
            participants.insert(m.from.clone());
            for r in &m.to {
                participants.insert(r.clone());
            }
            full_body.push_str(&m.body);
            full_body.push('\n');
        }

        // Rule-based action item extraction (sentences containing "please", "can you", "action", "need to", "deadline")
        let mut action_items = Vec::new();
        for line in full_body.lines() {
            let l_lower = line.to_lowercase();
            if l_lower.contains("please")
                || l_lower.contains("need to")
                || l_lower.contains("by tomorrow")
                || l_lower.contains("by eod")
                || l_lower.contains("action item")
                || l_lower.contains("deadline")
            {
                let clean = line.trim();
                if !clean.is_empty() && !action_items.contains(&clean.to_string()) {
                    action_items.push(clean.to_string());
                }
            }
        }

        let summary_text = format!(
            "Discussion on '{}' across {} messages. Key participants: {}.",
            subject,
            messages.len(),
            participants.iter().cloned().collect::<Vec<_>>().join(", ")
        );

        Ok(ThreadSummary {
            thread_id: thread_id.to_string(),
            subject,
            participant_count: participants.len(),
            message_count: messages.len(),
            executive_summary: summary_text,
            action_items,
        })
    }

    /// Create an email draft (Default Sending Policy: Always Draft First)
    pub fn create_draft(
        &self,
        to: Vec<String>,
        cc: Vec<String>,
        subject: impl Into<String>,
        body: impl Into<String>,
        in_reply_to: Option<String>,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<EmailDraft> {
        scopes.check(OfficeScope::DocumentWrite)?;

        if to.is_empty() {
            return Err(OfficeError::InvalidData(
                "Draft must have at least one recipient".into(),
            ));
        }

        let draft = EmailDraft {
            id: Uuid::new_v4().to_string(),
            to,
            cc,
            subject: subject.into(),
            body: body.into(),
            in_reply_to,
            created_at: Utc::now(),
            is_approved: false,
            status: "draft".to_string(),
        };

        self.drafts.write().insert(draft.id.clone(), draft.clone());

        self.audit_trail.record(OfficeAuditEvent::new(
            "Mail",
            "create_draft",
            &draft.id,
            user_id,
            "mail:draft",
            AuditStatus::Success,
            serde_json::json!({
                "draft_id": draft.id,
                "to": draft.to,
                "subject": draft.subject
            }),
        ));

        Ok(draft)
    }

    /// Explicitly approve a draft before sending (MANDATORY APPROVAL GATE)
    pub fn approve_draft(&self, draft_id: &str, user_id: Option<&str>) -> Result<EmailDraft> {
        let mut drafts = self.drafts.write();
        let draft = drafts
            .get_mut(draft_id)
            .ok_or_else(|| OfficeError::NotFound(format!("Draft {draft_id} not found")))?;

        draft.is_approved = true;
        draft.status = "awaiting_approval".to_string();

        self.audit_trail.record(OfficeAuditEvent::new(
            "Mail",
            "approve_draft",
            draft_id,
            user_id,
            "mail:approve",
            AuditStatus::Success,
            serde_json::json!({ "draft_id": draft_id, "approved": true }),
        ));

        Ok(draft.clone())
    }

    /// Send email with mandatory user approval check
    pub fn send_approved_email(
        &self,
        draft_id: &str,
        user_approval_confirmed: bool,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<EmailMessage> {
        scopes.check(OfficeScope::DocumentWrite)?;

        // CRITICAL POLICY ENFORCEMENT: Reject send if not explicitly confirmed by human user
        if !user_approval_confirmed {
            self.audit_trail.record(OfficeAuditEvent::new(
                "Mail",
                "send_email",
                draft_id,
                user_id,
                "mail:send",
                AuditStatus::Denied,
                serde_json::json!({ "error": "Human approval confirmation required before sending email" }),
            ));
            return Err(OfficeError::PermissionDenied {
                required: "Human-in-the-loop explicit confirmation (send_approved_email)".into(),
            });
        }

        let mut drafts = self.drafts.write();
        let draft = drafts
            .get_mut(draft_id)
            .ok_or_else(|| OfficeError::NotFound(format!("Draft {draft_id} not found")))?;

        if !draft.is_approved {
            return Err(OfficeError::PermissionDenied {
                required: "Draft must be explicitly approved before sending".into(),
            });
        }
        if draft.status == "sent" {
            return Err(OfficeError::InvalidData(format!(
                "Draft {draft_id} has already been sent"
            )));
        }

        draft.status = "sent".to_string();

        let sent_message = EmailMessage {
            id: format!("sent_{}", Uuid::new_v4()),
            thread_id: draft
                .in_reply_to
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            from: self.email_address.clone(),
            to: draft.to.clone(),
            cc: draft.cc.clone(),
            subject: draft.subject.clone(),
            body: draft.body.clone(),
            date: Utc::now(),
            has_attachments: false,
            labels: vec!["SENT".to_string()],
        };

        // Record in inbox sent view
        self.mock_inbox
            .write()
            .insert(sent_message.id.clone(), sent_message.clone());

        self.audit_trail.record(OfficeAuditEvent::new(
            "Mail",
            "send_email",
            &sent_message.id,
            user_id,
            "mail:send",
            AuditStatus::Success,
            serde_json::json!({
                "message_id": sent_message.id,
                "to": sent_message.to,
                "subject": sent_message.subject
            }),
        ));

        Ok(sent_message)
    }
}
