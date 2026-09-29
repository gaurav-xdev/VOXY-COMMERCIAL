use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use crate::audit::{AuditStatus, OfficeAuditEvent, OfficeAuditTrail};
use crate::error::{OfficeError, Result};
use crate::scope::{OfficeScope, ScopeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub attendees: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CalendarConnector {
    events: Arc<RwLock<Vec<CalendarEvent>>>,
    audit_trail: Arc<OfficeAuditTrail>,
}

impl CalendarConnector {
    pub fn new(audit_trail: Arc<OfficeAuditTrail>) -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
            audit_trail,
        }
    }

    /// Add an event to calendar after checking permissions and conflict detection.
    pub fn add_event(
        &self,
        title: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        location: Option<&str>,
        description: Option<&str>,
        attendees: Vec<String>,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<CalendarEvent> {
        if let Err(e) = scopes.check(OfficeScope::CalendarWrite) {
            self.audit_trail.record(OfficeAuditEvent::new(
                "Calendar",
                "add_event",
                title,
                user_id,
                OfficeScope::CalendarWrite.as_str(),
                AuditStatus::Denied,
                serde_json::json!({ "reason": e.to_string() }),
            ));
            return Err(e);
        }

        if end_time <= start_time {
            return Err(OfficeError::InvalidData(
                "Event end time must be after start time".to_string(),
            ));
        }

        // Conflict check against existing events
        let mut events = self.events.write();
        for existing in events.iter() {
            if start_time < existing.end_time && end_time > existing.start_time {
                let err = OfficeError::ConflictDetected {
                    conflict_title: existing.title.clone(),
                    start: existing.start_time.to_rfc3339(),
                    end: existing.end_time.to_rfc3339(),
                };
                self.audit_trail.record(OfficeAuditEvent::new(
                    "Calendar",
                    "add_event",
                    title,
                    user_id,
                    OfficeScope::CalendarWrite.as_str(),
                    AuditStatus::Failed,
                    serde_json::json!({ "conflict_with": existing.title }),
                ));
                return Err(err);
            }
        }

        let new_event = CalendarEvent {
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            start_time,
            end_time,
            location: location.map(|s| s.to_string()),
            description: description.map(|s| s.to_string()),
            attendees,
        };

        events.push(new_event.clone());

        self.audit_trail.record(OfficeAuditEvent::new(
            "Calendar",
            "add_event",
            title,
            user_id,
            OfficeScope::CalendarWrite.as_str(),
            AuditStatus::Success,
            serde_json::json!({ "event_id": new_event.id, "start": start_time, "end": end_time }),
        ));

        Ok(new_event)
    }

    /// Query events within a specific time window.
    pub fn get_events_between(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<Vec<CalendarEvent>> {
        if let Err(e) = scopes.check(OfficeScope::CalendarRead) {
            self.audit_trail.record(OfficeAuditEvent::new(
                "Calendar",
                "get_events_between",
                "time_window",
                user_id,
                OfficeScope::CalendarRead.as_str(),
                AuditStatus::Denied,
                serde_json::json!({ "reason": e.to_string() }),
            ));
            return Err(e);
        }

        let events = self.events.read();
        let matches: Vec<CalendarEvent> = events
            .iter()
            .filter(|e| e.end_time >= from && e.start_time <= to)
            .cloned()
            .collect();

        self.audit_trail.record(OfficeAuditEvent::new(
            "Calendar",
            "get_events_between",
            "time_window",
            user_id,
            OfficeScope::CalendarRead.as_str(),
            AuditStatus::Success,
            serde_json::json!({ "matched": matches.len() }),
        ));

        Ok(matches)
    }
}
