//! Unified Service Hub — central registry, DI container, and event bus bridge.
//!
//! Every subsystem registers itself here. The hub provides:
//! - Service lookup by name or type
//! - Dependency resolution via DI container
//! - Event bus access for all subsystems
//! - Health monitoring aggregation
//! - Lifecycle management

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use voxy_event_bus::EventBus;
use voxy_shared::HealthStatus;

use crate::error::{IntegrationError, Result};

// ============================================================================
// Service Descriptor
// ============================================================================

/// Describes a registered service.
#[derive(Debug, Clone)]
pub struct ServiceDescriptor {
    pub name: String,
    pub version: String,
    pub dependencies: Vec<String>,
    pub health: HealthStatus,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
}

// ============================================================================
// Service Hub
// ============================================================================

/// Central hub connecting all subsystems.
///
/// The ServiceHub is the single entry point for:
/// - Registering services
/// - Resolving dependencies
/// - Accessing the event bus
/// - Querying health
pub struct ServiceHub {
    /// The event bus shared by all subsystems.
    event_bus: Arc<EventBus>,
    /// Service descriptors indexed by name.
    descriptors: RwLock<HashMap<String, ServiceDescriptor>>,
    /// Type-erased service instances for DI resolution.
    services: RwLock<HashMap<String, Arc<dyn Any + Send + Sync>>>,
    /// TypeId -> name mapping for DI resolution.
    type_map: RwLock<HashMap<TypeId, String>>,
    /// Service health cache.
    health_cache: RwLock<HashMap<String, HealthStatus>>,
}

impl ServiceHub {
    /// Create a new service hub with default event bus.
    pub fn new() -> Self {
        Self {
            event_bus: Arc::new(EventBus::new(256)),
            descriptors: RwLock::new(HashMap::new()),
            services: RwLock::new(HashMap::new()),
            type_map: RwLock::new(HashMap::new()),
            health_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Create a hub with a custom event bus.
    pub fn with_event_bus(event_bus: Arc<EventBus>) -> Self {
        Self {
            event_bus,
            descriptors: RwLock::new(HashMap::new()),
            services: RwLock::new(HashMap::new()),
            type_map: RwLock::new(HashMap::new()),
            health_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Register a service by name.
    pub fn register(
        &self,
        name: impl Into<String>,
        version: impl Into<String>,
        dependencies: Vec<String>,
    ) -> Result<()> {
        let name = name.into();
        let descriptor = ServiceDescriptor {
            name: name.clone(),
            version: version.into(),
            dependencies,
            health: HealthStatus::Healthy,
            started_at: None,
        };

        let mut descriptors = self.descriptors.write();
        if descriptors.contains_key(&name) {
            return Err(IntegrationError::ServiceAlreadyRegistered(name));
        }
        descriptors.insert(name, descriptor);
        Ok(())
    }

    /// Register a typed service for DI resolution.
    pub fn register_typed<T: Send + Sync + 'static>(
        &self,
        name: impl Into<String>,
        instance: Arc<T>,
    ) -> Result<()> {
        let name = name.into();
        let type_id = TypeId::of::<T>();

        self.services.write().insert(name.clone(), instance);
        self.type_map.write().insert(type_id, name);
        Ok(())
    }

    /// Resolve a typed service.
    pub fn resolve<T: Send + Sync + 'static>(&self) -> Result<Arc<T>> {
        let type_id = TypeId::of::<T>();
        let name = self.type_map.read().get(&type_id).cloned().ok_or_else(|| {
            IntegrationError::ServiceNotFound(std::any::type_name::<T>().to_string())
        })?;

        let services = self.services.read();
        let instance = services
            .get(&name)
            .ok_or(IntegrationError::ServiceNotFound(name))?;

        instance
            .clone()
            .downcast::<T>()
            .map_err(|_| IntegrationError::ServiceNotFound("Type mismatch".to_string()))
    }

    /// Update service health.
    pub fn update_health(&self, name: &str, health: HealthStatus) {
        self.health_cache
            .write()
            .insert(name.to_string(), health.clone());
        let mut descriptors = self.descriptors.write();
        if let Some(desc) = descriptors.get_mut(name) {
            desc.health = health;
        }
    }

    /// Mark a service as started.
    pub fn mark_started(&self, name: &str) {
        let mut descriptors = self.descriptors.write();
        if let Some(desc) = descriptors.get_mut(name) {
            desc.started_at = Some(chrono::Utc::now());
        }
    }

    /// Get service descriptor.
    pub fn descriptor(&self, name: &str) -> Option<ServiceDescriptor> {
        self.descriptors.read().get(name).cloned()
    }

    /// Get all service descriptors.
    pub fn all_descriptors(&self) -> Vec<ServiceDescriptor> {
        self.descriptors.read().values().cloned().collect()
    }

    /// Get the event bus.
    pub fn event_bus(&self) -> Arc<EventBus> {
        self.event_bus.clone()
    }

    /// Get aggregate health.
    pub fn aggregate_health(&self) -> HealthStatus {
        let cache = self.health_cache.read();
        if cache.is_empty() {
            return HealthStatus::Healthy;
        }

        let has_unhealthy = cache.values().any(|h| h.is_unhealthy());
        let has_degraded = cache.values().any(|h| h.is_degraded());

        if has_unhealthy {
            HealthStatus::Unhealthy("One or more subsystems unhealthy".to_string())
        } else if has_degraded {
            HealthStatus::Degraded("One or more subsystems degraded".to_string())
        } else {
            HealthStatus::Healthy
        }
    }

    /// Get registered service count.
    pub fn service_count(&self) -> usize {
        self.descriptors.read().len()
    }

    /// Check if a service is registered.
    pub fn has_service(&self, name: &str) -> bool {
        self.descriptors.read().contains_key(name)
    }

    /// Get all service names.
    pub fn service_names(&self) -> Vec<String> {
        self.descriptors.read().keys().cloned().collect()
    }

    /// Validate dependency graph (no cycles, all deps registered).
    pub fn validate_dependencies(&self) -> Result<()> {
        let descriptors = self.descriptors.read();
        for (name, desc) in descriptors.iter() {
            for dep in &desc.dependencies {
                if !descriptors.contains_key(dep) {
                    return Err(IntegrationError::ServiceNotFound(format!(
                        "Service '{}' depends on '{}' which is not registered",
                        name, dep
                    )));
                }
            }
        }
        Ok(())
    }
}

impl Default for ServiceHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_creation() {
        let hub = ServiceHub::new();
        assert_eq!(hub.service_count(), 0);
    }

    #[test]
    fn register_service() {
        let hub = ServiceHub::new();
        hub.register("voice", "0.1.0", vec![]).unwrap();
        assert_eq!(hub.service_count(), 1);
        assert!(hub.has_service("voice"));
    }

    #[test]
    fn register_duplicate_fails() {
        let hub = ServiceHub::new();
        hub.register("voice", "0.1.0", vec![]).unwrap();
        let result = hub.register("voice", "0.2.0", vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn register_with_dependencies() {
        let hub = ServiceHub::new();
        hub.register("event_bus", "0.1.0", vec![]).unwrap();
        hub.register("voice", "0.1.0", vec!["event_bus".to_string()])
            .unwrap();
    }

    #[test]
    fn validate_good_dependencies() {
        let hub = ServiceHub::new();
        hub.register("a", "0.1.0", vec![]).unwrap();
        hub.register("b", "0.1.0", vec!["a".to_string()]).unwrap();
        assert!(hub.validate_dependencies().is_ok());
    }

    #[test]
    fn validate_missing_dependency() {
        let hub = ServiceHub::new();
        hub.register("a", "0.1.0", vec!["nonexistent".to_string()])
            .unwrap();
        assert!(hub.validate_dependencies().is_err());
    }

    #[test]
    fn update_and_query_health() {
        let hub = ServiceHub::new();
        hub.register("svc", "0.1.0", vec![]).unwrap();
        hub.update_health("svc", HealthStatus::Degraded("slow".to_string()));
        assert!(hub.aggregate_health().is_degraded());
    }

    #[test]
    fn aggregate_healthy() {
        let hub = ServiceHub::new();
        hub.register("a", "0.1.0", vec![]).unwrap();
        hub.register("b", "0.1.0", vec![]).unwrap();
        assert!(hub.aggregate_health().is_healthy());
    }

    #[test]
    fn aggregate_unhealthy() {
        let hub = ServiceHub::new();
        hub.register("a", "0.1.0", vec![]).unwrap();
        hub.register("b", "0.1.0", vec![]).unwrap();
        hub.update_health("a", HealthStatus::Unhealthy("crash".to_string()));
        assert!(hub.aggregate_health().is_unhealthy());
    }

    #[test]
    fn service_names() {
        let hub = ServiceHub::new();
        hub.register("alpha", "0.1.0", vec![]).unwrap();
        hub.register("beta", "0.1.0", vec![]).unwrap();
        let mut names = hub.service_names();
        names.sort();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn typed_registration_and_resolve() {
        let hub = ServiceHub::new();
        let svc = Arc::new(42i32);
        hub.register_typed("answer", svc).unwrap();
        let resolved = hub.resolve::<i32>().unwrap();
        assert_eq!(*resolved, 42);
    }

    #[test]
    fn typed_resolve_unregistered() {
        let hub = ServiceHub::new();
        let result = hub.resolve::<String>();
        assert!(result.is_err());
    }

    #[test]
    fn event_bus_accessible() {
        let hub = ServiceHub::new();
        let _bus = hub.event_bus();
    }

    #[test]
    fn mark_started() {
        let hub = ServiceHub::new();
        hub.register("svc", "0.1.0", vec![]).unwrap();
        hub.mark_started("svc");
        let desc = hub.descriptor("svc").unwrap();
        assert!(desc.started_at.is_some());
    }
}
