use async_trait::async_trait;

use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulingOrder {
    Topological,
    Sequential,
    Parallel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyState {
    Pending,
    Resolved,
    Running,
    Failed(String),
    Completed,
}

#[derive(Debug, Clone)]
pub struct DependencySpec {
    pub name: String,
    pub depends_on: Vec<String>,
    pub provides: Vec<String>,
    pub required: bool,
    pub order: SchedulingOrder,
    pub timeout_seconds: u64,
    pub auto_restart: bool,
}

#[derive(Debug, Clone)]
pub struct ResolutionResult {
    pub order: Vec<String>,
    pub resolved: Vec<String>,
    pub failed: Vec<String>,
    pub cycles: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct DependencyNode {
    pub name: String,
    pub depth: usize,
    pub dependencies: Vec<String>,
}

#[async_trait]
pub trait DependencyResolver: Send + Sync {
    async fn register(&self, spec: DependencySpec) -> Result<()>;
    async fn resolve(&self) -> Result<ResolutionResult>;
    async fn depends_on(&self, name: &str) -> Result<Vec<String>>;
    async fn dependents(&self, name: &str) -> Result<Vec<String>>;
    async fn state(&self, name: &str) -> Option<DependencyState>;
    async fn set_state(&self, name: &str, state: DependencyState) -> Result<()>;
    async fn all_specs(&self) -> Result<Vec<DependencySpec>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduling_order_variants() {
        assert_eq!(SchedulingOrder::Topological, SchedulingOrder::Topological);
        assert_eq!(SchedulingOrder::Sequential, SchedulingOrder::Sequential);
        assert_eq!(SchedulingOrder::Parallel, SchedulingOrder::Parallel);
    }

    #[test]
    fn test_dependency_state_transitions() {
        let states = vec![
            DependencyState::Pending,
            DependencyState::Resolved,
            DependencyState::Running,
            DependencyState::Completed,
        ];
        for state in &states {
            match state {
                DependencyState::Pending
                | DependencyState::Resolved
                | DependencyState::Running
                | DependencyState::Completed => {}
                DependencyState::Failed(_) => unreachable!(),
            }
        }
    }

    #[test]
    fn test_dependency_spec_creation() {
        let spec = DependencySpec {
            name: "service-a".into(),
            depends_on: vec!["service-b".into()],
            provides: vec!["api".into()],
            required: true,
            order: SchedulingOrder::Topological,
            timeout_seconds: 30,
            auto_restart: false,
        };
        assert_eq!(spec.name, "service-a");
        assert!(spec.required);
        assert!(!spec.auto_restart);
        assert_eq!(spec.depends_on, vec!["service-b"]);
    }

    #[test]
    fn test_resolution_result_creation() {
        let result = ResolutionResult {
            order: vec!["a".into(), "b".into()],
            resolved: vec!["a".into(), "b".into()],
            failed: vec![],
            cycles: vec![],
        };
        assert_eq!(result.order.len(), 2);
        assert!(result.failed.is_empty());
        assert!(result.cycles.is_empty());
    }

    #[test]
    fn test_dependency_node_creation() {
        let node = DependencyNode {
            name: "node".into(),
            depth: 0,
            dependencies: vec!["dep".into()],
        };
        assert_eq!(node.depth, 0);
        assert_eq!(node.dependencies.len(), 1);
    }
}
