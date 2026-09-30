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
        }
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
}
