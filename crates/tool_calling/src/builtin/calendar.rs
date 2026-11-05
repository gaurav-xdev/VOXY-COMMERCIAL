//! Native Calendar tools backed by persistent SQLite storage.

use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use voxy_office::{CalendarConnector, OfficeAuditTrail, RecurrenceRule, ScopeSet};

// Shared persistent SQLite calendar connector
static CALENDAR: parking_lot::RwLock<Option<Arc<CalendarConnector>>> =
    parking_lot::RwLock::new(None);

fn get_calendar() -> Arc<CalendarConnector> {
    let mut lock = CALENDAR.write();
    if let Some(ref cal) = *lock {
        cal.clone()
    } else {
        let db_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("voxy")
            .join("calendar");
        let db_path = db_dir.join("calendar.db");
        let audit = Arc::new(OfficeAuditTrail::new(200));
        let connector = CalendarConnector::with_sqlite(&db_path, audit)
            .unwrap_or_else(|_| CalendarConnector::new(Arc::new(OfficeAuditTrail::new(100))));
        let arc = Arc::new(connector);
        *lock = Some(arc.clone());
        arc
    }
}

/// Tool to schedule a calendar event with conflict detection
pub struct CalendarCreateEventTool {
    metadata: ToolMetadata,
}

impl CalendarCreateEventTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "calendar_create_event",
                "Create and schedule a new calendar event with persistent SQLite storage and conflict detection",
                ToolCategory::System,
                RiskTier::LowRisk,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["title", "start_time"],
                "properties": {
                    "title": { "type": "string", "description": "Title or summary of the event" },
                    "start_time": { "type": "string", "description": "RFC3339 start time e.g. 2026-10-28T14:00:00Z" },
                    "end_time": { "type": "string", "description": "Optional RFC3339 end time (defaults to 1 hour after start)" },
                    "location": { "type": "string", "description": "Optional event location or meeting room" },
                    "description": { "type": "string", "description": "Optional detailed agenda or notes" },
                    "attendees": { "type": "array", "items": { "type": "string" }, "description": "List of attendee email addresses" },
                    "recurrence": { "type": "string", "enum": ["none", "daily", "weekly", "monthly", "weekdays"], "description": "Recurrence pattern" }
                }
            })),
        }
    }
}

impl Default for CalendarCreateEventTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CalendarCreateEventTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        // timer
        let title = params
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'title'".into()))?;
        let start_str = params
            .get("start_time")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'start_time'".into()))?;

        let start_dt = DateTime::parse_from_rfc3339(start_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| ToolError::InvalidParams(format!("Invalid RFC3339 start_time: {e}")))?;

        let end_dt = if let Some(end_str) = params.get("end_time").and_then(|v| v.as_str()) {
            DateTime::parse_from_rfc3339(end_str)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ToolError::InvalidParams(format!("Invalid RFC3339 end_time: {e}")))?
        } else {
            start_dt + Duration::hours(1)
        };

        let location = params.get("location").and_then(|v| v.as_str());
        let description = params.get("description").and_then(|v| v.as_str());
        let recurrence_str = params
            .get("recurrence")
            .and_then(|v| v.as_str())
            .unwrap_or("none");
        let recurrence = RecurrenceRule::from_str(recurrence_str);

        let attendees: Vec<String> = params
            .get("attendees")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let cal = get_calendar();
        let scopes = ScopeSet::full_access();

        let event = cal
            .add_event_with_recurrence(
                title,
                start_dt,
                end_dt,
                location,
                description,
                attendees,
                recurrence,
                &scopes,
                Some(&ctx.session_id),
            )
            .map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to create calendar event: {e}"))
            })?;

        Ok(ToolResult::success(json!({
            "id": event.id,
            "title": event.title,
            "start_time": event.start_time.to_rfc3339(),
            "end_time": event.end_time.to_rfc3339(),
            "location": event.location,
            "recurrence": event.recurrence.as_str(),
            "status": event.status,
        }))
        .with_observation(format!(
            "Scheduled '{}' from {} to {} (survives restarts)",
            event.title,
            event.start_time.format("%Y-%m-%d %H:%M UTC"),
            event.end_time.format("%H:%M UTC")
        )))
    }
}

/// Tool to list scheduled calendar events
pub struct CalendarListEventsTool {
    metadata: ToolMetadata,
}

impl CalendarListEventsTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "calendar_list_events",
                "List scheduled calendar events from persistent SQLite storage",
                ToolCategory::System,
                RiskTier::LowRisk,
            )
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "start_time": { "type": "string", "description": "Optional RFC3339 window start (defaults to now)" },
                    "lookahead_days": { "type": "integer", "description": "Number of days ahead to search (default 7)" }
                }
            })),
        }
    }
}

impl Default for CalendarListEventsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CalendarListEventsTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        // timer
        let now = Utc::now();

        let start_dt = if let Some(s) = params.get("start_time").and_then(|v| v.as_str()) {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or(now)
        } else {
            now
        };

        let days = params
            .get("lookahead_days")
            .and_then(|v| v.as_i64())
            .unwrap_or(7);
        let end_dt = start_dt + Duration::days(days);

        let cal = get_calendar();
        let scopes = ScopeSet::full_access();

        let events = cal
            .list_events(start_dt, end_dt, &scopes, Some(&ctx.session_id))
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to list events: {e}")))?;

        let count = events.len();
        let ev_values: Vec<serde_json::Value> = events
            .into_iter()
            .map(|ev| {
                json!({
                    "id": ev.id,
                    "title": ev.title,
                    "start_time": ev.start_time.to_rfc3339(),
                    "end_time": ev.end_time.to_rfc3339(),
                    "location": ev.location,
                    "recurrence": ev.recurrence.as_str(),
                    "status": ev.status,
                })
            })
            .collect();

        Ok(
            ToolResult::success(json!({ "events": ev_values, "count": count })).with_observation(
                format!("Found {} calendar events in the next {} days", count, days),
            ),
        )
    }
}

/// Tool to delete a scheduled calendar event by ID
pub struct CalendarDeleteEventTool {
    metadata: ToolMetadata,
}

impl CalendarDeleteEventTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "calendar_delete_event",
                "Delete a scheduled calendar event by ID",
                ToolCategory::System,
                RiskTier::Modify,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["event_id"],
                "properties": {
                    "event_id": { "type": "string", "description": "Unique UUID of the event to delete" }
                }
            })),
        }
    }
}

impl Default for CalendarDeleteEventTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CalendarDeleteEventTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        // timer
        let event_id = params
            .get("event_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'event_id'".into()))?;

        let cal = get_calendar();
        let scopes = ScopeSet::full_access();

        let deleted = cal
            .delete_event(event_id, &scopes, Some(&ctx.session_id))
            .map_err(|e| ToolError::ExecutionFailed(format!("Delete error: {e}")))?;

        Ok(
            ToolResult::success(json!({ "event_id": event_id, "deleted": deleted }))
                .with_observation(if deleted {
                    format!("Calendar event '{}' successfully deleted", event_id)
                } else {
                    format!("Calendar event '{}' was not found", event_id)
                }),
        )
    }
}
