use crate::types::{CodingSession, HarnessError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct SessionStore {
    sessions_dir: PathBuf,
}

impl SessionStore {
    pub fn new(workspace_root: &Path) -> Self {
        let sessions_dir = workspace_root.join(".osmoo").join("sessions");
        Self { sessions_dir }
    }

    pub fn save_session(&self, session: &CodingSession) -> Result<()> {
        fs::create_dir_all(&self.sessions_dir).map_err(HarnessError::Io)?;

        let file_path = self
            .sessions_dir
            .join(format!("{}.json", session.session_id));
        let data = serde_json::to_vec_pretty(session).map_err(HarnessError::Json)?;

        fs::write(file_path, data).map_err(HarnessError::Io)?;

        Ok(())
    }

    pub fn load_session(&self, session_id: &Uuid) -> Result<Option<CodingSession>> {
        let file_path = self.sessions_dir.join(format!("{}.json", session_id));
        if !file_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(file_path).map_err(HarnessError::Io)?;

        let session = serde_json::from_str(&content).map_err(HarnessError::Json)?;

        Ok(Some(session))
    }
}
