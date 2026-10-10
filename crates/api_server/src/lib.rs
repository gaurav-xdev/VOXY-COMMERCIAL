//! Commercial Web & Server-Side REST API architecture for VOXY COM (REQ-API-01).
//!
//! Provides production-grade route handling, Argon2id authentication,
//! session lifecycle management, sliding-window rate limiting, Dodo Payments integration,
//! and structured RFC-compliant JSON error responses.

pub mod email;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod router;

pub use email::{EmailError, ResendMailer, SendEmailRequest, SendEmailResponse};
pub use error::{ApiError, ApiErrorBody, ApiResponseEnvelope};
pub use handlers::{
    ApiHandlers, ConsentRequest, CreateCheckoutRequest, CreateCheckoutResponse,
    EntitlementCheckRequest, EntitlementCheckResponse, GoogleAuthRequest, HealthResponse,
    LoginRequest, LoginResponse, RegisterRequest, RegisterResponse, RequestOtpRequest,
    RequestOtpResponse, ResetPasswordRequest, ResetPasswordResponse, VerifyOtpRequest,
    VerifyOtpResponse,
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
        let handlers = Arc::new(ApiHandlers::new(
            store.clone(),
            None,
            Some("test_webhook_secret_key".to_string()),
        ));

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
        assert!(!envelope.success);
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
        let handlers = Arc::new(ApiHandlers::new(store, None, None));
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
        assert!(!env.success);
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
        let req_no_auth =
            ApiRequest::new("POST", "/entitlements/check", "10.0.0.1").with_json(&check_req);

        let resp_unauth = router.dispatch(req_no_auth).await;
        assert_eq!(resp_unauth.status_code, 401);
        let env_unauth: ApiResponseEnvelope<()> =
            serde_json::from_slice(&resp_unauth.body).unwrap();
        assert_eq!(env_unauth.error.unwrap().code, "UNAUTHORIZED");

        // 2. Register user via /auth/register
        let reg_req = RegisterRequest {
            email: "alice@example.com".to_string(),
            password: "SuperSecurePassword123!".to_string(),
            name: Some("Alice Architect".to_string()),
        };
        let reg_api_req = ApiRequest::new("POST", "/auth/register", "10.0.0.1").with_json(&reg_req);
        let reg_resp = router.dispatch(reg_api_req).await;
        assert_eq!(reg_resp.status_code, 201);

        // 3. Login user via /auth/login to get bearer token
        let login_req = LoginRequest {
            email: "alice@example.com".to_string(),
            password: "SuperSecurePassword123!".to_string(),
        };
        let login_api_req =
            ApiRequest::new("POST", "/auth/login", "10.0.0.1").with_json(&login_req);
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

    #[tokio::test]
    async fn test_api_webhook_and_checkout_dispatch() {
        use base64::Engine;
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let (store, router) = setup_test_router().await;

        // 1. Unauthenticated checkout creation returns 401
        let checkout_req = CreateCheckoutRequest {
            plan_id: "plan_pro_monthly".to_string(),
            return_url: "https://voxy.ai/success".to_string(),
        };
        let req_unauth = ApiRequest::new("POST", "/checkout/create-session", "10.0.0.1")
            .with_json(&checkout_req);
        let resp_unauth = router.dispatch(req_unauth).await;
        assert_eq!(resp_unauth.status_code, 401);

        // 2. Webhook called with missing signature returns 401
        let test_user = store
            .create_user("webhook_user@voxy.ai", "dummy_hash")
            .await
            .unwrap();

        let dummy_payload = serde_json::json!({
            "event_id": "evt_test_123",
            "event_type": "subscription.active",
            "data": {
                "user_id": test_user.id,
                "subscription_id": "sub_999",
                "customer_id": "cus_999",
                "product_id": "plan_pro_monthly"
            }
        });
        let req_no_sig =
            ApiRequest::new("POST", "/webhooks/dodo", "10.0.0.1").with_json(&dummy_payload);
        let resp_no_sig = router.dispatch(req_no_sig).await;
        assert_eq!(resp_no_sig.status_code, 401);

        // 3. Webhook called with forged signature returns 401
        let req_bad_sig = ApiRequest::new("POST", "/webhooks/dodo", "10.0.0.1")
            .with_header("webhook-signature", "v1,ZmFrZXNpZ25hdHVyZQ==")
            .with_header("webhook-id", "msg_test_123")
            .with_header(
                "webhook-timestamp",
                &chrono::Utc::now().timestamp().to_string(),
            )
            .with_json(&dummy_payload);
        let resp_bad_sig = router.dispatch(req_bad_sig).await;
        assert_eq!(resp_bad_sig.status_code, 401);

        // 4. Webhook called with legitimate Standard Webhooks signature returns 200
        let secret = "test_webhook_secret_key";
        let body_str = serde_json::to_string(&dummy_payload).unwrap();
        let msg_id = "msg_valid_test_001";
        let ts = chrono::Utc::now().timestamp().to_string();
        let to_sign = format!("{}.{}.{}", msg_id, ts, body_str);

        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(to_sign.as_bytes());
        let sig_bytes = mac.finalize().into_bytes();
        let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig_bytes);
        let sig_header_val = format!("v1,{sig_b64}");

        let req_valid = ApiRequest::new("POST", "/webhooks/dodo", "10.0.0.1")
            .with_header("webhook-signature", &sig_header_val)
            .with_header("webhook-id", msg_id)
            .with_header("webhook-timestamp", &ts)
            .with_json(&dummy_payload);

        let resp_valid = router.dispatch(req_valid).await;
        assert_eq!(
            resp_valid.status_code, 200,
            "Valid webhook should return 200"
        );

        // Verify webhook event was recorded in DB
        let processed = store.is_webhook_processed("evt_test_123").await.unwrap();
        assert!(
            processed,
            "Webhook event must be marked processed in database"
        );
    }

    #[tokio::test]
    async fn test_admin_rbac_protection() {
        let (_store, router) = setup_test_router().await;

        // 1. Unauthenticated request to /admin/metrics returns 401
        let req_unauth = ApiRequest::new("GET", "/admin/metrics", "10.0.0.1");
        let resp_unauth = router.dispatch(req_unauth).await;
        assert_eq!(resp_unauth.status_code, 401);

        // 2. Register ordinary non-admin user
        let reg_user = RegisterRequest {
            email: "bob@standard-user.com".to_string(),
            password: "StandardPassword123!".to_string(),
            name: Some("Bob".to_string()),
        };
        let _ = router
            .dispatch(ApiRequest::new("POST", "/auth/register", "10.0.0.1").with_json(&reg_user))
            .await;

        let login_user = LoginRequest {
            email: "bob@standard-user.com".to_string(),
            password: "StandardPassword123!".to_string(),
        };
        let login_resp = router
            .dispatch(ApiRequest::new("POST", "/auth/login", "10.0.0.1").with_json(&login_user))
            .await;
        let env_user: ApiResponseEnvelope<LoginResponse> =
            serde_json::from_slice(&login_resp.body).unwrap();
        let user_token = env_user.data.unwrap().token;

        // 3. Ordinary user attempting to call /admin/metrics is strictly DENIED with 403 Forbidden
        let req_user = ApiRequest::new("GET", "/admin/metrics", "10.0.0.1")
            .with_header("authorization", &format!("Bearer {user_token}"));
        let resp_user = router.dispatch(req_user).await;
        assert_eq!(
            resp_user.status_code, 403,
            "Standard user must be denied admin endpoints with 403"
        );

        // 4. Register official admin user (@osmoo.in)
        let reg_admin = RegisterRequest {
            email: "admin@osmoo.in".to_string(),
            password: "AdminSecurePassword123!".to_string(),
            name: Some("Osmoo Admin".to_string()),
        };
        let _ = router
            .dispatch(ApiRequest::new("POST", "/auth/register", "10.0.0.1").with_json(&reg_admin))
            .await;

        let login_admin = LoginRequest {
            email: "admin@osmoo.in".to_string(),
            password: "AdminSecurePassword123!".to_string(),
        };
        let admin_login_resp = router
            .dispatch(ApiRequest::new("POST", "/auth/login", "10.0.0.1").with_json(&login_admin))
            .await;
        let env_admin: ApiResponseEnvelope<LoginResponse> =
            serde_json::from_slice(&admin_login_resp.body).unwrap();
        let admin_token = env_admin.data.unwrap().token;

        // 5. Admin user calling /admin/metrics is ALLOWED with 200 OK
        let req_admin = ApiRequest::new("GET", "/admin/metrics", "10.0.0.1")
            .with_header("authorization", &format!("Bearer {admin_token}"));
        let resp_admin = router.dispatch(req_admin).await;
        assert_eq!(
            resp_admin.status_code, 200,
            "Admin user must be permitted to access admin endpoints"
        );
    }

    #[tokio::test]
    async fn test_otp_flow_and_password_reset() {
        let (store, router) = setup_test_router().await;

        // 1. Register a test user
        let user_email = "alice@example.com";
        let initial_pwd = "OldPassword123!";
        let reg_req = RegisterRequest {
            email: user_email.to_string(),
            password: initial_pwd.to_string(),
            name: Some("Alice".to_string()),
        };
        let reg_resp = router
            .dispatch(ApiRequest::new("POST", "/auth/register", "10.0.0.1").with_json(&reg_req))
            .await;
        assert_eq!(reg_resp.status_code, 201);

        // 2. Request OTP for password reset
        let otp_req = RequestOtpRequest {
            email: user_email.to_string(),
            purpose: "password_reset".to_string(),
        };
        let otp_resp = router
            .dispatch(ApiRequest::new("POST", "/auth/otp/request", "10.0.0.1").with_json(&otp_req))
            .await;
        assert_eq!(otp_resp.status_code, 200);

        // Immediate secondary request within 60s cooldown is rejected with 429
        let otp_resp2 = router
            .dispatch(ApiRequest::new("POST", "/auth/otp/request", "10.0.0.1").with_json(&otp_req))
            .await;
        assert_eq!(otp_resp2.status_code, 429);

        // 3. Fetch active OTP from database to obtain code hash
        let active_otp = store
            .get_latest_active_otp(user_email, "password_reset")
            .await
            .unwrap()
            .expect("OTP must be recorded in database");
        assert_eq!(active_otp.attempts_count, 0);

        // 4. Try wrong OTP code -> returns 401 and increments attempts
        let wrong_verify = VerifyOtpRequest {
            email: user_email.to_string(),
            code: "000000".to_string(),
            purpose: "password_reset".to_string(),
        };
        let verify_wrong_resp = router
            .dispatch(ApiRequest::new("POST", "/auth/otp/verify", "10.0.0.1").with_json(&wrong_verify))
            .await;
        assert_eq!(verify_wrong_resp.status_code, 401);

        let reloaded_otp = store
            .get_latest_active_otp(user_email, "password_reset")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reloaded_otp.attempts_count, 1);

        // 5. Test password reset with invalid code -> returns 401
        let bad_reset = ResetPasswordRequest {
            email: user_email.to_string(),
            code: "999999".to_string(),
            new_password: "NewSuperSecretPassword123!".to_string(),
        };
        let bad_reset_resp = router
            .dispatch(ApiRequest::new("POST", "/auth/password/reset", "10.0.0.1").with_json(&bad_reset))
            .await;
        assert_eq!(bad_reset_resp.status_code, 401);

        // 6. Test password reset with valid code: create a known OTP
        let known_code = "654321";
        let known_hash = voxy_security::SessionTokenManager::hash_token(known_code);
        let exp = chrono::Utc::now() + chrono::Duration::minutes(10);
        let _ = store
            .create_otp_code(user_email, &known_hash, "password_reset", exp, 5)
            .await
            .unwrap();

        let valid_reset = ResetPasswordRequest {
            email: user_email.to_string(),
            code: known_code.to_string(),
            new_password: "NewSuperSecretPassword123!".to_string(),
        };
        let valid_reset_resp = router
            .dispatch(ApiRequest::new("POST", "/auth/password/reset", "10.0.0.1").with_json(&valid_reset))
            .await;
        assert_eq!(valid_reset_resp.status_code, 200);

        // 7. Verify login succeeds with new password and fails with old password
        let login_old = LoginRequest {
            email: user_email.to_string(),
            password: initial_pwd.to_string(),
        };
        let login_old_resp = router
            .dispatch(ApiRequest::new("POST", "/auth/login", "10.0.0.1").with_json(&login_old))
            .await;
        assert_eq!(login_old_resp.status_code, 401);

        let login_new = LoginRequest {
            email: user_email.to_string(),
            password: "NewSuperSecretPassword123!".to_string(),
        };
        let login_new_resp = router
            .dispatch(ApiRequest::new("POST", "/auth/login", "10.0.0.1").with_json(&login_new))
            .await;
        assert_eq!(login_new_resp.status_code, 200);
    }
}
