//! Native Email Management tools for searching inbox, reading threads, drafting messages,
//! and sending approved emails.

use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use voxy_office::{EmailMessage, MailConnector, OfficeAuditTrail, ScopeSet};

// Shared MailConnector instance
static MAIL_CONNECTOR: parking_lot::RwLock<Option<Arc<MailConnector>>> =
    parking_lot::RwLock::new(None);

fn get_mail_connector() -> Arc<MailConnector> {
    let mut lock = MAIL_CONNECTOR.write();
    if let Some(ref connector) = *lock {
        connector.clone()
    } else {
        let audit = Arc::new(OfficeAuditTrail::new(200));
        let connector = MailConnector::new("user@osmoo.local", audit);
        // Pre-populate with a few helpful workspace notifications/threads for realism
        connector.ingest_message(EmailMessage {
            id: "msg-welcome-01".to_string(),
            thread_id: "thread-onboarding".to_string(),
            from: "team@osmoo.ai".to_string(),
            to: vec!["user@osmoo.local".to_string()],
            cc: vec![],
            subject: "Welcome to OSMOO Workspace Companion".to_string(),
            body: "Welcome to your local AI companion. Your calendar, emails, and documents are integrated securely with local-first encryption.".to_string(),
            date: chrono::Utc::now(),
            has_attachments: false,
            labels: vec!["inbox".to_string(), "important".to_string()],
        });
        let arc = Arc::new(connector);
        *lock = Some(arc.clone());
        arc
    }
}

/// Tool to search email inbox
pub struct EmailSearchInboxTool {
    metadata: ToolMetadata,
}

impl EmailSearchInboxTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "email_search_inbox",
                "Search email inbox across sender, subject, and content keywords",
                ToolCategory::System,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": { "type": "string", "description": "Search keyword or query string" },
                    "max_results": { "type": "integer", "description": "Maximum number of emails to return (default 10)", "default": 10 }
                }
            })),
        }
    }
}

impl Default for EmailSearchInboxTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EmailSearchInboxTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'query' parameter".to_string()))?;
        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let connector = get_mail_connector();
        let scopes = ScopeSet::full_access();
        let messages = connector
            .search_inbox(query, max_results, &scopes, Some(ctx.session_id.as_str()))
            .map_err(|e| ToolError::ExecutionFailed(format!("email_search_inbox failed: {e}")))?;

        let count = messages.len();
        Ok(ToolResult::success(json!({
            "query": query,
            "count": count,
            "messages": messages,
        })))
    }
}

/// Tool to read and summarize an email thread
pub struct EmailReadThreadTool {
    metadata: ToolMetadata,
}

impl EmailReadThreadTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "email_read_thread",
                "Retrieve and summarize an email conversation thread with action item extraction",
                ToolCategory::System,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["thread_id"],
                "properties": {
                    "thread_id": { "type": "string", "description": "ID of the email thread to inspect" }
                }
            })),
        }
    }
}

impl Default for EmailReadThreadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EmailReadThreadTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let thread_id = args
            .get("thread_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'thread_id' parameter".to_string()))?;

        let connector = get_mail_connector();
        let scopes = ScopeSet::full_access();
        let messages = connector
            .read_thread(thread_id, &scopes, Some(ctx.session_id.as_str()))
            .map_err(|e| ToolError::ExecutionFailed(format!("email_read_thread failed: {e}")))?;

        let summary = connector.summarize_thread(thread_id, &scopes).ok();

        Ok(ToolResult::success(json!({
            "thread_id": thread_id,
            "message_count": messages.len(),
            "summary": summary,
            "messages": messages,
        })))
    }
}

/// Tool to draft an email message
pub struct EmailCreateDraftTool {
    metadata: ToolMetadata,
}

impl EmailCreateDraftTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "email_create_draft",
                "Create a new email draft (does NOT send until explicitly approved)",
                ToolCategory::System,
                RiskTier::Modify,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["to", "subject", "body"],
                "properties": {
                    "to": { "type": "array", "items": { "type": "string" }, "description": "Recipient email addresses" },
                    "cc": { "type": "array", "items": { "type": "string" }, "description": "CC email addresses" },
                    "subject": { "type": "string", "description": "Email subject" },
                    "body": { "type": "string", "description": "Email body content" },
                    "in_reply_to": { "type": "string", "description": "Optional thread/message ID to reply to" }
                }
            })),
        }
    }
}

impl Default for EmailCreateDraftTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EmailCreateDraftTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let to = args
            .get("to")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str().map(|str_val| str_val.to_string()))
                    .collect::<Vec<_>>()
            })
            .ok_or_else(|| ToolError::InvalidParams("Missing 'to' array parameter".to_string()))?;

        let cc = args
            .get("cc")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str().map(|str_val| str_val.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let subject = args
            .get("subject")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'subject' parameter".to_string()))?;

        let body = args
            .get("body")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'body' parameter".to_string()))?;

        let in_reply_to = args
            .get("in_reply_to")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let connector = get_mail_connector();
        let scopes = ScopeSet::full_access();
        let draft = connector
            .create_draft(to, cc, subject, body, in_reply_to, &scopes, Some(ctx.session_id.as_str()))
            .map_err(|e| ToolError::ExecutionFailed(format!("email_create_draft failed: {e}")))?;

        Ok(ToolResult::success(json!({
            "draft_id": draft.id,
            "status": draft.status,
            "requires_approval": !draft.is_approved,
            "draft": draft,
        })))
    }
}

/// Tool to send an approved email draft
pub struct EmailSendApprovedTool {
    metadata: ToolMetadata,
}

impl EmailSendApprovedTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "email_send_approved",
                "Send an email draft that has been explicitly approved by the user",
                ToolCategory::System,
                RiskTier::Privileged,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["draft_id", "approval_token"],
                "properties": {
                    "draft_id": { "type": "string", "description": "ID of the draft to send" },
                    "approval_token": { "type": "string", "description": "Security confirmation token granted upon user approval" }
                }
            }))
            .with_confirmation(true),
        }
    }
}

impl Default for EmailSendApprovedTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EmailSendApprovedTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let draft_id = args
            .get("draft_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'draft_id' parameter".to_string()))?;

        let approval_token = args
            .get("approval_token")
            .and_then(|v| v.as_str())
            .filter(|t| !t.trim().is_empty())
            .ok_or_else(|| ToolError::InvalidParams("Missing or empty 'approval_token' parameter (mandatory user approval gate)".to_string()))?;

        if approval_token.trim().len() < 6 {
            return Err(ToolError::InvalidParams(
                "Invalid 'approval_token': token must be at least 6 characters".to_string(),
            ));
        }

        if !ctx.user_confirmed {
            return Err(ToolError::ExecutionFailed(
                "Human approval required: user confirmation was not granted for email send".to_string(),
            ));
        }

        let connector = get_mail_connector();
        // Mark draft as approved with explicit user context
        connector
            .approve_draft(draft_id, Some(ctx.session_id.as_str()))
            .map_err(|e| ToolError::ExecutionFailed(format!("Approval failed: {e}")))?;

        let scopes = ScopeSet::full_access();
        let sent_message = connector
            .send_approved_email(draft_id, ctx.user_confirmed, &scopes, Some(ctx.session_id.as_str()))
            .map_err(|e| ToolError::ExecutionFailed(format!("email_send_approved failed: {e}")))?;

        Ok(ToolResult::success(json!({
            "status": "sent",
            "message_id": sent_message.id,
            "sent_message": sent_message,
        })))
    }
}
