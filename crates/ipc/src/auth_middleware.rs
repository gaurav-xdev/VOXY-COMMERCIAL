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
    trusted_token_hashes: RwLock<Option<std::collections::HashSet<Vec<u8>>>>,
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
        }
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

                // Cryptographic validation against configured trusted tokens
                {
                    let guard = self.trusted_token_hashes.read();
                    if let Some(ref trusted) = *guard {
                        use sha2::{Digest, Sha256};
                        let mut hasher = Sha256::new();
                        hasher.update(token_str.as_bytes());
                        let hash = hasher.finalize().to_vec();
                        if !trusted.contains(&hash) {
                            return Err(AuthError::AuthenticationFailed(
                                "Invalid token hash".into(),
                            ));
                        }
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

                let token = CapabilityToken::new(
                    &request.client_name,
                    "voxy-auth",
                    granted_capabilities,
                    3600,
                    token_str.as_bytes().to_vec(),
                );

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
        let mw = AuthMiddleware::new(2); // 2 requests per minute
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
}
