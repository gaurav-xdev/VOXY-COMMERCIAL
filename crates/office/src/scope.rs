use crate::error::{OfficeError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Granular permission scopes for enterprise office automation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OfficeScope {
    FilesRead,
    FilesWrite,
    FilesDelete,
    EmailRead,
    EmailDraft,
    EmailSend,
    CalendarRead,
    CalendarWrite,
    SpreadsheetRead,
    SpreadsheetWrite,
    SpreadsheetExport,
    DocumentRead,
    DocumentWrite,
}

impl OfficeScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FilesRead => "files:read",
            Self::FilesWrite => "files:write",
            Self::FilesDelete => "files:delete",
            Self::EmailRead => "email:read",
            Self::EmailDraft => "email:draft",
            Self::EmailSend => "email:send",
            Self::CalendarRead => "calendar:read",
            Self::CalendarWrite => "calendar:write",
            Self::SpreadsheetRead => "spreadsheet:read",
            Self::SpreadsheetWrite => "spreadsheet:write",
            Self::SpreadsheetExport => "spreadsheet:export",
            Self::DocumentRead => "document:read",
            Self::DocumentWrite => "document:write",
        }
    }
}

/// A validated collection of scopes assigned to a session or connector call.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScopeSet {
    scopes: HashSet<OfficeScope>,
}

impl ScopeSet {
    pub fn new() -> Self {
        Self {
            scopes: HashSet::new(),
        }
    }

    pub fn full_access() -> Self {
        let mut s = HashSet::new();
        s.insert(OfficeScope::FilesRead);
        s.insert(OfficeScope::FilesWrite);
        s.insert(OfficeScope::FilesDelete);
        s.insert(OfficeScope::EmailRead);
        s.insert(OfficeScope::EmailDraft);
        s.insert(OfficeScope::EmailSend);
        s.insert(OfficeScope::CalendarRead);
        s.insert(OfficeScope::CalendarWrite);
        s.insert(OfficeScope::SpreadsheetRead);
        s.insert(OfficeScope::SpreadsheetWrite);
        s.insert(OfficeScope::SpreadsheetExport);
        s.insert(OfficeScope::DocumentRead);
        s.insert(OfficeScope::DocumentWrite);
        Self { scopes: s }
    }

    pub fn read_only() -> Self {
        let mut s = HashSet::new();
        s.insert(OfficeScope::FilesRead);
        s.insert(OfficeScope::EmailRead);
        s.insert(OfficeScope::CalendarRead);
        s.insert(OfficeScope::SpreadsheetRead);
        s.insert(OfficeScope::DocumentRead);
        Self { scopes: s }
    }

    pub fn grant(&mut self, scope: OfficeScope) {
        self.scopes.insert(scope);
    }

    pub fn revoke(&mut self, scope: &OfficeScope) {
        self.scopes.remove(scope);
    }

    pub fn contains(&self, scope: OfficeScope) -> bool {
        self.scopes.contains(&scope)
    }

    pub fn check(&self, required: OfficeScope) -> Result<()> {
        if self.contains(required) {
            Ok(())
        } else {
            Err(OfficeError::PermissionDenied {
                required: required.as_str().to_string(),
            })
        }
    }
}
