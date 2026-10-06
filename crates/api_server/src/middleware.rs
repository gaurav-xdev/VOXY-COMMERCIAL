use crate::error::ApiError;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use voxy_database::CommercialStore;
use voxy_security::SessionTokenManager;

/// Authenticated context populated by `AuthMiddleware`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    pub user_id: String,
    pub session_id: String,
    pub email: String,
    pub token_hash: String,
    pub is_admin: bool,
}

/// Authentication middleware that validates bearer tokens against the commercial SQLite store.
#[derive(Clone)]
pub struct AuthMiddleware {
    store: Arc<CommercialStore>,
}

impl AuthMiddleware {
    pub fn new(store: Arc<CommercialStore>) -> Self {
        Self { store }
    }

    /// Authenticate a request from its raw Authorization header.
    pub async fn authenticate(&self, auth_header: Option<&str>) -> Result<AuthContext, ApiError> {
        let header = auth_header.ok_or_else(|| {
            ApiError::Unauthorized("Missing 'Authorization' header in request".to_string())
        })?;

        if !header.starts_with("Bearer ") {
            return Err(ApiError::Unauthorized(
                "Invalid Authorization header format; expected 'Bearer <token>'".to_string(),
            ));
        }

        let raw_token = header["Bearer ".len()..].trim();
        if raw_token.is_empty() {
            return Err(ApiError::Unauthorized(
                "Empty bearer token provided".to_string(),
            ));
        }

        // Hash token to lookup in database (tokens stored as SHA-256 hashes)
        let token_hash = SessionTokenManager::hash_token(raw_token);

        let session = self
            .store
            .get_session_by_token_hash(&token_hash)
            .await
            .map_err(|e| ApiError::Internal(format!("Database query failed: {e}")))?
            .ok_or_else(|| {
                ApiError::Unauthorized("Invalid or revoked session token".to_string())
            })?;

        if session.revoked_at.is_some() {
            return Err(ApiError::Unauthorized(
                "Session token has been revoked".to_string(),
            ));
        }

        // Parse expires_at
        if let Ok(exp_dt) = DateTime::parse_from_rfc3339(&session.expires_at) {
            if Utc::now() > exp_dt.with_timezone(&Utc) {
                return Err(ApiError::Unauthorized(
                    "Session token has expired".to_string(),
                ));
            }
        }

        // Look up user to verify status
        let user = self
            .store
            .get_user_by_id(&session.user_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database query failed: {e}")))?
            .ok_or_else(|| {
                ApiError::Unauthorized("Associated user account not found".to_string())
            })?;

        if user.status != "active" {
            return Err(ApiError::Forbidden(
                "User account is deactivated".to_string(),
            ));
        }

        let is_admin = user.email.ends_with("@osmoo.in")
            || user.email.ends_with("@osmiora.com")
            || std::env::var("OSMOO_ADMIN_EMAILS")
                .map(|e| {
                    e.split(',')
                        .any(|admin| admin.trim().eq_ignore_ascii_case(&user.email))
                })
                .unwrap_or(false);

        Ok(AuthContext {
            user_id: user.id,
            session_id: session.id,
            email: user.email,
            token_hash,
            is_admin,
        })
    }

    /// Enforce admin role requirement on an authenticated context.
    pub fn require_admin(&self, auth: &AuthContext) -> Result<(), ApiError> {
        if !auth.is_admin {
            return Err(ApiError::Forbidden(
                "Access denied: administrator privileges required".to_string(),
            ));
        }
        Ok(())
    }
}

/// Sliding-window rate limiter per client key (IP address or user ID).
#[derive(Debug, Clone)]
pub struct RateLimitMiddleware {
    max_requests: usize,
    window_duration: Duration,
    request_records: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
}

impl RateLimitMiddleware {
    pub fn new(max_requests: usize, window_duration: Duration) -> Self {
        Self {
            max_requests,
            window_duration,
            request_records: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check whether a client key is within allowed rate limits.
    pub fn check(&self, client_key: &str) -> Result<(), ApiError> {
        let now = Instant::now();
        let mut records = self.request_records.write();

        let timestamps = records.entry(client_key.to_string()).or_default();

        // Evict expired timestamps outside window
        let cutoff = now.checked_sub(self.window_duration).unwrap_or(now);
        timestamps.retain(|&ts| ts > cutoff);

        if timestamps.len() >= self.max_requests {
            return Err(ApiError::RateLimitExceeded(format!(
                "Too many requests. Limit is {} per {}s. Please back off.",
                self.max_requests,
                self.window_duration.as_secs()
            )));
        }

        timestamps.push(now);
        Ok(())
    }
}
