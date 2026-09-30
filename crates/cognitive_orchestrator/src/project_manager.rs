//! Project Manager — understands projects, milestones, repos, deadlines.
//!
//! Tracks projects with:
//! - Files, repositories, branches, commits
//! - Versions and deadlines
//! - Risks and dependencies
//! - Milestones and progress

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Core Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectId(pub String);

impl Default for ProjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MilestoneId(pub String);

impl Default for MilestoneId {
    fn default() -> Self {
        Self::new()
    }
}

impl MilestoneId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepositoryId(pub String);

/// Project status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectStatus {
    Planning,
    Active,
    OnHold,
    Completed,
    Cancelled,
}

/// Risk level.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum RiskLevel {
    #[default]
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

/// A project milestone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMilestone {
    pub id: MilestoneId,
    pub title: String,
    pub description: String,
    pub due_date: Option<chrono::DateTime<chrono::Utc>>,
    pub completed: bool,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub progress: f32,
}

/// A file tracked in the project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub path: String,
    pub file_type: String,
    pub last_modified: chrono::DateTime<chrono::Utc>,
    pub size_bytes: u64,
    pub version: u32,
}

/// A git repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: RepositoryId,
    pub name: String,
    pub url: String,
    pub branch: String,
    pub last_commit: Option<String>,
    pub last_commit_message: Option<String>,
    pub last_commit_time: Option<chrono::DateTime<chrono::Utc>>,
    pub dirty: bool,
    pub ahead: u32,
    pub behind: u32,
}

/// A risk identified in the project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRisk {
    pub id: String,
    pub title: String,
    pub description: String,
    pub level: RiskLevel,
    pub mitigation: Option<String>,
    pub owner: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// A project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub description: String,
    pub status: ProjectStatus,
    pub progress: f32,
    pub start_date: Option<chrono::DateTime<chrono::Utc>>,
    pub deadline: Option<chrono::DateTime<chrono::Utc>>,
    pub estimated_completion: Option<chrono::DateTime<chrono::Utc>>,
    pub milestones: Vec<ProjectMilestone>,
    pub files: Vec<ProjectFile>,
    pub repositories: Vec<Repository>,
    pub risks: Vec<ProjectRisk>,
    pub dependencies: Vec<ProjectId>,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Project summary for dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: ProjectId,
    pub name: String,
    pub status: ProjectStatus,
    pub progress: f32,
    pub milestone_count: usize,
    pub completed_milestones: usize,
    pub risk_count: usize,
    pub high_risk_count: usize,
    pub days_until_deadline: Option<i64>,
}

// ============================================================================
// Project Manager
// ============================================================================

/// Manages projects, milestones, repositories, and risks.
pub struct ProjectManager {
    projects: HashMap<ProjectId, Project>,
    max_projects: usize,
}

impl ProjectManager {
    pub fn new(max_projects: usize) -> Self {
        Self {
            projects: HashMap::new(),
            max_projects,
        }
    }

    pub fn default_manager() -> Self {
        Self::new(50)
    }

    /// Create a new project.
    pub fn create_project(
        &mut self,
        name: String,
        description: String,
    ) -> Result<ProjectId, ProjectError> {
        if self.projects.len() >= self.max_projects {
            return Err(ProjectError::CapacityReached(self.max_projects));
        }

        let id = ProjectId::new();
        let now = chrono::Utc::now();
        let project = Project {
            id: id.clone(),
            name,
            description,
            status: ProjectStatus::Planning,
            progress: 0.0,
            start_date: None,
            deadline: None,
            estimated_completion: None,
            milestones: Vec::new(),
            files: Vec::new(),
            repositories: Vec::new(),
            risks: Vec::new(),
            dependencies: Vec::new(),
            tags: Vec::new(),
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        };

        self.projects.insert(id.clone(), project);
        Ok(id)
    }

    /// Get a project by ID.
    pub fn get(&self, id: &ProjectId) -> Option<&Project> {
        self.projects.get(id)
    }

    /// Get all projects.
    pub fn all(&self) -> Vec<&Project> {
        self.projects.values().collect()
    }

    /// Get project summaries.
    pub fn summaries(&self) -> Vec<ProjectSummary> {
        self.projects
            .values()
            .map(|p| {
                let completed_milestones = p.milestones.iter().filter(|m| m.completed).count();
                let high_risk_count = p
                    .risks
                    .iter()
                    .filter(|r| r.level >= RiskLevel::High)
                    .count();

                let days_until_deadline = p.deadline.map(|d| {
                    let now = chrono::Utc::now();
                    (d - now).num_days()
                });

                ProjectSummary {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    status: p.status.clone(),
                    progress: p.progress,
                    milestone_count: p.milestones.len(),
                    completed_milestones,
                    risk_count: p.risks.len(),
                    high_risk_count,
                    days_until_deadline,
                }
            })
            .collect()
    }

    /// Add a milestone to a project.
    pub fn add_milestone(
        &mut self,
        project_id: &ProjectId,
        title: String,
        description: String,
        due_date: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<MilestoneId, ProjectError> {
        let project = self
            .projects
            .get_mut(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.0.clone()))?;

        let id = MilestoneId::new();
        project.milestones.push(ProjectMilestone {
            id: id.clone(),
            title,
            description,
            due_date,
            completed: false,
            completed_at: None,
            progress: 0.0,
        });
        project.updated_at = chrono::Utc::now();
        Ok(id)
    }

    /// Complete a milestone.
    pub fn complete_milestone(
        &mut self,
        project_id: &ProjectId,
        milestone_id: &MilestoneId,
    ) -> Result<(), ProjectError> {
        let project = self
            .projects
            .get_mut(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.0.clone()))?;

        let milestone = project
            .milestones
            .iter_mut()
            .find(|m| m.id == *milestone_id)
            .ok_or_else(|| ProjectError::MilestoneNotFound(milestone_id.0.clone()))?;

        milestone.completed = true;
        milestone.completed_at = Some(chrono::Utc::now());
        milestone.progress = 1.0;

        // Update project progress
        let total = project.milestones.len();
        let completed = project.milestones.iter().filter(|m| m.completed).count();
        project.progress = if total > 0 {
            completed as f32 / total as f32
        } else {
            0.0
        };

        project.updated_at = chrono::Utc::now();
        Ok(())
    }

    /// Add a risk to a project.
    pub fn add_risk(
        &mut self,
        project_id: &ProjectId,
        title: String,
        description: String,
        level: RiskLevel,
        mitigation: Option<String>,
    ) -> Result<String, ProjectError> {
        let project = self
            .projects
            .get_mut(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.0.clone()))?;

        let risk_id = Uuid::new_v4().to_string();
        project.risks.push(ProjectRisk {
            id: risk_id.clone(),
            title,
            description,
            level,
            mitigation,
            owner: None,
            created_at: chrono::Utc::now(),
        });
        project.updated_at = chrono::Utc::now();
        Ok(risk_id)
    }

    /// Add a repository to a project.
    pub fn add_repository(
        &mut self,
        project_id: &ProjectId,
        repo: Repository,
    ) -> Result<(), ProjectError> {
        let project = self
            .projects
            .get_mut(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.0.clone()))?;

        project.repositories.push(repo);
        project.updated_at = chrono::Utc::now();
        Ok(())
    }

    /// Update project status.
    pub fn set_status(
        &mut self,
        project_id: &ProjectId,
        status: ProjectStatus,
    ) -> Result<(), ProjectError> {
        let project = self
            .projects
            .get_mut(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.0.clone()))?;

        project.status = status;
        project.updated_at = chrono::Utc::now();
        Ok(())
    }

    /// Set project deadline.
    pub fn set_deadline(
        &mut self,
        project_id: &ProjectId,
        deadline: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), ProjectError> {
        let project = self
            .projects
            .get_mut(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.0.clone()))?;

        project.deadline = Some(deadline);
        project.updated_at = chrono::Utc::now();
        Ok(())
    }

    /// Remove a project.
    pub fn remove(&mut self, id: &ProjectId) -> Result<Project, ProjectError> {
        self.projects
            .remove(id)
            .ok_or_else(|| ProjectError::NotFound(id.0.clone()))
    }
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, Clone, thiserror::Error)]
pub enum ProjectError {
    #[error("Project not found: {0}")]
    NotFound(String),

    #[error("Milestone not found: {0}")]
    MilestoneNotFound(String),

    #[error("Capacity reached: {0} projects maximum")]
    CapacityReached(usize),
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manager_creation() {
        let _mgr = ProjectManager::default_manager();
    }

    #[test]
    fn create_project() {
        let mut mgr = ProjectManager::default_manager();
        let id = mgr
            .create_project("VOXY".to_string(), "AI OS".to_string())
            .unwrap();
        assert!(mgr.get(&id).is_some());
    }

    #[test]
    fn add_milestone() {
        let mut mgr = ProjectManager::default_manager();
        let id = mgr
            .create_project("Test".to_string(), "".to_string())
            .unwrap();
        let _ms_id = mgr
            .add_milestone(&id, "M1".to_string(), "First".to_string(), None)
            .unwrap();
        assert!(mgr.get(&id).unwrap().milestones.len() == 1);
    }

    #[test]
    fn complete_milestone() {
        let mut mgr = ProjectManager::default_manager();
        let id = mgr
            .create_project("Test".to_string(), "".to_string())
            .unwrap();
        let ms_id = mgr
            .add_milestone(&id, "M1".to_string(), "".to_string(), None)
            .unwrap();
        mgr.complete_milestone(&id, &ms_id).unwrap();
        assert_eq!(mgr.get(&id).unwrap().milestones[0].progress, 1.0);
        assert_eq!(mgr.get(&id).unwrap().progress, 1.0);
    }

    #[test]
    fn add_risk() {
        let mut mgr = ProjectManager::default_manager();
        let id = mgr
            .create_project("Test".to_string(), "".to_string())
            .unwrap();
        mgr.add_risk(
            &id,
            "Risk 1".to_string(),
            "Description".to_string(),
            RiskLevel::High,
            None,
        )
        .unwrap();
        assert_eq!(mgr.get(&id).unwrap().risks.len(), 1);
    }

    #[test]
    fn set_status() {
        let mut mgr = ProjectManager::default_manager();
        let id = mgr
            .create_project("Test".to_string(), "".to_string())
            .unwrap();
        mgr.set_status(&id, ProjectStatus::Active).unwrap();
        assert_eq!(mgr.get(&id).unwrap().status, ProjectStatus::Active);
    }

    #[test]
    fn summaries() {
        let mut mgr = ProjectManager::default_manager();
        mgr.create_project("P1".to_string(), "".to_string())
            .unwrap();
        mgr.create_project("P2".to_string(), "".to_string())
            .unwrap();
        let summaries = mgr.summaries();
        assert_eq!(summaries.len(), 2);
    }

    #[test]
    fn add_repository() {
        let mut mgr = ProjectManager::default_manager();
        let id = mgr
            .create_project("Test".to_string(), "".to_string())
            .unwrap();
        let repo = Repository {
            id: RepositoryId(Uuid::new_v4().to_string()),
            name: "voxy".to_string(),
            url: "https://github.com/test/voxy".to_string(),
            branch: "main".to_string(),
            last_commit: None,
            last_commit_message: None,
            last_commit_time: None,
            dirty: false,
            ahead: 0,
            behind: 0,
        };
        mgr.add_repository(&id, repo).unwrap();
        assert_eq!(mgr.get(&id).unwrap().repositories.len(), 1);
    }

    #[test]
    fn remove_project() {
        let mut mgr = ProjectManager::default_manager();
        let id = mgr
            .create_project("Test".to_string(), "".to_string())
            .unwrap();
        let removed = mgr.remove(&id).unwrap();
        assert_eq!(removed.name, "Test");
        assert!(mgr.get(&id).is_none());
    }

    #[test]
    fn not_found_error() {
        let mut mgr = ProjectManager::default_manager();
        let result = mgr.set_status(&ProjectId::new(), ProjectStatus::Active);
        assert!(result.is_err());
    }

    #[test]
    fn capacity_limit() {
        let mut mgr = ProjectManager::new(1);
        mgr.create_project("P1".to_string(), "".to_string())
            .unwrap();
        let result = mgr.create_project("P2".to_string(), "".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn set_deadline() {
        let mut mgr = ProjectManager::default_manager();
        let id = mgr
            .create_project("Test".to_string(), "".to_string())
            .unwrap();
        let deadline = chrono::Utc::now() + chrono::Duration::days(30);
        mgr.set_deadline(&id, deadline).unwrap();
        assert!(mgr.get(&id).unwrap().deadline.is_some());
    }
}
