use crate::error::{Result, StudioError};
use crate::security::{EnclaveAccessController, OSMOO_OPERATOR_IDENTITY};
use std::path::{Path, PathBuf};

/// Manages the physical layout and access boundary of the installed OpenMontage assets.
#[derive(Debug, Clone)]
pub struct VideoStudioEnclave {
    enclave_root: PathBuf,
    controller: EnclaveAccessController,
}

impl VideoStudioEnclave {
    /// Discover or initialize the video studio enclave at the specified path.
    pub fn new(enclave_root: impl Into<PathBuf>) -> Self {
        let root = enclave_root.into();
        let controller = EnclaveAccessController::new(&root);
        Self {
            enclave_root: root,
            controller,
        }
    }

    /// Access controller for authorization verification.
    pub fn access_controller(&self) -> &EnclaveAccessController {
        &self.controller
    }

    /// Path to installed skills.
    pub fn skills_path(&self) -> PathBuf {
        self.enclave_root.join("skills")
    }

    /// Path to installed tools.
    pub fn tools_path(&self) -> PathBuf {
        self.enclave_root.join("tools")
    }

    /// Path to installed pipeline definitions.
    pub fn pipelines_path(&self) -> PathBuf {
        self.enclave_root.join("pipeline_defs")
    }

    /// Path to installed Remotion composition studio.
    pub fn remotion_path(&self) -> PathBuf {
        self.enclave_root.join("remotion-composer")
    }

    /// Path to JSON schemas.
    pub fn schemas_path(&self) -> PathBuf {
        self.enclave_root.join("schemas")
    }

    /// Isolated scratchpad workspace for in-flight video production.
    pub fn workspace_path(&self) -> PathBuf {
        self.enclave_root.join("workspace")
    }

    /// Initialize and secure the workspace directory.
    pub fn initialize(&self, token: &str) -> Result<()> {
        self.controller
            .verify_access(OSMOO_OPERATOR_IDENTITY, token)?;
        let ws = self.workspace_path();
        self.controller.secure_workspace_permissions(&ws)?;
        Ok(())
    }

    /// Enumerate all installed production pipelines (YAML).
    pub fn list_pipelines(&self, token: &str) -> Result<Vec<String>> {
        self.controller
            .verify_access(OSMOO_OPERATOR_IDENTITY, token)?;
        let p_path = self.pipelines_path();
        if !p_path.exists() {
            return Ok(Vec::new());
        }

        let mut pipelines = Vec::new();
        for entry in std::fs::read_dir(p_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    pipelines.push(stem.to_string());
                }
            }
        }
        pipelines.sort();
        Ok(pipelines)
    }

    /// Enumerate installed skills across core, creative, and pipelines.
    pub fn list_skills(&self, token: &str) -> Result<Vec<String>> {
        self.controller
            .verify_access(OSMOO_OPERATOR_IDENTITY, token)?;
        let s_path = self.skills_path();
        if !s_path.exists() {
            return Ok(Vec::new());
        }

        let mut skills = Vec::new();
        collect_markdown_files(&s_path, &s_path, &mut skills);
        skills.sort();
        Ok(skills)
    }

    /// Enumerate installed tool modules.
    pub fn list_tools(&self, token: &str) -> Result<Vec<String>> {
        self.controller
            .verify_access(OSMOO_OPERATOR_IDENTITY, token)?;
        let t_path = self.tools_path();
        if !t_path.exists() {
            return Ok(Vec::new());
        }

        let mut tools = Vec::new();
        for entry in std::fs::read_dir(t_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    tools.push(name.to_string());
                }
            }
        }
        tools.sort();
        Ok(tools)
    }

    /// Load the raw YAML playbook for a pipeline.
    pub fn get_pipeline_yaml(&self, pipeline_name: &str, token: &str) -> Result<String> {
        self.controller
            .verify_access(OSMOO_OPERATOR_IDENTITY, token)?;
        let path = self.pipelines_path().join(format!("{pipeline_name}.yaml"));
        if !path.exists() {
            return Err(StudioError::PipelineError(format!(
                "Pipeline definition '{pipeline_name}' not found at {:?}",
                path
            )));
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(content)
    }
}

fn collect_markdown_files(root: &Path, dir: &Path, list: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_markdown_files(root, &path, list);
            } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
                if let Ok(rel) = path.strip_prefix(root) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    let name = rel_str.trim_end_matches(".md");
                    list.push(name.to_string());
                }
            }
        }
    }
}
