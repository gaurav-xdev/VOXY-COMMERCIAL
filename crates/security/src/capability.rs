//! Capability types and registry.

use std::collections::HashMap;

/// Risk level for capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiskLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// Capability category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityCategory {
    Audio,
    Automation,
    File,
    Memory,
    Network,
    Process,
    Registry,
    Screen,
    System,
}

/// A security capability.
#[derive(Debug, Clone)]
pub struct Capability {
    pub id: String,
    pub name: String,
    pub description: String,
    pub risk_level: RiskLevel,
    pub category: CapabilityCategory,
}

impl Capability {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        risk_level: RiskLevel,
        category: CapabilityCategory,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            risk_level,
            category,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Check if this capability matches a request.
    pub fn matches(&self, requested: &str) -> bool {
        self.id == requested
            || (self.id.ends_with(":*") && requested.starts_with(&self.id[..self.id.len() - 1]))
    }
}

/// Registry of available capabilities.
pub struct CapabilityRegistry {
    capabilities: HashMap<String, Capability>,
}

impl CapabilityRegistry {
    /// Create a new registry with default capabilities.
    pub fn new() -> Self {
        let mut registry = Self {
            capabilities: HashMap::new(),
        };
        registry.register_defaults();
        registry
    }

    /// Register a capability.
    pub fn register(&mut self, capability: Capability) -> crate::Result<()> {
        if self.capabilities.contains_key(&capability.id) {
            return Err(crate::SecurityError::CapabilityAlreadyRegistered(
                capability.id,
            ));
        }
        self.capabilities.insert(capability.id.clone(), capability);
        Ok(())
    }

    /// Get a capability by ID.
    pub fn get(&self, id: &str) -> Option<&Capability> {
        self.capabilities.get(id)
    }

    /// Check if a capability exists.
    pub fn has(&self, id: &str) -> bool {
        self.capabilities.contains_key(id)
    }

    /// List all capabilities.
    pub fn list_all(&self) -> Vec<&Capability> {
        self.capabilities.values().collect()
    }

    /// Find capabilities matching a request.
    pub fn find_matching(&self, requested: &str) -> Vec<&Capability> {
        self.capabilities
            .values()
            .filter(|c| c.matches(requested))
            .collect()
    }

    fn register_defaults(&mut self) {
        // ── NONE (no permission needed) ────────────────────────────
        let none_caps = vec![
            (
                "system:info",
                "System Info",
                CapabilityCategory::System,
                "Read system information",
            ),
            (
                "system:time",
                "System Time",
                CapabilityCategory::System,
                "Read system time",
            ),
            (
                "system:version",
                "VOXY Version",
                CapabilityCategory::System,
                "Read VOXY version",
            ),
            (
                "app:basic",
                "Basic App",
                CapabilityCategory::System,
                "Basic application functionality",
            ),
            (
                "ui:focus",
                "UI Focus",
                CapabilityCategory::System,
                "Get focused window info",
            ),
        ];
        for (id, name, cat, desc) in none_caps {
            let _ = self
                .register(Capability::new(id, name, RiskLevel::None, cat).with_description(desc));
        }

        // ── LOW (auto-granted for trusted, notification required) ───
        let low_caps = vec![
            (
                "clipboard:read",
                "Clipboard Read",
                CapabilityCategory::System,
                "Read clipboard contents",
            ),
            (
                "clipboard:write",
                "Clipboard Write",
                CapabilityCategory::System,
                "Write to clipboard",
            ),
            (
                "file:read",
                "File Read",
                CapabilityCategory::File,
                "Read files in allowed paths",
            ),
            (
                "notification:read",
                "Notification Read",
                CapabilityCategory::System,
                "Read notifications",
            ),
            (
                "network:status",
                "Network Status",
                CapabilityCategory::Network,
                "Check network status",
            ),
            (
                "ui:list_windows",
                "List Windows",
                CapabilityCategory::System,
                "List open windows",
            ),
            (
                "memory:self_read",
                "Self Read",
                CapabilityCategory::Memory,
                "Read own process memory",
            ),
            (
                "automation:read",
                "Automation Read",
                CapabilityCategory::Automation,
                "Read UI state",
            ),
            (
                "memory:read",
                "Memory Read",
                CapabilityCategory::Memory,
                "Read memory",
            ),
        ];
        for (id, name, cat, desc) in low_caps {
            let _ = self
                .register(Capability::new(id, name, RiskLevel::Low, cat).with_description(desc));
        }

        // ── MEDIUM (requires user consent) ─────────────────────────
        let medium_caps = vec![
            (
                "app:launch",
                "Launch App",
                CapabilityCategory::System,
                "Launch applications",
            ),
            (
                "file:write",
                "File Write",
                CapabilityCategory::File,
                "Modify files",
            ),
            (
                "file:download",
                "File Download",
                CapabilityCategory::Network,
                "Download files to disk",
            ),
            (
                "browser:automation",
                "Browser Automation",
                CapabilityCategory::Automation,
                "Browser control",
            ),
            (
                "network:http",
                "HTTP Request",
                CapabilityCategory::Network,
                "Outbound HTTP requests",
            ),
            (
                "audio:loopback",
                "Audio Loopback",
                CapabilityCategory::Audio,
                "Capture system audio",
            ),
            (
                "screen:metadata",
                "Screen Metadata",
                CapabilityCategory::Screen,
                "Get screen resolution/layout",
            ),
            (
                "model:inference",
                "Model Inference",
                CapabilityCategory::System,
                "Run model inference",
            ),
            (
                "memory:write",
                "Memory Write",
                CapabilityCategory::Memory,
                "Write memory",
            ),
            (
                "network:read",
                "Network Read",
                CapabilityCategory::Network,
                "HTTP GET requests",
            ),
        ];
        for (id, name, cat, desc) in medium_caps {
            let _ = self
                .register(Capability::new(id, name, RiskLevel::Medium, cat).with_description(desc));
        }

        // ── HIGH (requires explicit consent + Guardian check) ──────
        let high_caps = vec![
            (
                "file:delete",
                "Delete Files",
                CapabilityCategory::File,
                "Delete files",
            ),
            (
                "file:permissions",
                "File Permissions",
                CapabilityCategory::File,
                "Change file permissions",
            ),
            (
                "registry:read",
                "Registry Read",
                CapabilityCategory::System,
                "Read registry",
            ),
            (
                "registry:write",
                "Registry Write",
                CapabilityCategory::System,
                "Modify registry",
            ),
            (
                "system:settings",
                "System Settings",
                CapabilityCategory::System,
                "Change system settings",
            ),
            (
                "network:config_read",
                "Network Config Read",
                CapabilityCategory::Network,
                "Read network configuration",
            ),
            (
                "network:config_write",
                "Network Config Write",
                CapabilityCategory::Network,
                "Change network configuration",
            ),
            (
                "audio:capture",
                "Audio Capture",
                CapabilityCategory::Audio,
                "Microphone access",
            ),
            (
                "screen:capture",
                "Screen Capture",
                CapabilityCategory::Screen,
                "Screenshot/recording",
            ),
            (
                "automation:input",
                "Automation Input",
                CapabilityCategory::Automation,
                "Mouse/keyboard simulation",
            ),
            (
                "automation:write",
                "Automation Write",
                CapabilityCategory::Automation,
                "Interact with UI",
            ),
            (
                "process:list",
                "Process List",
                CapabilityCategory::System,
                "List running processes",
            ),
            (
                "process:info",
                "Process Info",
                CapabilityCategory::System,
                "Get process details",
            ),
            (
                "network:write",
                "Network Write",
                CapabilityCategory::Network,
                "HTTP POST/PUT requests",
            ),
        ];
        for (id, name, cat, desc) in high_caps {
            let _ = self
                .register(Capability::new(id, name, RiskLevel::High, cat).with_description(desc));
        }

        // ── CRITICAL (requires MFA + Guardian + Threat Analysis) ───
        let critical_caps = vec![
            (
                "disk:format",
                "Format Disk",
                CapabilityCategory::System,
                "Format drives",
            ),
            (
                "disk:partition",
                "Partition Disk",
                CapabilityCategory::System,
                "Modify partitions",
            ),
            (
                "disk:bitlocker",
                "BitLocker",
                CapabilityCategory::System,
                "BitLocker operations",
            ),
            (
                "service:manage",
                "Manage Services",
                CapabilityCategory::System,
                "Start/stop Windows services",
            ),
            (
                "security:policy",
                "Security Policy",
                CapabilityCategory::System,
                "Modify security policies",
            ),
            (
                "security:firewall",
                "Firewall",
                CapabilityCategory::System,
                "Change firewall rules",
            ),
            (
                "process:spawn",
                "Spawn Process",
                CapabilityCategory::System,
                "Launch processes",
            ),
            (
                "process:inject",
                "Inject Process",
                CapabilityCategory::System,
                "Code injection",
            ),
            (
                "process:kill",
                "Kill Process",
                CapabilityCategory::System,
                "Kill processes",
            ),
            (
                "plugin:load",
                "Load Plugin",
                CapabilityCategory::System,
                "Load plugins",
            ),
            (
                "plugin:unload",
                "Unload Plugin",
                CapabilityCategory::System,
                "Unload plugins",
            ),
            (
                "agent:spawn",
                "Spawn Agent",
                CapabilityCategory::System,
                "Create agents",
            ),
            (
                "agent:control",
                "Control Agent",
                CapabilityCategory::System,
                "Control other agents",
            ),
            (
                "home:security",
                "Home Security",
                CapabilityCategory::System,
                "Home security system",
            ),
            (
                "home:door_lock",
                "Door Lock",
                CapabilityCategory::System,
                "Electronic door locks",
            ),
            (
                "home:emergency",
                "Emergency",
                CapabilityCategory::System,
                "Emergency actions (alarm, fire)",
            ),
            (
                "home:camera",
                "Security Camera",
                CapabilityCategory::System,
                "Security camera access",
            ),
            (
                "network:listen",
                "Network Listen",
                CapabilityCategory::Network,
                "Inbound network connections",
            ),
            (
                "identity:impersonate",
                "Impersonate",
                CapabilityCategory::System,
                "Act as another entity",
            ),
        ];
        for (id, name, cat, desc) in critical_caps {
            let _ = self.register(
                Capability::new(id, name, RiskLevel::Critical, cat).with_description(desc),
            );
        }
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_creation() {
        let cap = Capability::new(
            "test:cap",
            "Test",
            RiskLevel::Low,
            CapabilityCategory::System,
        );
        assert_eq!(cap.id, "test:cap");
        assert_eq!(cap.name, "Test");
        assert_eq!(cap.risk_level, RiskLevel::Low);
    }

    #[test]
    fn capability_matching() {
        let cap = Capability::new(
            "audio:*",
            "Audio",
            RiskLevel::Low,
            CapabilityCategory::Audio,
        );
        assert!(cap.matches("audio:capture"));
        assert!(cap.matches("audio:playback"));
        assert!(!cap.matches("video:capture"));
    }

    #[test]
    fn default_registry_has_capabilities() {
        let registry = CapabilityRegistry::new();
        assert!(registry.has("audio:capture"));
        assert!(registry.has("file:read"));
        assert!(registry.has("system:info"));
    }

    #[test]
    fn registry_operations() {
        let mut registry = CapabilityRegistry::new();
        let cap = Capability::new(
            "test:cap",
            "Test",
            RiskLevel::Low,
            CapabilityCategory::System,
        );

        assert!(registry.register(cap).is_ok());
        assert!(registry.has("test:cap"));

        let found = registry.get("test:cap").unwrap();
        assert_eq!(found.name, "Test");

        let matching = registry.find_matching("test:cap");
        assert_eq!(matching.len(), 1);
    }
}
