use crate::types::{HarnessError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransactionStatus {
    Active,
    Committed,
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournaledFile {
    pub rel_path: String,
    pub existed_before: bool,
    pub backup_file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionManifest {
    pub tx_id: Uuid,
    pub status: TransactionStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub files: Vec<JournaledFile>,
}

pub struct TransactionJournal {
    workspace_root: PathBuf,
    journal_root: PathBuf,
}

pub struct ActiveTransaction {
    pub manifest: TransactionManifest,
    tx_dir: PathBuf,
    workspace_root: PathBuf,
}

impl TransactionJournal {
    pub fn new(workspace_root: &Path) -> Self {
        let journal_root = workspace_root.join(".osmoo").join("journal");
        Self {
            workspace_root: workspace_root.to_path_buf(),
            journal_root,
        }
    }

    pub fn begin(&self) -> Result<ActiveTransaction> {
        let tx_id = Uuid::new_v4();
        let tx_dir = self.journal_root.join(tx_id.to_string());
        fs::create_dir_all(tx_dir.join("pre_images"))
            .map_err(|e| HarnessError::TransactionError(format!("Failed to create journal dir: {}", e)))?;

        let manifest = TransactionManifest {
            tx_id,
            status: TransactionStatus::Active,
            started_at: Utc::now(),
            completed_at: None,
            files: Vec::new(),
        };

        let manifest_bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|e| HarnessError::TransactionError(format!("Manifest serialization: {}", e)))?;
        fs::write(tx_dir.join("manifest.json"), manifest_bytes)
            .map_err(|e| HarnessError::TransactionError(format!("Manifest write: {}", e)))?;

        Ok(ActiveTransaction {
            manifest,
            tx_dir,
            workspace_root: self.workspace_root.clone(),
        })
    }

    pub fn recover_pending_transactions(&self) -> Result<Vec<Uuid>> {
        let mut recovered = Vec::new();
        if !self.journal_root.exists() {
            return Ok(recovered);
        }

        let entries = fs::read_dir(&self.journal_root)
            .map_err(|e| HarnessError::TransactionError(format!("Failed to read journal dir: {}", e)))?;

        for entry in entries.flatten() {
            let tx_dir = entry.path();
            let manifest_path = tx_dir.join("manifest.json");
            if manifest_path.exists() {
                if let Ok(content) = fs::read_to_string(&manifest_path) {
                    if let Ok(manifest) = serde_json::from_str::<TransactionManifest>(&content) {
                        if manifest.status == TransactionStatus::Active {
                            tracing::warn!(
                                tx_id = %manifest.tx_id,
                                "Found incomplete transaction on startup; rolling back..."
                            );
                            let mut tx = ActiveTransaction {
                                manifest: manifest.clone(),
                                tx_dir: tx_dir.clone(),
                                workspace_root: self.workspace_root.clone(),
                            };
                            if let Ok(()) = tx.rollback() {
                                recovered.push(manifest.tx_id);
                            }
                        }
                    }
                }
            }
        }

        Ok(recovered)
    }
}

impl ActiveTransaction {
    pub fn record_before_modify(&mut self, rel_path: &Path) -> Result<()> {
        let rel_str = rel_path.to_string_lossy().to_string();
        if self.manifest.files.iter().any(|f| f.rel_path == rel_str) {
            return Ok(()); // Already backed up in this transaction
        }

        let full_target = self.workspace_root.join(rel_path);
        let existed = full_target.exists();
        let backup_name = format!("{}_{}", self.manifest.files.len(), rel_path.file_name().unwrap_or_default().to_string_lossy());
        let backup_path = self.tx_dir.join("pre_images").join(&backup_name);

        if existed {
            fs::copy(&full_target, &backup_path)
                .map_err(|e| HarnessError::TransactionError(format!("Pre-image backup failed: {}", e)))?;
        }

        self.manifest.files.push(JournaledFile {
            rel_path: rel_str,
            existed_before: existed,
            backup_file_name: backup_name,
        });

        self.save_manifest()
    }

    pub fn commit(&mut self) -> Result<()> {
        self.manifest.status = TransactionStatus::Committed;
        self.manifest.completed_at = Some(Utc::now());
        self.save_manifest()
    }

    pub fn rollback(&mut self) -> Result<()> {
        tracing::info!(tx_id = %self.manifest.tx_id, "Rolling back transaction files...");
        for file in self.manifest.files.iter().rev() {
            let target_path = self.workspace_root.join(&file.rel_path);
            if file.existed_before {
                let backup_path = self.tx_dir.join("pre_images").join(&file.backup_file_name);
                if backup_path.exists() {
                    if let Some(parent) = target_path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    if let Err(e) = fs::copy(&backup_path, &target_path) {
                        tracing::error!(file = %file.rel_path, error = %e, "Rollback copy failed");
                    }
                }
            } else if target_path.exists() {
                let _ = fs::remove_file(&target_path);
            }
        }

        self.manifest.status = TransactionStatus::RolledBack;
        self.manifest.completed_at = Some(Utc::now());
        self.save_manifest()
    }

    fn save_manifest(&self) -> Result<()> {
        let manifest_bytes = serde_json::to_vec_pretty(&self.manifest)
            .map_err(|e| HarnessError::TransactionError(format!("Manifest serialization: {}", e)))?;
        fs::write(self.tx_dir.join("manifest.json"), manifest_bytes)
            .map_err(|e| HarnessError::TransactionError(format!("Manifest write: {}", e)))?;
        Ok(())
    }
}
