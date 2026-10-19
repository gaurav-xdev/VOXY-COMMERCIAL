//! IPC Auth Middleware — gates every request with token validation.
//!
//! SECURITY: All IPC method calls must go through this middleware.
//! Requests without a valid (signed, non-expired) token are rejected.

use std::collections::HashMap;

use parking_lot::RwLock;

use crate::auth::{AuthMethod, AuthRequest, AuthResponse, CapabilityClaim, CapabilityToken};

/// Errors returned by the auth middleware.
#[derive(Debug, Clone)]
pub enum AuthError {
    NoToken,
    TokenExpired,
    TokenUnsigned,
    InsufficientCapabilities(String),
    AuthenticationFailed(String),
    RateLimited,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoToken => write!(f, "No authentication token provided"),
            Self::TokenExpired => write!(f, "Authentication token has expired"),
            Self::TokenUnsigned => write!(f, "Authentication token is not signed"),
            Self::InsufficientCapabilities(cap) => {
                write!(f, "Insufficient capabilities: requires '{}'", cap)
            }
            Self::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            Self::RateLimited => write!(f, "Rate limited — too many requests"),
        }
    }
}

impl std::error::Error for AuthError {}

/// Rate limiter per subject.
struct RateLimiter {
    max_requests: usize,
    window_ms: u64,
    subjects: HashMap<String, Vec<u64>>,
}

impl RateLimiter {
    fn new(max_requests: usize, window_ms: u64) -> Self {
        Self {
            max_requests,
            window_ms,
            subjects: HashMap::new(),
        }
    }

    fn check_and_record(&mut self, subject: &str, now_ms: u64) -> bool {
        // Prune expired entries to prevent unbounded memory growth
        self.subjects.retain(|_, timestamps| {
            timestamps.retain(|&t| now_ms.saturating_sub(t) <= self.window_ms);
            !timestamps.is_empty()
        });

        let timestamps = self.subjects.entry(subject.to_string()).or_default();
        timestamps.retain(|&t| now_ms.saturating_sub(t) <= self.window_ms);
        if timestamps.len() >= self.max_requests {
            return false;
        }
        timestamps.push(now_ms);
        true
    }
}

/// Auth middleware that validates tokens on every IPC request.
pub struct AuthMiddleware {
    /// Active sessions keyed by session ID.
    sessions: RwLock<HashMap<uuid::Uuid, SessionInfo>>,
    /// Rate limiter.
    rate_limiter: RwLock<RateLimiter>,
    /// Required capabilities per method name.
    method_capabilities: HashMap<String, String>,
    /// Optional set of trusted token SHA-256 hashes for authentication.
    /// Default is None (unconfigured). Authentication will fail closed if None or empty.
    trusted_token_hashes: RwLock<Option<std::collections::HashSet<Vec<u8>>>>,
    /// Allowed capabilities policy: either None (in which case only capabilities registered in method_capabilities or system policy are authorized),
    /// or Some(set) specifying exact allowed capabilities for issued tokens.
    allowed_capabilities: RwLock<Option<std::collections::HashSet<String>>>,
    /// Optional HMAC-SHA256 signing secret for authentic token signature validation.
    signing_secret: RwLock<Option<Vec<u8>>>,
}

struct SessionInfo {
    token: CapabilityToken,
    #[allow(dead_code)]
    created_at: u64,
}

impl AuthMiddleware {
    pub fn new(max_requests_per_minute: usize) -> Self {
        let mut method_capabilities = HashMap::new();
        // Map IPC methods to required capabilities
        method_capabilities.insert("audio:capture".to_string(), "audio:capture".to_string());
        method_capabilities.insert("audio:playback".to_string(), "audio:capture".to_string());
        method_capabilities.insert("screen:capture".to_string(), "screen:capture".to_string());
        method_capabilities.insert(
            "automation:execute".to_string(),
            "automation:write".to_string(),
        );
        method_capabilities.insert("file:read".to_string(), "file:read".to_string());
        method_capabilities.insert("file:write".to_string(), "file:write".to_string());
        method_capabilities.insert("memory:read".to_string(), "memory:read".to_string());
        method_capabilities.insert("memory:write".to_string(), "memory:write".to_string());
        method_capabilities.insert("network:request".to_string(), "network:write".to_string());

        Self {
            sessions: RwLock::new(HashMap::new()),
            rate_limiter: RwLock::new(RateLimiter::new(
                max_requests_per_minute,
                60_000, // 1 minute window
            )),
            method_capabilities,
            trusted_token_hashes: RwLock::new(None),
            allowed_capabilities: RwLock::new(None),
            signing_secret: RwLock::new(None),
        }
    }

    /// Set an HMAC-SHA256 secret key for signing session tokens and verifying request tokens.
    pub fn set_signing_secret(&self, secret: &[u8]) {
        *self.signing_secret.write() = Some(secret.to_vec());
    }

    /// Fluent builder method to set an HMAC-SHA256 secret key.
    pub fn with_signing_secret(self, secret: &[u8]) -> Self {
        *self.signing_secret.write() = Some(secret.to_vec());
        self
    }

    /// Add a trusted token whose SHA-256 hash will be verified during authenticate().
    pub fn add_trusted_token(&self, token: &str) {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        let hash = hasher.finalize().to_vec();
        let mut guard = self.trusted_token_hashes.write();
        let set = guard.get_or_insert_with(std::collections::HashSet::new);
        set.insert(hash);
    }

    /// Fluent builder method to add a trusted token.
    pub fn with_trusted_token(self, token: &str) -> Self {
        self.add_trusted_token(token);
        self
    }

    /// Fluent builder method to configure explicit allowed capabilities.
    pub fn with_allowed_capabilities(
        self,
        capabilities: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut guard = self.allowed_capabilities.write();
        let set = guard.get_or_insert_with(std::collections::HashSet::new);
        for cap in capabilities {
            set.insert(cap.into());
        }
        drop(guard);
        self
    }

    /// Add an allowed capability to the authorization policy.
    pub fn add_allowed_capability(&self, capability: &str) {
        let mut guard = self.allowed_capabilities.write();
        let set = guard.get_or_insert_with(std::collections::HashSet::new);
        set.insert(capability.to_string());
    }

    /// Check if a capability is authorized by server policy.
    /// If an explicit `allowed_capabilities` set is configured, only those capabilities are permitted.
    /// Otherwise, only registered system capabilities (`method_capabilities` values) are permitted.
    pub fn is_capability_allowed(&self, capability: &str) -> bool {
        let guard = self.allowed_capabilities.read();
        if let Some(ref allowed) = *guard {
            allowed.contains(capability)
        } else {
            self.method_capabilities.values().any(|c| c == capability)
        }
    }

    /// Authenticate a new session. Returns a session token on success.
    pub fn authenticate(&self, request: &AuthRequest) -> Result<AuthResponse, AuthError> {
        match request.method {
            AuthMethod::Token | AuthMethod::ApiKey => {
                // Extract the token from credentials
                let token_str = request
                    .credentials
                    .get("token")
                    .or_else(|| request.credentials.get("key"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AuthError::AuthenticationFailed("Missing token in credentials".into())
                    })?;

                if token_str.is_empty() {
                    return Err(AuthError::AuthenticationFailed("Empty token".into()));
                }

                // Cryptographic validation against configured trusted tokens (fail closed)
                {
                    let guard = self.trusted_token_hashes.read();
                    match guard.as_ref() {
                        None => {
                            return Err(AuthError::AuthenticationFailed(
                                "Trust store not configured; failing closed".into(),
                            ));
                        }
                        Some(trusted) if trusted.is_empty() => {
                            return Err(AuthError::AuthenticationFailed(
                                "Trust store is empty; failing closed".into(),
                            ));
                        }
                        Some(trusted) => {
                            use sha2::{Digest, Sha256};
                            let mut hasher = Sha256::new();
                            hasher.update(token_str.as_bytes());
                            let hash = hasher.finalize().to_vec();
                            if !trusted.contains(&hash) {
                                return Err(AuthError::AuthenticationFailed(
                                    "Invalid token: credential not found in trust store".into(),
                                ));
                            }
                        }
                    }
                }

                // Capability authorization policy check: ensure caller cannot grant arbitrary capabilities
                for cap in &request.requested_capabilities {
                    if !self.is_capability_allowed(cap) {
                        return Err(AuthError::AuthenticationFailed(format!(
                            "Unauthorized capability requested: '{}'",
                            cap
                        )));
                    }
                }

                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                let granted_capabilities: Vec<CapabilityClaim> = request
                    .requested_capabilities
                    .iter()
                    .map(|cap| CapabilityClaim {
                        capability: cap.clone(),
                        resource: None,
                        constraints: vec![],
                    })
                    .collect();

                let mut token = CapabilityToken::new(
                    &request.client_name,
                    "voxy-auth",
                    granted_capabilities,
                    3600,
                    token_str.as_bytes().to_vec(),
                );

                let secret_guard = self.signing_secret.read();
                let secret = secret_guard.as_ref().ok_or_else(|| {
                    AuthError::AuthenticationFailed(
                        "Signing secret not configured; failing closed".into(),
                    )
                })?;
                if secret.is_empty() {
                    return Err(AuthError::AuthenticationFailed(
                        "Signing secret is empty; failing closed".into(),
                    ));
                }
                token.sign(secret);

                let session_id = uuid::Uuid::new_v4();
                self.sessions.write().insert(
                    session_id,
                    SessionInfo {
                        token: token.clone(),
                        created_at: now_ms,
                    },
                );

                Ok(AuthResponse {
                    success: true,
                    token: Some(token),
                    session_id: Some(session_id),
                    error: None,
                    granted_capabilities: request.requested_capabilities.clone(),
                })
            }
            _ => Err(AuthError::AuthenticationFailed(format!(
                "Unsupported auth method: {:?}",
                request.method
            ))),
        }
    }

    /// Validate a request against a session token.
    /// This is the main entry point for gating IPC requests.
    pub fn validate_request(&self, session_id: &uuid::Uuid, method: &str) -> Result<(), AuthError> {
        // Rate limit check
        {
            let mut limiter = self.rate_limiter.write();
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            if !limiter.check_and_record(&session_id.to_string(), now_ms) {
                return Err(AuthError::RateLimited);
            }
        }

        // Session validation
        let token = {
            let sessions = self.sessions.read();
            sessions
                .get(session_id)
                .map(|s| s.token.clone())
                .ok_or(AuthError::NoToken)?
        };

        // Token validity
        if token.is_expired() {
            return Err(AuthError::TokenExpired);
        }
        if token.signature.is_empty() {
            return Err(AuthError::TokenUnsigned);
        }

        // Cryptographic HMAC-SHA256 signature verification: fails closed if missing or empty
        let secret_guard = self.signing_secret.read();
        let secret = secret_guard.as_ref().ok_or_else(|| {
            AuthError::AuthenticationFailed(
                "Signing secret not configured; failing closed".into(),
            )
        })?;
        if secret.is_empty() {
            return Err(AuthError::AuthenticationFailed(
                "Signing secret is empty; failing closed".into(),
            ));
        }

        if !token.verify_signature(secret) {
            return Err(AuthError::AuthenticationFailed(
                "Token signature verification failed: invalid HMAC".into(),
            ));
        }

        // Capability check
        if let Some(required_cap) = self.method_capabilities.get(method) {
            if !token.has_capability(required_cap) {
                return Err(AuthError::InsufficientCapabilities(required_cap.clone()));
            }
        }

        Ok(())
    }

    /// Revoke a session.
    pub fn revoke_session(&self, session_id: &uuid::Uuid) {
        self.sessions.write().remove(session_id);
    }

    /// Clean up expired sessions.
    pub fn cleanup_expired(&self) {
        let mut sessions = self.sessions.write();
        sessions.retain(|_, session| !session.token.is_expired());
    }

    /// Get the number of active sessions.
    pub fn active_sessions(&self) -> usize {
        self.sessions.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_middleware() -> AuthMiddleware {
        AuthMiddleware::new(100)
            .with_signing_secret(b"default-test-signing-secret")
            .with_trusted_token("valid-token-123")
            .with_trusted_token("token")
    }

    fn make_auth_request(token: &str, capabilities: Vec<String>) -> AuthRequest {
        AuthRequest {
            method: AuthMethod::Token,
            credentials: serde_json::json!({"token": token}),
            requested_capabilities: capabilities,
            client_version: "1.0.0".to_string(),
            client_name: "test-client".to_string(),
        }
    }

    #[test]
    fn authenticate_and_validate() {
        let mw = make_middleware();
        let req = make_auth_request("valid-token-123", vec!["audio:capture".to_string()]);
        let resp = mw.authenticate(&req).unwrap();
        assert!(resp.success);
        let session_id = resp.session_id.unwrap();

        assert!(mw.validate_request(&session_id, "audio:capture").is_ok());
    }

    #[test]
    fn reject_no_token() {
        let mw = make_middleware();
        let fake_id = uuid::Uuid::new_v4();
        assert!(matches!(
            mw.validate_request(&fake_id, "audio:capture"),
            Err(AuthError::NoToken)
        ));
    }

    #[test]
    fn reject_insufficient_capabilities() {
        let mw = make_middleware();
        let req = make_auth_request("token", vec!["audio:capture".to_string()]);
        let resp = mw.authenticate(&req).unwrap();
        let session_id = resp.session_id.unwrap();

        assert!(matches!(
            mw.validate_request(&session_id, "file:write"),
            Err(AuthError::InsufficientCapabilities(_))
        ));
    }

    #[test]
    fn rate_limiting() {
        let mw = AuthMiddleware::new(2)
            .with_signing_secret(b"default-test-signing-secret")
            .with_trusted_token("token"); // 2 requests per minute
        let req = make_auth_request("token", vec![]);
        let resp = mw.authenticate(&req).unwrap();
        let session_id = resp.session_id.unwrap();

        assert!(mw.validate_request(&session_id, "custom:method").is_ok());
        assert!(mw.validate_request(&session_id, "custom:method").is_ok());
        assert!(matches!(
            mw.validate_request(&session_id, "custom:method"),
            Err(AuthError::RateLimited)
        ));
    }

    #[test]
    fn revoke_session() {
        let mw = make_middleware();
        let req = make_auth_request("token", vec![]);
        let resp = mw.authenticate(&req).unwrap();
        let session_id = resp.session_id.unwrap();

        assert!(mw.validate_request(&session_id, "test").is_ok());
        mw.revoke_session(&session_id);
        assert!(matches!(
            mw.validate_request(&session_id, "test"),
            Err(AuthError::NoToken)
        ));
    }

    #[test]
    fn cleanup_expired_sessions() {
        let mw = make_middleware();
        assert_eq!(mw.active_sessions(), 0);
        let req = make_auth_request("token", vec![]);
        let _ = mw.authenticate(&req).unwrap();
        assert_eq!(mw.active_sessions(), 1);
        mw.cleanup_expired();
        // Token has 3600s TTL so still active
        assert_eq!(mw.active_sessions(), 1);
    }

    #[test]
    fn test_auth_middleware_hmac_signature_verification_and_tamper_rejection() {
        let secret = b"super-secret-hmac-key-for-ipc";
        let mw = AuthMiddleware::new(100)
            .with_signing_secret(secret)
            .with_trusted_token("valid-token-123");

        let req = make_auth_request("valid-token-123", vec!["audio:capture".to_string()]);
        let resp = mw.authenticate(&req).unwrap();
        assert!(resp.success);
        let session_id = resp.session_id.unwrap();

        // Valid signature passes
        assert!(mw.validate_request(&session_id, "audio:capture").is_ok());

        // Tampered token signature fails
        {
            let mut sessions = mw.sessions.write();
            if let Some(session) = sessions.get_mut(&session_id) {
                // Corrupt signature
                session.token.signature = vec![0xDE, 0xAD, 0xBE, 0xEF];
            }
        }
        let err = mw.validate_request(&session_id, "audio:capture");
        assert!(matches!(err, Err(AuthError::AuthenticationFailed(_))));
    }

    #[test]
    fn test_unconfigured_or_empty_signing_secret_fails_closed() {
        // 1. Unconfigured secret in authenticate fails closed
        let mw_unconfigured = AuthMiddleware::new(100).with_trusted_token("valid-token-123");
        let req = make_auth_request("valid-token-123", vec!["audio:capture".to_string()]);
        let err = mw_unconfigured.authenticate(&req);
        assert!(matches!(err, Err(AuthError::AuthenticationFailed(msg)) if msg.contains("Signing secret not configured")));

        // 2. Empty secret in authenticate fails closed
        let mw_empty = AuthMiddleware::new(100)
            .with_signing_secret(b"")
            .with_trusted_token("valid-token-123");
        let err2 = mw_empty.authenticate(&req);
        assert!(matches!(err2, Err(AuthError::AuthenticationFailed(msg)) if msg.contains("Signing secret is empty")));

        // 3. Unconfigured secret in validate_request fails closed even if session exists
        let mw_configured = AuthMiddleware::new(100)
            .with_signing_secret(b"secret")
            .with_trusted_token("valid-token-123");
        let resp = mw_configured.authenticate(&req).unwrap();
        let session_id = resp.session_id.unwrap();
        assert!(mw_configured.validate_request(&session_id, "audio:capture").is_ok());

        // Now clear the secret on the middleware
        *mw_configured.signing_secret.write() = None;
        let err3 = mw_configured.validate_request(&session_id, "audio:capture");
        assert!(matches!(err3, Err(AuthError::AuthenticationFailed(msg)) if msg.contains("Signing secret not configured")));

        // Set empty secret
        *mw_configured.signing_secret.write() = Some(vec![]);
        let err4 = mw_configured.validate_request(&session_id, "audio:capture");
        assert!(matches!(err4, Err(AuthError::AuthenticationFailed(msg)) if msg.contains("Signing secret is empty")));
    }

    #[test]
    fn test_unconfigured_trust_store_fails_closed() {
        // Unconfigured trusted tokens must fail closed by default, even if secret is configured
        let mw = AuthMiddleware::new(100).with_signing_secret(b"some-secret");
        let req = make_auth_request("arbitrary-credential", vec![]);
        let err = mw.authenticate(&req);
        assert!(
            matches!(err, Err(AuthError::AuthenticationFailed(ref msg)) if msg.contains("Trust store not configured")),
            "Expected unconfigured trust store failure, got: {:?}",
            err
        );
    }

    #[test]
    fn test_empty_trust_store_fails_closed() {
        let mw = AuthMiddleware::new(100).with_signing_secret(b"some-secret");
        // Explicitly set an empty hash set in trusted_token_hashes
        *mw.trusted_token_hashes.write() = Some(std::collections::HashSet::new());
        let req = make_auth_request("arbitrary-credential", vec![]);
        let err = mw.authenticate(&req);
        assert!(
            matches!(err, Err(AuthError::AuthenticationFailed(ref msg)) if msg.contains("Trust store is empty")),
            "Expected empty trust store failure, got: {:?}",
            err
        );
    }

    #[test]
    fn test_unknown_credential_rejected() {
        let mw = AuthMiddleware::new(100)
            .with_signing_secret(b"some-secret")
            .with_trusted_token("expected-trusted-token");
        let req = make_auth_request("unknown-attacker-credential", vec![]);
        let err = mw.authenticate(&req);
        assert!(
            matches!(err, Err(AuthError::AuthenticationFailed(ref msg)) if msg.contains("Invalid token")),
            "Expected unknown credential rejection, got: {:?}",
            err
        );
    }

    #[test]
    fn test_valid_credential_accepted() {
        let mw = AuthMiddleware::new(100)
            .with_signing_secret(b"some-secret")
            .with_trusted_token("my-trusted-token");
        let req = make_auth_request("my-trusted-token", vec!["audio:capture".to_string()]);
        let res = mw.authenticate(&req);
        assert!(res.is_ok(), "Valid credential must succeed");
        let resp = res.unwrap();
        assert!(resp.success);
        assert_eq!(resp.granted_capabilities, vec!["audio:capture"]);
    }

    #[test]
    fn test_unauthorized_capability_request_rejected() {
        let mw = AuthMiddleware::new(100)
            .with_signing_secret(b"some-secret")
            .with_trusted_token("my-trusted-token");
        // Request capability not in system or allowed capabilities
        let req = make_auth_request(
            "my-trusted-token",
            vec!["admin:grant_root_privileges".to_string()],
        );
        let err = mw.authenticate(&req);
        assert!(
            matches!(err, Err(AuthError::AuthenticationFailed(ref msg)) if msg.contains("Unauthorized capability requested")),
            "Expected unauthorized capability rejection, got: {:?}",
            err
        );
    }

    #[test]
    fn test_explicit_allowed_capabilities_policy() {
        // Restrict allowed capabilities strictly to "audio:capture"
        let mw = AuthMiddleware::new(100)
            .with_signing_secret(b"some-secret")
            .with_trusted_token("my-trusted-token")
            .with_allowed_capabilities(vec!["audio:capture"]);

        // 1. Permitted capability passes
        let req_ok = make_auth_request("my-trusted-token", vec!["audio:capture".to_string()]);
        assert!(mw.authenticate(&req_ok).is_ok());

        // 2. System capability "file:read" that is NOT in explicit allowed list gets rejected
        let req_denied = make_auth_request("my-trusted-token", vec!["file:read".to_string()]);
        let err = mw.authenticate(&req_denied);
        assert!(
            matches!(err, Err(AuthError::AuthenticationFailed(ref msg)) if msg.contains("Unauthorized capability requested: 'file:read'")),
            "Expected policy rejection for file:read, got: {:?}",
            err
        );
    }

    #[test]
    fn test_apikey_auth_method() {
        let mw = AuthMiddleware::new(100)
            .with_signing_secret(b"some-secret")
            .with_trusted_token("api-key-12345");
        let req = AuthRequest {
            method: AuthMethod::ApiKey,
            credentials: serde_json::json!({"key": "api-key-12345"}),
            requested_capabilities: vec!["audio:capture".to_string()],
            client_version: "1.0.0".to_string(),
            client_name: "api-client".to_string(),
        };
        let res = mw.authenticate(&req);
        assert!(res.is_ok(), "ApiKey authentication must succeed with valid key");
    }
}
