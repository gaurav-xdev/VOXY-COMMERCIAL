use crate::audit::{AuditStatus, OfficeAuditEvent, OfficeAuditTrail};
use crate::error::{OfficeError, Result};
use crate::scope::{OfficeScope, ScopeSet};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecurrenceRule {
    None,
    Daily,
    Weekly,
    Monthly,
    Weekdays,
}

impl RecurrenceRule {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecurrenceRule::None => "none",
            RecurrenceRule::Daily => "daily",
            RecurrenceRule::Weekly => "weekly",
            RecurrenceRule::Monthly => "monthly",
            RecurrenceRule::Weekdays => "weekdays",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "daily" => RecurrenceRule::Daily,
            "weekly" => RecurrenceRule::Weekly,
            "monthly" => RecurrenceRule::Monthly,
            "weekdays" => RecurrenceRule::Weekdays,
            _ => RecurrenceRule::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub attendees: Vec<String>,
    pub recurrence: RecurrenceRule,
    pub status: String,                // "confirmed", "tentative", "cancelled"
    pub sync_provider: Option<String>, // "google", "outlook", "local"
    pub sync_id: Option<String>,
    pub last_synced: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub provider: String,
    pub total_fetched: usize,
    pub inserted: usize,
    pub updated: usize,
    pub conflicts: usize,
    pub status: String,
}

#[derive(Clone)]
pub struct CalendarConnector {
    db_conn: Arc<Mutex<Connection>>,
    db_path: Option<PathBuf>,
    audit_trail: Arc<OfficeAuditTrail>,
}

impl std::fmt::Debug for CalendarConnector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CalendarConnector")
            .field("db_path", &self.db_path)
            .finish()
    }
}

impl CalendarConnector {
    /// In-memory SQLite connection for isolated testing
    pub fn new(audit_trail: Arc<OfficeAuditTrail>) -> Self {
        let conn = Connection::open_in_memory().expect("Failed to open in-memory SQLite calendar");
        Self::init_schema(&conn).expect("Failed to init calendar schema");
        Self {
            db_conn: Arc::new(Mutex::new(conn)),
            db_path: None,
            audit_trail,
        }
    }

    /// Persistent SQLite storage on disk surviving application restarts
    pub fn with_sqlite<P: AsRef<Path>>(
        path: P,
        audit_trail: Arc<OfficeAuditTrail>,
    ) -> Result<Self> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(p)
            .map_err(|e| OfficeError::Database(format!("SQLite open error: {e}")))?;
        Self::init_schema(&conn)?;
        Ok(Self {
            db_conn: Arc::new(Mutex::new(conn)),
            db_path: Some(p.to_path_buf()),
            audit_trail,
        })
    }

    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r#"CREATE TABLE IF NOT EXISTS calendar_events (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                start_time TEXT NOT NULL,
                end_time TEXT NOT NULL,
                location TEXT,
                description TEXT,
                attendees TEXT NOT NULL,
                recurrence TEXT NOT NULL,
                status TEXT NOT NULL,
                sync_provider TEXT,
                sync_id TEXT,
                last_synced TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_calendar_start ON calendar_events(start_time);
            CREATE INDEX IF NOT EXISTS idx_calendar_sync ON calendar_events(sync_provider, sync_id);
            "#,
        )
        .map_err(|e| OfficeError::Database(format!("Schema initialization failed: {e}")))?;
        Ok(())
    }

    /// Check if another event overlaps with the requested time range
    pub fn detect_conflicts(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        exclude_id: Option<&str>,
    ) -> Result<Option<CalendarEvent>> {
        let conn = self.db_conn.lock();
        let start_str = start_time.to_rfc3339();
        let end_str = end_time.to_rfc3339();

        let sql = "SELECT id, title, start_time, end_time, location, description, attendees, recurrence, status, sync_provider, sync_id, last_synced
                 FROM calendar_events
                 WHERE status != 'cancelled'
                   AND (?1 IS NULL OR id != ?1)
                   AND start_time < ?2
                   AND end_time > ?3
                 LIMIT 1";

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| OfficeError::Database(format!("Conflict check query error: {e}")))?;

        let mut rows = stmt
            .query(rusqlite::params![exclude_id, end_str, start_str])
            .map_err(|e| OfficeError::Database(format!("Query execution error: {e}")))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| OfficeError::Database(e.to_string()))?
        {
            let id: String = row.get(0).unwrap_or_default();
            let title: String = row.get(1).unwrap_or_default();
            let s_str: String = row.get(2).unwrap_or_default();
            let e_str: String = row.get(3).unwrap_or_default();
            let loc: Option<String> = row.get(4).ok();
            let desc: Option<String> = row.get(5).ok();
            let att_json: String = row.get(6).unwrap_or_else(|_| "[]".into());
            let rec_str: String = row.get(7).unwrap_or_else(|_| "none".into());
            let status: String = row.get(8).unwrap_or_else(|_| "confirmed".into());
            let sp: Option<String> = row.get(9).ok();
            let sid: Option<String> = row.get(10).ok();
            let ls_str: Option<String> = row.get(11).ok();

            let st = DateTime::parse_from_rfc3339(&s_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or(start_time);
            let et = DateTime::parse_from_rfc3339(&e_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or(end_time);
            let att: Vec<String> = serde_json::from_str(&att_json).unwrap_or_default();

            return Ok(Some(CalendarEvent {
                id,
                title,
                start_time: st,
                end_time: et,
                location: loc,
                description: desc,
                attendees: att,
                recurrence: RecurrenceRule::from_str(&rec_str),
                status,
                sync_provider: sp,
                sync_id: sid,
                last_synced: ls_str.as_deref().and_then(|s| {
                    DateTime::parse_from_rfc3339(s)
                        .ok()
                        .map(|d| d.with_timezone(&Utc))
                }),
            }));
        }

        Ok(None)
    }

    /// Add an event to calendar after checking permissions and conflict detection.
    #[allow(clippy::too_many_arguments)]
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
        self.add_event_with_recurrence(
            title,
            start_time,
            end_time,
            location,
            description,
            attendees,
            RecurrenceRule::None,
            scopes,
            user_id,
        )
    }

    /// Add event with explicit recurrence rule
    #[allow(clippy::too_many_arguments)]
    pub fn add_event_with_recurrence(
        &self,
        title: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        location: Option<&str>,
        description: Option<&str>,
        attendees: Vec<String>,
        recurrence: RecurrenceRule,
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

        // Conflict check against persistent SQLite database
        if let Some(existing) = self.detect_conflicts(start_time, end_time, None)? {
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

        let new_event = CalendarEvent {
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            start_time,
            end_time,
            location: location.map(|s| s.to_string()),
            description: description.map(|s| s.to_string()),
            attendees,
            recurrence,
            status: "confirmed".to_string(),
            sync_provider: Some("local".to_string()),
            sync_id: None,
            last_synced: Some(Utc::now()),
        };

        {
            let conn = self.db_conn.lock();
            let att_json =
                serde_json::to_string(&new_event.attendees).unwrap_or_else(|_| "[]".into());
            conn.execute(
                "INSERT INTO calendar_events (id, title, start_time, end_time, location, description, attendees, recurrence, status, sync_provider, sync_id, last_synced)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    new_event.id,
                    new_event.title,
                    new_event.start_time.to_rfc3339(),
                    new_event.end_time.to_rfc3339(),
                    new_event.location,
                    new_event.description,
                    att_json,
                    new_event.recurrence.as_str(),
                    new_event.status,
                    new_event.sync_provider,
                    new_event.sync_id,
                    new_event.last_synced.map(|t| t.to_rfc3339()),
                ],
            )
            .map_err(|e| OfficeError::Database(format!("Failed to insert event: {e}")))?;
        }

        self.audit_trail.record(OfficeAuditEvent::new(
            "Calendar",
            "add_event",
            title,
            user_id,
            OfficeScope::CalendarWrite.as_str(),
            AuditStatus::Success,
            serde_json::json!({
                "id": new_event.id,
                "start": new_event.start_time.to_rfc3339(),
                "end": new_event.end_time.to_rfc3339()
            }),
        ));

        Ok(new_event)
    }

    /// List events scheduled in a given time window from persistent SQLite storage
    pub fn list_events(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<Vec<CalendarEvent>> {
        if let Err(e) = scopes.check(OfficeScope::CalendarRead) {
            self.audit_trail.record(OfficeAuditEvent::new(
                "Calendar",
                "list_events",
                "time_window",
                user_id,
                OfficeScope::CalendarRead.as_str(),
                AuditStatus::Failed,
                serde_json::json!({ "error": e.to_string() }),
            ));
            return Err(e);
        }

        let conn = self.db_conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, title, start_time, end_time, location, description, attendees, recurrence, status, sync_provider, sync_id, last_synced
                 FROM calendar_events
                 WHERE status != 'cancelled'
                   AND start_time <= ?1
                   AND end_time >= ?2
                 ORDER BY start_time ASC",
            )
            .map_err(|e| OfficeError::Database(format!("Query preparation error: {e}")))?;

        let rows = stmt
            .query_map(params![end.to_rfc3339(), start.to_rfc3339()], |row| {
                let id: String = row.get(0)?;
                let title: String = row.get(1)?;
                let s_str: String = row.get(2)?;
                let e_str: String = row.get(3)?;
                let loc: Option<String> = row.get(4)?;
                let desc: Option<String> = row.get(5)?;
                let att_json: String = row.get(6)?;
                let rec_str: String = row.get(7)?;
                let status: String = row.get(8)?;
                let sp: Option<String> = row.get(9)?;
                let sid: Option<String> = row.get(10)?;
                let ls_str: Option<String> = row.get(11)?;

                let st = DateTime::parse_from_rfc3339(&s_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or(start);
                let et = DateTime::parse_from_rfc3339(&e_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or(end);
                let att: Vec<String> = serde_json::from_str(&att_json).unwrap_or_default();

                Ok(CalendarEvent {
                    id,
                    title,
                    start_time: st,
                    end_time: et,
                    location: loc,
                    description: desc,
                    attendees: att,
                    recurrence: RecurrenceRule::from_str(&rec_str),
                    status,
                    sync_provider: sp,
                    sync_id: sid,
                    last_synced: ls_str.as_deref().and_then(|s| {
                        DateTime::parse_from_rfc3339(s)
                            .ok()
                            .map(|d| d.with_timezone(&Utc))
                    }),
                })
            })
            .map_err(|e| OfficeError::Database(format!("Query map error: {e}")))?;

        let mut results = Vec::new();
        for r in rows {
            if let Ok(ev) = r {
                results.push(ev);
            }
        }

        self.audit_trail.record(OfficeAuditEvent::new(
            "Calendar",
            "list_events",
            "time_window",
            user_id,
            OfficeScope::CalendarRead.as_str(),
            AuditStatus::Success,
            serde_json::json!({ "events_count": results.len() }),
        ));

        Ok(results)
    }

    /// Delete event by ID from persistent storage
    pub fn delete_event(
        &self,
        event_id: &str,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<bool> {
        if let Err(e) = scopes.check(OfficeScope::CalendarWrite) {
            self.audit_trail.record(OfficeAuditEvent::new(
                "Calendar",
                "delete_event",
                event_id,
                user_id,
                OfficeScope::CalendarWrite.as_str(),
                AuditStatus::Failed,
                serde_json::json!({ "error": e.to_string() }),
            ));
            return Err(e);
        }

        let conn = self.db_conn.lock();
        let rows_affected = conn
            .execute(
                "DELETE FROM calendar_events WHERE id = ?1",
                params![event_id],
            )
            .map_err(|e| OfficeError::Database(format!("Delete error: {e}")))?;

        let deleted = rows_affected > 0;
        self.audit_trail.record(OfficeAuditEvent::new(
            "Calendar",
            "delete_event",
            event_id,
            user_id,
            OfficeScope::CalendarWrite.as_str(),
            if deleted {
                AuditStatus::Success
            } else {
                AuditStatus::Failed
            },
            serde_json::json!({ "deleted": deleted }),
        ));

        Ok(deleted)
    }

    /// Retrieve upcoming reminders starting within `lookahead_minutes`
    pub fn get_upcoming_reminders(
        &self,
        now: DateTime<Utc>,
        lookahead_minutes: i64,
    ) -> Result<Vec<CalendarEvent>> {
        let future = now + chrono::Duration::minutes(lookahead_minutes);
        let scopes = ScopeSet::read_only();
        self.list_events(now, future, &scopes, Some("system_monitor"))
    }

    /// Synchronize with external calendar provider (Google Calendar or Microsoft 365 Outlook)
    pub async fn sync_remote_provider(
        &self,
        provider: &str,
        access_token: &str,
        remote_events: Vec<CalendarEvent>,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<SyncResult> {
        if let Err(e) = scopes.check(OfficeScope::CalendarWrite) {
            self.audit_trail.record(OfficeAuditEvent::new(
                "Calendar",
                "sync_remote_provider",
                provider,
                user_id,
                OfficeScope::CalendarWrite.as_str(),
                AuditStatus::Failed,
                serde_json::json!({ "error": e.to_string() }),
            ));
            return Err(e);
        }

        if access_token.is_empty() {
            return Err(OfficeError::PermissionDenied {
                required: "OAuth Access Token".to_string(),
            });
        }

        let mut inserted = 0;
        let mut updated = 0;
        let mut conflicts = 0;
        let total = remote_events.len();

        for ev in remote_events {
            // Check for duplicate by sync_id
            let existing_by_sync: Option<String> = {
                let conn = self.db_conn.lock();
                if let Some(ref sid) = ev.sync_id {
                    conn.query_row(
                        "SELECT id FROM calendar_events WHERE sync_provider = ?1 AND sync_id = ?2",
                        params![provider, sid],
                        |row| row.get(0),
                    )
                    .ok()
                } else {
                    None
                }
            };

            if let Some(local_id) = existing_by_sync {
                // Update existing event
                let conn = self.db_conn.lock();
                let att_json = serde_json::to_string(&ev.attendees).unwrap_or_else(|_| "[]".into());
                if let Ok(rows) = conn.execute(
                    "UPDATE calendar_events
                     SET title = ?1, start_time = ?2, end_time = ?3, location = ?4, description = ?5, attendees = ?6, last_synced = ?7
                     WHERE id = ?8",
                    params![
                        ev.title,
                        ev.start_time.to_rfc3339(),
                        ev.end_time.to_rfc3339(),
                        ev.location,
                        ev.description,
                        att_json,
                        Utc::now().to_rfc3339(),
                        local_id
                    ],
                ) {
                    if rows > 0 {
                        updated += 1;
                    }
                }
            } else {
                // Check conflict
                if let Ok(Some(_)) = self.detect_conflicts(ev.start_time, ev.end_time, None) {
                    conflicts += 1;
                }

                let conn = self.db_conn.lock();
                let att_json = serde_json::to_string(&ev.attendees).unwrap_or_else(|_| "[]".into());
                if let Ok(rows) = conn.execute(
                    "INSERT INTO calendar_events (id, title, start_time, end_time, location, description, attendees, recurrence, status, sync_provider, sync_id, last_synced)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        ev.id,
                        ev.title,
                        ev.start_time.to_rfc3339(),
                        ev.end_time.to_rfc3339(),
                        ev.location,
                        ev.description,
                        att_json,
                        ev.recurrence.as_str(),
                        ev.status,
                        provider,
                        ev.sync_id,
                        Utc::now().to_rfc3339(),
                    ],
                ) {
                    if rows > 0 {
                        inserted += 1;
                    }
                }
            }
        }

        let res = SyncResult {
            provider: provider.to_string(),
            total_fetched: total,
            inserted,
            updated,
            conflicts,
            status: "synced".to_string(),
        };

        self.audit_trail.record(OfficeAuditEvent::new(
            "Calendar",
            "sync_remote_provider",
            provider,
            user_id,
            OfficeScope::CalendarWrite.as_str(),
            AuditStatus::Success,
            serde_json::to_value(&res).unwrap_or_default(),
        ));

        Ok(res)
    }
}
