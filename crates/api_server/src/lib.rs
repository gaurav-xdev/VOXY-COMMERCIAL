//! Commercial Web & Server-Side REST API architecture for VOXY COM (REQ-API-01).
//!
//! Provides production-grade route handling, Argon2id authentication,
//! session lifecycle management, sliding-window rate limiting, Dodo Payments integration,
//! and structured RFC-compliant JSON error responses.

pub mod error;
pub mod handlers;
pub mod middleware;
pub mod router;

pub use error::{ApiError, ApiErrorBody, ApiResponseEnvelope};
pub use handlers::{
    ApiHandlers, ConsentRequest, EntitlementCheckRequest, EntitlementCheckResponse, HealthResponse,
    LoginRequest, LoginResponse, RegisterRequest, RegisterResponse,
};
pub use middleware::{AuthContext, AuthMiddleware, RateLimitMiddleware};
pub use router::{ApiRequest, ApiResponse, ApiRouter};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::tempdir;
    use voxy_database::{CommercialStore, DatabaseConfig, SqliteDatabase, StorageProvider};

    async fn setup_test_router() -> (Arc<CommercialStore>, ApiRouter) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("commercial_api_test.db");
        let config = DatabaseConfig {
            path: Some(db_path.to_string_lossy().to_string()),
            ..Default::default()
        };
        let db = Arc::new(SqliteDatabase::new());
        db.connect(&config).await.unwrap();

        let store = Arc::new(CommercialStore::new(db));
        store.initialize_schema().await.unwrap();
        store.seed_default_plans().await.unwrap();

        let auth_mw = Arc::new(AuthMiddleware::new(store.clone()));
        let rate_limiter = Arc::new(RateLimitMiddleware::new(100, Duration::from_secs(60)));
        let handlers = Arc::new(ApiHandlers::new(store.clone(), None));

        let router = ApiRouter::new(handlers, auth_mw, rate_limiter);
        (store, router)
    }

    #[tokio::test]
    async fn test_api_structured_error_responses() {
        let (_, router) = setup_test_router().await;

        // 1. Non-existent route returns 404 with structured error envelope
        let req = ApiRequest::new("GET", "/non/existent/endpoint", "127.0.0.1");
        let resp = router.dispatch(req).await;

        assert_eq!(resp.status_code, 404);
        let envelope: ApiResponseEnvelope<()> = serde_json::from_slice(&resp.body).unwrap();
        assert_eq!(envelope.success, false);
        assert!(envelope.data.is_none());

        let err = envelope.error.unwrap();
        assert_eq!(err.code, "NOT_FOUND");
        assert!(err.message.contains("not found"));
        assert!(!err.request_id.is_empty());
        assert!(!err.timestamp.is_empty());

        // 2. Malformed JSON on register returns 400 with BAD_REQUEST
        let mut malformed_req = ApiRequest::new("POST", "/auth/register", "127.0.0.1");
        malformed_req.body = b"{ bad json".to_vec();
        let resp400 = router.dispatch(malformed_req).await;

        assert_eq!(resp400.status_code, 400);
        let env400: ApiResponseEnvelope<()> = serde_json::from_slice(&resp400.body).unwrap();
        assert_eq!(env400.error.unwrap().code, "BAD_REQUEST");
    }

    #[tokio::test]
    async fn test_api_rate_limiting() {
        let (store, _) = setup_test_router().await;
        let strict_limiter = Arc::new(RateLimitMiddleware::new(5, Duration::from_secs(60)));
        let auth_mw = Arc::new(AuthMiddleware::new(store.clone()));
        let handlers = Arc::new(ApiHandlers::new(store, None));
        let router = ApiRouter::new(handlers, auth_mw, strict_limiter);

        // Limit is 5 requests per 60s
        for i in 1..=5 {
            let req = ApiRequest::new("GET", "/health", "192.168.1.100");
            let resp = router.dispatch(req).await;
            assert_eq!(resp.status_code, 200, "Request {i} should succeed");
        }

        // 6th request from same IP must be rate limited with 429
        let req6 = ApiRequest::new("GET", "/health", "192.168.1.100");
        let resp6 = router.dispatch(req6).await;
        assert_eq!(resp6.status_code, 429);

        let env: ApiResponseEnvelope<()> = serde_json::from_slice(&resp6.body).unwrap();
        assert_eq!(env.success, false);
        let err = env.error.unwrap();
        assert_eq!(err.code, "RATE_LIMIT_EXCEEDED");
        assert!(err.message.contains("Too many requests"));

        // Request from different IP should still succeed
        let req_other_ip = ApiRequest::new("GET", "/health", "192.168.1.101");
        let resp_other = router.dispatch(req_other_ip).await;
        assert_eq!(resp_other.status_code, 200);
    }

    #[tokio::test]
    async fn test_api_auth_middleware() {
        let (_store, router) = setup_test_router().await;

        // 1. Calling protected endpoint without token returns 401 UNAUTHORIZED
        let check_req = EntitlementCheckRequest {
            feature: "coding_harness".to_string(),
        };
        let req_no_auth = ApiRequest::new("POST", "/entitlements/check", "10.0.0.1")
            .with_json(&check_req);

        let resp_unauth = router.dispatch(req_no_auth).await;
        assert_eq!(resp_unauth.status_code, 401);
        let env_unauth: ApiResponseEnvelope<()> = serde_json::from_slice(&resp_unauth.body).unwrap();
        assert_eq!(env_unauth.error.unwrap().code, "UNAUTHORIZED");

        // 2. Register user via /auth/register
        let reg_req = RegisterRequest {
            email: "alice@example.com".to_string(),
            password: "SuperSecurePassword123!".to_string(),
            name: Some("Alice Architect".to_string()),
        };
        let reg_api_req = ApiRequest::new("POST", "/auth/register", "10.0.0.1")
            .with_json(&reg_req);
        let reg_resp = router.dispatch(reg_api_req).await;
        assert_eq!(reg_resp.status_code, 201);

        // 3. Login user via /auth/login to get bearer token
        let login_req = LoginRequest {
            email: "alice@example.com".to_string(),
            password: "SuperSecurePassword123!".to_string(),
        };
        let login_api_req = ApiRequest::new("POST", "/auth/login", "10.0.0.1")
            .with_json(&login_req);
        let login_resp = router.dispatch(login_api_req).await;
        assert_eq!(login_resp.status_code, 200);

        let login_env: ApiResponseEnvelope<LoginResponse> =
            serde_json::from_slice(&login_resp.body).unwrap();
        let token = login_env.data.unwrap().token;
        assert!(!token.is_empty());

        // 4. Access protected endpoint with valid bearer token
        let req_with_auth = ApiRequest::new("POST", "/entitlements/check", "10.0.0.1")
            .with_header("authorization", &format!("Bearer {token}"))
            .with_json(&check_req);

        let resp_auth = router.dispatch(req_with_auth).await;
        assert_eq!(resp_auth.status_code, 200);

        // 5. Logout and verify token is revoked
        let logout_req = ApiRequest::new("POST", "/auth/logout", "10.0.0.1")
            .with_header("authorization", &format!("Bearer {token}"));
        let logout_resp = router.dispatch(logout_req).await;
        assert_eq!(logout_resp.status_code, 200);

        // Subsequent call with revoked token returns 401
        let req_revoked = ApiRequest::new("POST", "/entitlements/check", "10.0.0.1")
            .with_header("authorization", &format!("Bearer {token}"))
            .with_json(&check_req);
        let resp_revoked = router.dispatch(req_revoked).await;
        assert_eq!(resp_revoked.status_code, 401);
    }
}
