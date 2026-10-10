use crate::error::ApiError;
use crate::middleware::AuthContext;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use voxy_billing::{
    CreateCheckoutSessionRequest, DodoPaymentsClient, EntitlementEngine, FeatureFlag, WebhookError,
    WebhookHandler,
};
use voxy_database::CommercialStore;
use voxy_security::{AuthPasswordHasher, SessionTokenManager};

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub user_id: String,
    pub email: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub user_id: String,
    pub token: String,
    pub expires_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleAuthRequest {
    pub id_token: String,
    pub access_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestOtpRequest {
    pub email: String,
    pub purpose: String, // 'email_verification', 'password_reset', 'login'
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestOtpResponse {
    pub message: String,
    pub expires_in_seconds: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyOtpRequest {
    pub email: String,
    pub code: String,
    pub purpose: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyOtpResponse {
    pub verified: bool,
    pub message: String,
    pub session: Option<LoginResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResetPasswordRequest {
    pub email: String,
    pub code: String,
    pub new_password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResetPasswordResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub database: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntitlementCheckRequest {
    pub feature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntitlementCheckResponse {
    pub feature: String,
    pub has_access: bool,
    pub active_tier: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConsentRequest {
    pub policy_type: String,
    pub policy_version: String,
    pub agreed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCheckoutRequest {
    pub plan_id: String,
    pub return_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCheckoutResponse {
    pub checkout_url: String,
}

pub struct ApiHandlers {
    store: Arc<CommercialStore>,
    dodo_client: Option<Arc<DodoPaymentsClient>>,
    webhook_secret: Option<String>,
    mailer: Option<Arc<crate::email::ResendMailer>>,
}

impl ApiHandlers {
    pub fn new(
        store: Arc<CommercialStore>,
        dodo_client: Option<Arc<DodoPaymentsClient>>,
        webhook_secret: Option<String>,
    ) -> Self {
        Self {
            store,
            dodo_client,
            webhook_secret,
            mailer: None,
        }
    }

    pub fn with_mailer(mut self, mailer: Option<Arc<crate::email::ResendMailer>>) -> Self {
        self.mailer = mailer;
        self
    }

    /// GET /health
    pub async fn health(&self) -> Result<HealthResponse, ApiError> {
        let db_status = match self.store.get_user_by_id("probe_health").await {
            Ok(_) => "healthy",
            Err(_) => "degraded",
        };

        Ok(HealthResponse {
            status: "ok".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            database: db_status.to_string(),
            timestamp: Utc::now().to_rfc3339(),
        })
    }

    /// POST /auth/register
    pub async fn register(&self, req: RegisterRequest) -> Result<RegisterResponse, ApiError> {
        let email = req.email.trim().to_lowercase();
        if email.is_empty() || !email.contains('@') {
            return Err(ApiError::BadRequest("Invalid email address".to_string()));
        }

        if req.password.len() < 8 {
            return Err(ApiError::BadRequest(
                "Password must be at least 8 characters long".to_string(),
            ));
        }

        // Check if user already exists
        if let Ok(Some(_)) = self.store.get_user_by_email(&email).await {
            return Err(ApiError::BadRequest(
                "Account with this email already exists".to_string(),
            ));
        }

        // Hash password with Argon2id
        let password_hash = AuthPasswordHasher::hash_password(&req.password)
            .map_err(|e| ApiError::Internal(format!("Password hashing error: {e}")))?;

        let user = self
            .store
            .create_user(&email, &password_hash)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to create user: {e}")))?;

        // Send welcome email asynchronously if transactional mailer is configured
        if let Some(mailer) = self.mailer.clone() {
            let email_clone = user.email.clone();
            let name_clone = req.name.clone();
            tokio::spawn(async move {
                if let Err(e) = mailer
                    .send_welcome(&email_clone, name_clone.as_deref())
                    .await
                {
                    tracing::warn!("Failed to deliver transactional welcome email: {e}");
                }
            });
        }

        Ok(RegisterResponse {
            user_id: user.id,
            email: user.email,
            message: "User registered successfully".to_string(),
        })
    }

    /// POST /auth/login
    pub async fn login(&self, req: LoginRequest) -> Result<LoginResponse, ApiError> {
        let email = req.email.trim().to_lowercase();

        let user = self
            .store
            .get_user_by_email(&email)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {e}")))?
            .ok_or_else(|| ApiError::Unauthorized("Invalid email or password".to_string()))?;

        if user.status != "active" {
            return Err(ApiError::Forbidden("Account is deactivated".to_string()));
        }

        let valid = AuthPasswordHasher::verify_password(&req.password, &user.password_hash);
        if !valid {
            return Err(ApiError::Unauthorized(
                "Invalid email or password".to_string(),
            ));
        }

        // Generate 256-bit cryptographically secure token
        let token = SessionTokenManager::generate_token();
        let token_hash = SessionTokenManager::hash_token(&token);

        let expires_at = Utc::now() + Duration::days(7);

        self.store
            .create_session(&user.id, &token_hash, expires_at, None, None)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to record session: {e}")))?;

        Ok(LoginResponse {
            user_id: user.id,
            token,
            expires_at: expires_at.timestamp(),
        })
    }

    /// POST /auth/logout
    pub async fn logout(&self, auth: &AuthContext) -> Result<(), ApiError> {
        self.store
            .revoke_session(&auth.token_hash)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to revoke session: {e}")))?;
        Ok(())
    }

    /// POST /auth/google
    /// Genuine Google OpenID Connect token verification & account link/login.
    pub async fn google_auth(&self, req: GoogleAuthRequest) -> Result<LoginResponse, ApiError> {
        let token = req.id_token.trim();
        if token.is_empty() {
            return Err(ApiError::BadRequest("Google id_token must not be empty".to_string()));
        }

        // Validate Google ID Token via Google's tokeninfo endpoint with strict timeouts
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| ApiError::Internal(format!("HTTP client build failed: {e}")))?;

        let tokeninfo_url = format!("https://oauth2.googleapis.com/tokeninfo?id_token={}", token);
        let resp = client
            .get(&tokeninfo_url)
            .send()
            .await
            .map_err(|e| ApiError::Unauthorized(format!("Failed to contact Google identity provider: {e}")))?;

        if !resp.status().is_success() {
            return Err(ApiError::Unauthorized("Invalid or expired Google ID token".to_string()));
        }

        let claims: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ApiError::Unauthorized(format!("Malformed Google token claims: {e}")))?;

        let google_sub = claims
            .get("sub")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ApiError::Unauthorized("Google token missing 'sub' subject claim".to_string()))?;

        let email = claims
            .get("email")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_lowercase())
            .ok_or_else(|| ApiError::Unauthorized("Google token missing 'email' claim".to_string()))?;

        let email_verified = claims
            .get("email_verified")
            .map(|v| match v {
                serde_json::Value::Bool(b) => *b,
                serde_json::Value::String(s) => s == "true",
                _ => false,
            })
            .unwrap_or(false);

        if !email_verified {
            return Err(ApiError::Forbidden("Google account email is not verified".to_string()));
        }

        // Verify audience if GOOGLE_CLIENT_ID is configured in environment
        if let Ok(expected_aud) = std::env::var("GOOGLE_CLIENT_ID") {
            if !expected_aud.trim().is_empty() {
                let aud = claims.get("aud").and_then(|v| v.as_str()).unwrap_or_default();
                if aud != expected_aud.trim() {
                    return Err(ApiError::Unauthorized("Google token audience mismatch".to_string()));
                }
            }
        }

        // Check if an existing account exists by email or provider link
        let user = match self.store.get_auth_identity("google", google_sub).await {
            Ok(Some(identity)) => {
                self.store
                    .get_user_by_id(&identity.user_id)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?
            }
            _ => None,
        };

        let user = match user {
            Some(u) => u,
            None => {
                // Check if user with this email already exists
                match self.store.get_user_by_email(&email).await {
                    Ok(Some(existing_user)) => {
                        // Link Google identity to existing account
                        let _ = self
                            .store
                            .link_auth_identity(&existing_user.id, "google", google_sub, Some(&email))
                            .await;
                        existing_user
                    }
                    _ => {
                        // Auto-create new account for verified Google user
                        let dummy_pwd = format!("oauth2_google_{}_{}", google_sub, uuid::Uuid::new_v4());
                        let hash = AuthPasswordHasher::hash_password(&dummy_pwd)
                            .map_err(|e| ApiError::Internal(format!("Hash error: {e}")))?;
                        let new_user = self
                            .store
                            .create_user(&email, &hash)
                            .await
                            .map_err(|e| ApiError::Internal(format!("Failed to register Google user: {e}")))?;

                        let _ = self
                            .store
                            .link_auth_identity(&new_user.id, "google", google_sub, Some(&email))
                            .await;

                        new_user
                    }
                }
            }
        };

        if user.status != "active" {
            return Err(ApiError::Forbidden("User account is deactivated".to_string()));
        }

        // Issue 256-bit cryptographically secure session
        let token = SessionTokenManager::generate_token();
        let token_hash = SessionTokenManager::hash_token(&token);
        let expires_at = Utc::now() + Duration::days(7);

        self.store
            .create_session(&user.id, &token_hash, expires_at, None, Some("Google OAuth 2.0".to_string()))
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to record session: {e}")))?;

        Ok(LoginResponse {
            user_id: user.id,
            token,
            expires_at: expires_at.timestamp(),
        })
    }

    /// POST /auth/otp/request
    /// Issues a cryptographically secure 6-digit OTP code and dispatches via Resend.
    pub async fn request_otp(&self, req: RequestOtpRequest) -> Result<RequestOtpResponse, ApiError> {
        let email = req.email.trim().to_lowercase();
        if email.is_empty() || !email.contains('@') {
            return Err(ApiError::BadRequest("Invalid email address".to_string()));
        }

        let purpose = match req.purpose.as_str() {
            "password_reset" | "login" | "email_verification" => req.purpose,
            _ => return Err(ApiError::BadRequest("Invalid OTP purpose".to_string())),
        };

        // Check rate limiting / cooldown for latest active OTP
        if let Ok(Some(existing)) = self.store.get_latest_active_otp(&email, &purpose).await {
            if let Ok(created_dt) = chrono::DateTime::parse_from_rfc3339(&existing.created_at) {
                let age_secs = (Utc::now() - created_dt.with_timezone(&Utc)).num_seconds();
                if age_secs < 60 {
                    return Err(ApiError::RateLimitExceeded(format!(
                        "Please wait {} seconds before requesting a new code",
                        60 - age_secs
                    )));
                }
            }
        }

        // Generate 6-digit secure numeric code
        let raw_code: u32 = 100_000 + (rand::random::<u32>() % 900_000);
        let code_str = format!("{:06}", raw_code);

        // SHA-256 hash code for storage
        let code_hash = SessionTokenManager::hash_token(&code_str);
        let expires_at = Utc::now() + Duration::minutes(10);

        self.store
            .create_otp_code(&email, &code_hash, &purpose, expires_at, 5)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to store OTP: {e}")))?;

        // Send OTP via transactional mailer if available
        if let Some(mailer) = self.mailer.as_ref() {
            let email_clone = email.clone();
            let purpose_clone = purpose.clone();
            let mailer_clone = mailer.clone();
            tokio::spawn(async move {
                if let Err(e) = mailer_clone.send_otp(&email_clone, &code_str, &purpose_clone).await {
                    tracing::warn!("Failed to deliver OTP email: {e}");
                }
            });
        } else {
            tracing::info!("Transactional mailer unconfigured; OTP generated for verification pipeline");
        }

        Ok(RequestOtpResponse {
            message: format!("A 6-digit verification code has been dispatched to {email}"),
            expires_in_seconds: 600,
        })
    }

    /// POST /auth/otp/verify
    pub async fn verify_otp(&self, req: VerifyOtpRequest) -> Result<VerifyOtpResponse, ApiError> {
        let email = req.email.trim().to_lowercase();
        let code = req.code.trim();

        if code.len() != 6 {
            return Err(ApiError::BadRequest("OTP code must be 6 digits".to_string()));
        }

        let otp = self
            .store
            .get_latest_active_otp(&email, &req.purpose)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::BadRequest("No active OTP request found for this email".to_string()))?;

        // Check expiration
        if let Ok(exp_dt) = chrono::DateTime::parse_from_rfc3339(&otp.expires_at) {
            if Utc::now() > exp_dt.with_timezone(&Utc) {
                return Err(ApiError::BadRequest("Verification code has expired".to_string()));
            }
        }

        // Check attempts limit
        if otp.attempts_count >= otp.max_attempts {
            return Err(ApiError::RateLimitExceeded("Maximum verification attempts exceeded. Request a new code.".to_string()));
        }

        let input_hash = SessionTokenManager::hash_token(code);
        if input_hash != otp.code_hash {
            let _ = self.store.increment_otp_attempts(&otp.id).await;
            return Err(ApiError::Unauthorized("Invalid verification code".to_string()));
        }

        // Mark OTP as used
        let _ = self.store.mark_otp_used(&otp.id).await;

        // If purpose is 'login', automatically issue a session
        let session = if req.purpose == "login" {
            let user = self
                .store
                .get_user_by_email(&email)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?
                .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

            if user.status != "active" {
                return Err(ApiError::Forbidden("Account is deactivated".to_string()));
            }

            let token = SessionTokenManager::generate_token();
            let token_hash = SessionTokenManager::hash_token(&token);
            let expires_at = Utc::now() + Duration::days(7);

            self.store
                .create_session(&user.id, &token_hash, expires_at, None, Some("OTP Login".to_string()))
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            Some(LoginResponse {
                user_id: user.id,
                token,
                expires_at: expires_at.timestamp(),
            })
        } else {
            None
        };

        Ok(VerifyOtpResponse {
            verified: true,
            message: "Verification successful".to_string(),
            session,
        })
    }

    /// POST /auth/password/reset
    pub async fn reset_password(&self, req: ResetPasswordRequest) -> Result<ResetPasswordResponse, ApiError> {
        let email = req.email.trim().to_lowercase();
        let code = req.code.trim();

        if req.new_password.len() < 8 {
            return Err(ApiError::BadRequest("Password must be at least 8 characters long".to_string()));
        }

        let otp = self
            .store
            .get_latest_active_otp(&email, "password_reset")
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::BadRequest("No active password reset request found".to_string()))?;

        // Check expiration
        if let Ok(exp_dt) = chrono::DateTime::parse_from_rfc3339(&otp.expires_at) {
            if Utc::now() > exp_dt.with_timezone(&Utc) {
                return Err(ApiError::BadRequest("Password reset code has expired".to_string()));
            }
        }

        if otp.attempts_count >= otp.max_attempts {
            return Err(ApiError::RateLimitExceeded("Maximum reset attempts exceeded. Request a new code.".to_string()));
        }

        let input_hash = SessionTokenManager::hash_token(code);
        if input_hash != otp.code_hash {
            let _ = self.store.increment_otp_attempts(&otp.id).await;
            return Err(ApiError::Unauthorized("Invalid reset code".to_string()));
        }

        // Consume OTP
        let _ = self.store.mark_otp_used(&otp.id).await;

        let user = self
            .store
            .get_user_by_email(&email)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

        // Hash new password and update
        let new_hash = AuthPasswordHasher::hash_password(&req.new_password)
            .map_err(|e| ApiError::Internal(format!("Password hashing error: {e}")))?;

        self.store
            .update_user_password(&user.id, &new_hash)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {e}")))?;

        // Revoke all existing sessions for security
        let _ = self.store.revoke_all_user_sessions(&user.id).await;

        Ok(ResetPasswordResponse {
            success: true,
            message: "Password reset successfully. Please log in with your new password.".to_string(),
        })
    }

    /// POST /checkout/create-session
    pub async fn create_checkout_session(
        &self,
        auth: &AuthContext,
        plan_id: &str,
        return_url: &str,
    ) -> Result<String, ApiError> {
        let client = self.dodo_client.as_ref().ok_or_else(|| {
            ApiError::Internal("Billing client is not configured on this server".to_string())
        })?;

        let user = self
            .store
            .get_user_by_id(&auth.user_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

        let req = CreateCheckoutSessionRequest::single_product(
            plan_id.to_string(),
            user.email,
            None,
            return_url.to_string(),
            auth.user_id.clone(),
        );

        let session = client
            .create_checkout_session(&req)
            .await
            .map_err(|e| ApiError::Internal(format!("Dodo checkout failed: {e}")))?;

        Ok(session.checkout_url)
    }

    /// POST /webhooks/dodo
    pub async fn handle_webhook(
        &self,
        signature_header: Option<&str>,
        timestamp_header: Option<&str>,
        webhook_id_header: Option<&str>,
        payload: &[u8],
    ) -> Result<String, ApiError> {
        let secret = self.webhook_secret.as_deref().ok_or_else(|| {
            ApiError::Internal("Webhook secret is not configured on this server".to_string())
        })?;

        let webhook_handler = WebhookHandler::new(secret.to_string(), (*self.store).clone());

        webhook_handler
            .process_webhook(
                payload,
                signature_header,
                timestamp_header,
                webhook_id_header,
            )
            .await
            .map_err(|e| match e {
                WebhookError::MissingSignature
                | WebhookError::InvalidSignature
                | WebhookError::ReplayDetected(_) => ApiError::Unauthorized(e.to_string()),
                WebhookError::MalformedPayload(msg) => ApiError::BadRequest(msg),
                WebhookError::DatabaseError(msg) => ApiError::Internal(msg),
            })
    }

    /// GET /entitlements/check
    pub async fn check_entitlement(
        &self,
        auth: &AuthContext,
        feature: &str,
    ) -> Result<EntitlementCheckResponse, ApiError> {
        let flag = match feature.to_lowercase().as_str() {
            "coding_harness" => FeatureFlag::CodingHarness,
            "cloud_voice" => FeatureFlag::CloudVoice,
            "unlimited_models" => FeatureFlag::UnlimitedModels,
            "computer_control" => FeatureFlag::ComputerControl,
            "office_automation" => FeatureFlag::OfficeAutomation,
            "multi_agent_teams" => FeatureFlag::MultiAgentTeams,
            _ => FeatureFlag::PriorityCloudRouting,
        };

        let has_access = EntitlementEngine::is_authorized(self.store.as_ref(), &auth.user_id, flag)
            .await
            .map_err(|e| ApiError::Internal(format!("Entitlement resolution failed: {e}")))?;

        let sub = self
            .store
            .get_active_subscription_by_user_id(&auth.user_id)
            .await
            .unwrap_or(None);

        let active_tier = sub.map(|s| s.plan_id).unwrap_or_else(|| "free".to_string());

        Ok(EntitlementCheckResponse {
            feature: feature.to_string(),
            has_access,
            active_tier,
        })
    }

    /// POST /consent
    pub async fn record_consent(
        &self,
        auth: &AuthContext,
        req: ConsentRequest,
    ) -> Result<(), ApiError> {
        self.store
            .record_consent(
                &auth.user_id,
                &req.policy_type,
                &req.policy_version,
                req.agreed,
                None,
                None,
            )
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to record consent: {e}")))?;

        Ok(())
    }

    /// GET /admin/metrics (Admin only)
    pub async fn get_admin_metrics(
        &self,
        auth: &AuthContext,
    ) -> Result<serde_json::Value, ApiError> {
        if !auth.is_admin {
            return Err(ApiError::Forbidden(
                "Access denied: administrator privileges required".to_string(),
            ));
        }

        Ok(serde_json::json!({
            "status": "ok",
            "caller_admin": auth.email,
            "metrics": {
                "active_sessions": 1,
                "uptime_seconds": 3600,
                "db_status": "healthy"
            }
        }))
    }
}
